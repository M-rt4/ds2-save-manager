//! The JSON document: an unchanged one changes no byte, edits go through the checked
//! writers, fields the editor only shows are reported and left alone, and every problem is
//! named where it is.

use super::character;
use super::document::{apply, export, parse, preview, to_text, Document, ItemDoc, FORMAT};
use super::file::{character_entry, SaveFile, HEADER_ENTRY};
use super::header;
use super::inventory::{KEYS, MAIN};
use super::items::{catalog, Category};
use super::testdata;
use super::Edition;

const OTHER: u64 = 0x0110_0001_0000_1337;
const DAGGER: u32 = 1_000_000;
const SHARD: u32 = 60_970_000;

fn new_item(id: u32, quantity: Option<u16>) -> ItemDoc {
    ItemDoc {
        record: None,
        id,
        name: None,
        quantity,
        reinforcement: None,
        infusion: None,
        durability: None,
        charges: None,
        category: None,
        region: None,
    }
}

fn save() -> SaveFile {
    let mut blocks = testdata::save_blocks(&[(0, "Nele", 37), (3, "Pate", 12)], 64);
    // A character deleted in game, and a key item, which the editor can't remove.
    blocks[character_entry(5)] = testdata::character_block("Gone", [10; 9]);
    let key = catalog().items.iter().find(|d| d.cat == Category::Key).unwrap().id;
    blocks[character_entry(0)][KEYS.start..KEYS.start + 4].copy_from_slice(&key.to_le_bytes());
    blocks[character_entry(0)][KEYS.start + 8] = 1;
    SaveFile::from_blocks(Edition::Scholar, &blocks)
}

fn edited(s: &SaveFile, change: impl FnOnce(&mut Document)) -> String {
    let mut doc = export(s);
    change(&mut doc);
    to_text(&doc)
}

fn nele(doc: &mut Document) -> &mut super::document::CharacterDoc {
    doc.characters.as_mut().unwrap().iter_mut().find(|c| c.slot == 0).unwrap()
}

fn item(doc: &mut Document, record: usize) -> &mut ItemDoc {
    nele(doc).inventory.as_mut().unwrap().iter_mut().find(|i| i.record == Some(record)).unwrap()
}

#[test]
fn an_unchanged_document_changes_no_byte() {
    let mut s = save();
    let raw = s.raw().to_vec();
    let doc = export(&s);
    let text = to_text(&doc);
    assert_eq!(parse(&text).unwrap(), doc, "the text reads back as the same document");
    let p = preview(&s, &text);
    assert!(p.syntax.is_none() && p.problems.is_empty() && p.changes.is_empty(), "{p:?}");
    assert!(apply(&mut s, &text).unwrap().is_empty());
    assert!(s.encode() == raw);
    // Only occupied slots are characters; the deleted one stays in its block.
    let slots: Vec<usize> = doc.characters.unwrap().iter().map(|c| c.slot).collect();
    assert_eq!(slots, [0, 3]);
}

#[test]
fn small_records_read_one_per_line() {
    let s = save();
    let text = to_text(&export(&s));
    let items = character::read(&s, 0).inventory.items.len() + character::read(&s, 3).inventory.items.len();
    assert_eq!(text.lines().filter(|l| l.trim_start().starts_with("{ \"record\":")).count(), items);
    assert!(text.lines().any(|l| l.trim_start().starts_with("\"attributes\": { \"vigor\": 10,")));
    assert!(text.lines().any(|l| l.contains("\"charges\": [3, 3]")));
    assert!(text.starts_with(&format!("{{\n  \"format\": \"{FORMAT}\",\n  \"edition\": \"scholar\",")));
}

#[test]
fn edits_go_through_the_writers() {
    let mut s = save();
    let longsword = MAIN.start + 16;
    let shards = MAIN.start + 48;
    let text = edited(&s, |d| {
        d.account = Some(OTHER.to_string());
        let c = nele(d);
        c.name = Some("Aldia".into());
        c.souls = Some(50_000);
        c.journey = Some(2);
        c.deaths = Some(7);
        let mut a = c.attributes.unwrap();
        a.vigor = 20;
        c.attributes = Some(a);
        c.level = Some(a.level() as u16);
        let cov = c.covenants.as_mut().unwrap();
        cov[3].discovered = Some(true);
        cov[3].rank = Some(2);
        c.covenant = Some(4);
        let inv = c.inventory.as_mut().unwrap();
        inv.retain(|i| i.record != Some(MAIN.start)); // the Estus Flask
        inv.push(ItemDoc {
            record: None,
            id: DAGGER,
            name: None,
            quantity: None,
            reinforcement: Some(3),
            infusion: Some("fire".into()),
            durability: None,
            charges: None,
            category: None,
            region: None,
        });
        item(d, shards).quantity = Some(25);
        item(d, longsword).reinforcement = Some(7);
    });
    let p = preview(&s, &text);
    assert!(p.syntax.is_none() && p.problems.is_empty(), "{p:?}");
    assert!(p.changes.iter().all(|c| c.applies), "{:?}", p.changes);
    let paths: Vec<&str> = p.changes.iter().map(|c| c.place.path.as_str()).collect();
    for want in [
        "account",
        "characters[slot 0].name",
        "characters[slot 0].souls",
        "characters[slot 0].attributes",
        "characters[slot 0].journey",
        "characters[slot 0].deaths",
        "characters[slot 0].covenants[4].discovered",
        "characters[slot 0].covenants[4].rank",
        "characters[slot 0].covenant",
        &format!("characters[slot 0].inventory[record {shards}].quantity"),
        &format!("characters[slot 0].inventory[record {longsword}].reinforcement"),
        &format!("characters[slot 0].inventory[record {}].record", MAIN.start),
    ] {
        assert!(paths.contains(&want), "{want} in {paths:?}");
    }
    assert!(paths.iter().any(|p| p.contains("[new item ")), "{paths:?}");

    let changes = apply(&mut s, &text).unwrap();
    assert_eq!(changes, p.changes);
    let c = character::read(&s, 0);
    assert_eq!((c.name.as_str(), c.souls, c.journey, c.deaths, c.attributes.vigor), ("Aldia", 50_000, 2, 7, 20));
    assert_eq!(c.level as i32, c.attributes.level());
    assert_eq!(c.covenant, 4);
    assert_eq!((c.covenants[3].discovered, c.covenants[3].rank), (true, 2));
    let names: Vec<(&str, Option<u16>, u8)> = c.inventory.items.iter().map(|i| (i.name.as_str(), i.quantity, i.reinforcement)).collect();
    assert!(!names.iter().any(|n| n.0 == "Estus Flask"));
    assert!(names.contains(&("Longsword", None, 7)));
    assert!(names.contains(&("Titanite Shard", Some(25), 0)));
    let dagger = c.inventory.items.iter().find(|i| i.id == DAGGER).unwrap();
    assert_eq!((dagger.reinforcement, dagger.infusion.as_deref()), (3, Some("Fire")));
    // The Load Game menu follows, through character::apply.
    let menu = header::read_record(s.block(HEADER_ENTRY), 0).unwrap();
    assert_eq!((menu.name.as_str(), menu.level as i32), ("Aldia", c.attributes.level()));
    assert_eq!(header::steam_id(s.block(HEADER_ENTRY)), Some(OTHER));
    // What was written reads back as what was asked for.
    let reread = SaveFile::parse(s.encode()).unwrap();
    assert!(preview(&reread, &to_text(&export(&s))).changes.is_empty());
}

#[test]
fn shown_fields_are_reported_and_left_alone() {
    let mut s = save();
    let raw = s.raw().to_vec();
    let text = edited(&s, |d| {
        d.edition = Some(Edition::Original);
        d.menu.as_mut().unwrap()[0].name = "Someone".into();
        d.blocks.as_mut().unwrap()[0].size = 1;
        let c = nele(d);
        c.level = Some(99);
        c.class = Some(7);
        c.hp = Some(1);
        c.soul_memory = Some(1);
        c.hollowing = Some(3);
        c.female = Some(true);
        let longsword = item(d, MAIN.start + 16);
        longsword.durability = Some(1.0);
        longsword.infusion = Some("Dark".into());
        // Names and categories are for reading; they don't even count as changes.
        longsword.name = Some("Anything".into());
        longsword.category = Some(Category::Ring);
    });
    let p = preview(&s, &text);
    assert!(p.problems.is_empty(), "{:?}", p.problems);
    assert_eq!(p.changes.len(), 11, "{:?}", p.changes);
    assert!(p.changes.iter().all(|c| !c.applies));
    assert!(apply(&mut s, &text).is_ok());
    assert!(s.encode() == raw, "nothing written");
}

#[test]
fn problems_are_named_where_they_are() {
    let s = save();
    let shards = MAIN.start + 48;
    let check = |text: String, path: &str| {
        let p = preview(&s, &text);
        assert!(p.problems.iter().any(|x| x.place.path == path), "{path} in {:?} / {:?}", p.problems, p.syntax);
        let mut copy = s.clone();
        assert!(apply(&mut copy, &text).is_err());
        assert!(copy.encode() == s.raw(), "a refused document changes nothing");
    };
    check(edited(&s, |d| nele(d).souls = Some(1_000_000_000)), "characters[slot 0].souls");
    check(
        edited(&s, |d| {
            let c = nele(d);
            let mut a = c.attributes.unwrap();
            a.faith = 0;
            c.attributes = Some(a);
        }),
        "characters[slot 0].attributes",
    );
    check(edited(&s, |d| item(d, shards).quantity = Some(0)), &format!("characters[slot 0].inventory[record {shards}].quantity"));
    check(edited(&s, |d| item(d, shards).id = DAGGER), &format!("characters[slot 0].inventory[record {shards}].id"));
    check(edited(&s, |d| item(d, shards).record = Some(MAIN.start + 1600)), &format!("characters[slot 0].inventory[record {}].record", MAIN.start + 1600));
    check(edited(&s, |d| nele(d).inventory.as_mut().unwrap().retain(|i| i.record != Some(KEYS.start))), &format!("characters[slot 0].inventory[record {}].record", KEYS.start));
    check(
        edited(&s, |d| {
            nele(d).inventory.as_mut().unwrap().push(ItemDoc {
                record: None,
                id: 1,
                name: None,
                quantity: None,
                reinforcement: None,
                infusion: None,
                durability: None,
                charges: None,
                category: None,
                region: None,
            })
        }),
        "characters[slot 0].inventory[new item 4].id",
    );
    check(edited(&s, |d| d.characters.as_mut().unwrap().retain(|c| c.slot != 3)), "characters[slot 3].slot");
    check(edited(&s, |d| d.characters.as_mut().unwrap()[1].slot = 4), "characters[slot 4].slot");
    check(edited(&s, |d| nele(d).covenant = Some(2)), "characters[slot 0].covenant");
    check(edited(&s, |d| d.account = Some("12345".into())), "account");
    check(edited(&s, |d| d.format = "something/2".into()), "format");

    // Text that is not the document: a line and a column.
    let text = to_text(&export(&s));
    let broken = text.replacen("\"souls\": ", "\"soul\": ", 1);
    let p = preview(&s, &broken);
    let e = p.syntax.expect("unknown field");
    assert!(e.message.contains("unknown field `soul`"), "{}", e.message);
    assert_eq!(text.lines().nth(e.line - 1).map(|l| l.contains("\"souls\"")), Some(true), "line {}", e.line);
    let e = preview(&s, &text.replacen("37,", "37,,", 1)).syntax.expect("syntax");
    assert!(e.line > 1 && e.column > 1);
}

#[test]
fn adding_a_held_stackable_joins_its_stack() {
    let mut s = save();
    let shards = MAIN.start + 48; // 10 Titanite Shards
    let add = |d: &mut Document, q| nele(d).inventory.as_mut().unwrap().push(new_item(SHARD, Some(q)));
    let stacked = |p: &super::document::Preview| p.changes.iter().filter_map(|c| c.stack.clone()).map(|x| (x.added, x.total)).collect::<Vec<_>>();

    // Two entries: 10 + 5, then + 90 up to the limit of 99.
    let text = edited(&s, |d| {
        add(d, 5);
        add(d, 90);
    });
    let p = preview(&s, &text);
    assert!(p.problems.is_empty(), "{:?}", p.problems);
    assert_eq!(stacked(&p), [(5, 15), (90, 99)]);
    // A count changed in the same document is the one the new entry joins.
    let p = preview(&s, &edited(&s, |d| {
        item(d, shards).quantity = Some(20);
        add(d, 5);
    }));
    assert_eq!(stacked(&p), [(5, 25)]);
    // A stack removed in the same document is not joined: the entry takes a record.
    let p = preview(&s, &edited(&s, |d| {
        nele(d).inventory.as_mut().unwrap().retain(|i| i.record != Some(shards));
        add(d, 5);
    }));
    assert!(stacked(&p).is_empty());
    assert!(p.changes.iter().any(|c| c.to == "\"Titanite Shard ×5\""), "{:?}", p.changes);

    apply(&mut s, &text).unwrap();
    let held: Vec<(usize, Option<u16>)> = character::read(&s, 0).inventory.items.iter().filter(|i| i.id == SHARD).map(|i| (i.offset, i.quantity)).collect();
    assert_eq!(held, [(shards, Some(99))], "one stack, as the preview said");
}

#[test]
fn a_full_inventory_is_found_before_anything_is_written() {
    let mut blocks = testdata::save_blocks(&[(0, "Nele", 37)], 64);
    let armor = catalog().items.iter().find(|d| d.cat == Category::Armor).unwrap().id;
    for o in MAIN.step_by(16) {
        blocks[character_entry(0)][o..o + 4].copy_from_slice(&armor.to_le_bytes());
    }
    let s = SaveFile::from_blocks(Edition::Scholar, &blocks);
    let text = edited(&s, |d| {
        nele(d).inventory.as_mut().unwrap().push(ItemDoc {
            record: None,
            id: DAGGER,
            name: None,
            quantity: None,
            reinforcement: None,
            infusion: None,
            durability: None,
            charges: None,
            category: None,
            region: None,
        })
    });
    let p = preview(&s, &text);
    assert_eq!(p.problems.len(), 1, "{p:?}");
    assert!(p.problems[0].message.contains("no free record"));
}

/// Exports a real save and imports it again, which must leave every byte as it was:
/// `DS2_DOCUMENT=<save.sl2> cargo test --lib -- --ignored --nocapture document_real_file`
#[test]
#[ignore]
fn document_real_file() {
    let raw = std::fs::read(std::env::var_os("DS2_DOCUMENT").expect("DS2_DOCUMENT")).unwrap();
    let mut s = SaveFile::parse(raw.clone()).unwrap();
    let text = to_text(&export(&s));
    let p = preview(&s, &text);
    assert!(p.syntax.is_none() && p.problems.is_empty() && p.changes.is_empty(), "{p:?}");
    assert!(apply(&mut s, &text).unwrap().is_empty());
    assert!(s.encode() == raw);
    eprintln!("{} lines, {} bytes of JSON; unchanged import leaves the save byte for byte", text.lines().count(), text.len());
}
