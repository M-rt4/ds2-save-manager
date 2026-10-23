use super::character::{self, Attributes, CharacterEdit};
use super::file::{character_entry, world_entry, SaveFile, HEADER_ENTRY, SUMMARY_ENTRY};
use super::header;
use super::inventory::{InventoryOp, MAIN};
use super::items::Category;
use super::testdata::{self, STEAM};
use super::transfer;
use super::Edition;

fn save(edition: Edition, names: &[(usize, &str, u16)]) -> SaveFile {
    SaveFile::from_blocks(edition, &testdata::save_blocks(names, 4096))
}

#[test]
fn detects_both_editions() {
    for ed in Edition::ALL {
        let s = save(ed, &[(0, "Nele", 37)]);
        assert_eq!(s.source_edition, ed);
        assert_eq!(s.block_count(), 23);
        assert!(s.checksum_failures().is_empty());
    }
}

#[test]
fn rejects_unknown_key_and_non_bnd4() {
    let mut raw = testdata::container(Edition::Scholar, &testdata::save_blocks(&[], 64));
    let e = SaveFile::parse(b"not a save at all, clearly not one".repeat(4)).unwrap_err();
    assert_eq!(e.code, "notSave");
    // Flip a ciphertext byte in entry 0's first block: its length prefix turns to noise.
    let entries = super::bnd4::parse(&raw).unwrap();
    raw[entries[0].offset + 32] ^= 0xFF;
    raw[entries[1].offset + 32] ^= 0xFF;
    assert_eq!(SaveFile::parse(raw).unwrap_err().code, "unknownKey");
}

#[test]
fn unchanged_save_encodes_to_identical_bytes() {
    let s = save(Edition::Original, &[(0, "Nele", 37), (3, "Lucatiel", 12)]);
    assert_eq!(s.encode(), s.raw());
}

#[test]
fn conversion_reencrypts_every_entry_with_the_other_key() {
    let original = save(Edition::Original, &[(0, "Nele", 37)]);
    let converted = transfer::convert(&original, Edition::Scholar);
    let reread = SaveFile::parse(converted.encode()).unwrap();
    assert_eq!(reread.source_edition, Edition::Scholar);
    assert!(reread.checksum_failures().is_empty());
    for i in 0..original.block_count() {
        assert_eq!(reread.block(i), original.block(i), "entry {i}");
    }
    let back = SaveFile::parse(transfer::convert(&reread, Edition::Original).encode()).unwrap();
    assert_eq!(back.encode(), original.raw());
}

#[test]
fn reads_slots_and_characters() {
    let s = save(Edition::Scholar, &[(0, "Nele", 37), (4, "Pate", 12)]);
    let slots: Vec<_> = (0..10).map(|i| character::summary(&s, i)).collect();
    assert!(slots[0].occupied && slots[4].occupied && !slots[1].occupied);
    assert_eq!(slots[4].name, "Pate");
    assert_eq!(slots[0].play_time, 3600);

    let c = character::read(&s, 0);
    assert!(c.layout_ok);
    assert_eq!(c.name, "Nele");
    assert_eq!(c.level, 37);
    assert_eq!(c.attributes.adaptability, 10);
    assert_eq!(c.journey, 1);

    let inv = &c.inventory.items;
    assert_eq!(inv.len(), 3);
    assert_eq!(inv[0].name, "Estus Flask");
    assert_eq!(inv[0].charges, Some((3, 3)));
    assert_eq!(inv[1].name, "Longsword");
    assert_eq!((inv[1].reinforcement, inv[1].infusion.as_deref()), (5, Some("Fire")));
    assert_eq!(inv[2].quantity, Some(10));
    assert_eq!(header::steam_id(s.block(HEADER_ENTRY)), Some(STEAM));
}

#[test]
fn deleted_characters_show_as_ghosts() {
    let mut blocks = testdata::save_blocks(&[(0, "Nele", 37)], 64);
    blocks[character_entry(2)] = testdata::character_block("Gone", [10; 9]);
    let s = SaveFile::from_blocks(Edition::Scholar, &blocks);
    let ghost = character::summary(&s, 2);
    assert!(!ghost.occupied && ghost.ghost);
    assert_eq!(ghost.name, "Gone");
}

#[test]
fn edits_status_and_keeps_the_menu_in_step() {
    let mut s = save(Edition::Scholar, &[(0, "Nele", 37)]);
    let mut attrs = character::read(&s, 0).attributes;
    attrs.vigor = 20;
    attrs.faith = 15;
    character::apply(
        &mut s,
        0,
        &CharacterEdit { name: Some("Aldia".into()), souls: Some(50_000), attributes: Some(attrs), ..Default::default() },
    )
    .unwrap();

    let c = character::read(&s, 0);
    assert_eq!((c.name.as_str(), c.souls, c.level), ("Aldia", 50_000, 52));
    for e in [HEADER_ENTRY, SUMMARY_ENTRY] {
        let rec = header::read_record(s.block(e), 0).unwrap();
        assert_eq!((rec.name.as_str(), rec.level), ("Aldia", 52));
        let r = header::record(s.block(e), 0).unwrap();
        let vigor_in_menu = super::bytes::u16_at(&s.block(e)[r], header::rec::ATTRS);
        assert_eq!(vigor_in_menu, 20);
    }
    let reread = SaveFile::parse(s.encode()).unwrap();
    assert_eq!(character::read(&reread, 0).name, "Aldia");
}

#[test]
fn rejects_bad_edits_without_changing_anything() {
    let mut s = save(Edition::Scholar, &[(0, "Nele", 37)]);
    let before = s.encode();
    let bad_attrs = Attributes { vigor: 0, ..character::read(&s, 0).attributes };
    assert!(character::apply(&mut s, 0, &CharacterEdit { attributes: Some(bad_attrs), ..Default::default() }).is_err());
    assert!(character::apply(&mut s, 0, &CharacterEdit { name: Some("x".repeat(17)), ..Default::default() }).is_err());
    assert!(character::apply(&mut s, 5, &CharacterEdit { souls: Some(1), ..Default::default() }).is_err());
    // The first op is fine, the second targets an empty record: neither may land.
    let ops = vec![
        InventoryOp::SetQuantity { offset: MAIN.start + 48, quantity: 50 },
        InventoryOp::Remove { offset: MAIN.start + 32 },
    ];
    assert!(character::apply(&mut s, 0, &CharacterEdit { inventory: ops, ..Default::default() }).is_err());
    assert_eq!(s.encode(), before);
}

#[test]
fn inventory_ops() {
    let mut s = save(Edition::Scholar, &[(0, "Nele", 37)]);
    // Ops apply in order: the flask's record is freed before the dagger looks for room.
    let ops = vec![
        InventoryOp::SetQuantity { offset: MAIN.start + 48, quantity: 500 },
        InventoryOp::Remove { offset: MAIN.start },
        InventoryOp::Add { id: 60_970_000, quantity: 5, reinforcement: 0, infusion: 0 },
        InventoryOp::Add { id: 1_000_000, quantity: 1, reinforcement: 3, infusion: 2 },
    ];
    character::apply(&mut s, 0, &CharacterEdit { inventory: ops, ..Default::default() }).unwrap();
    let inv = character::read(&s, 0).inventory.items;
    let names: Vec<_> = inv.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["Dagger", "Longsword", "Titanite Shard"]);
    // Clamped to 99, then +5 on the existing stack, clamped again.
    assert_eq!(inv[2].quantity, Some(99));
    // The new dagger took the first free record and the longsword's durability.
    assert_eq!(inv[0].offset, MAIN.start);
    assert_eq!((inv[0].reinforcement, inv[0].infusion.as_deref(), inv[0].durability), (3, Some("Magic"), Some(40.0)));
    assert_eq!(inv[0].category, Category::Weapon);

    let key = InventoryOp::Add { id: 50_800_000, quantity: 1, reinforcement: 0, infusion: 0 };
    assert!(character::apply(&mut s, 0, &CharacterEdit { inventory: vec![key], ..Default::default() }).is_err());
}

#[test]
fn transfers_a_character_between_editions() {
    let src = save(Edition::Original, &[(0, "Nele", 37), (1, "Benhart", 20)]);
    let mut dst = save(Edition::Scholar, &[(0, "Vengarl", 50), (2, "Gilligan", 9)]);
    transfer::transfer(&src, 1, &mut dst, 2).unwrap();

    assert_eq!(dst.block(character_entry(2)), src.block(character_entry(1)));
    assert_eq!(dst.block(world_entry(2)), src.block(world_entry(1)));
    assert_eq!(character::summary(&dst, 2).name, "Benhart");
    assert_eq!(header::read_record(dst.block(SUMMARY_ENTRY), 2).unwrap().name, "Benhart");
    // The other slots and the account stay the target's.
    assert_eq!(character::summary(&dst, 0).name, "Vengarl");
    assert_eq!(dst.edition, Edition::Scholar);
    let reread = SaveFile::parse(dst.encode()).unwrap();
    assert_eq!(reread.source_edition, Edition::Scholar);
    assert_eq!(character::summary(&reread, 2).name, "Benhart");
}

#[test]
fn merge_keeps_the_targets_system_blocks() {
    let src = SaveFile::from_blocks(Edition::Original, &testdata::save_blocks(&[(0, "Nele", 37)], 1024));
    let base = SaveFile::from_blocks(Edition::Scholar, &testdata::save_blocks(&[(5, "Lucatiel", 30)], 4096));
    let check = transfer::compare(&src, &base);
    assert!(check.compatible);
    assert_eq!(check.system_size_differences, vec![21]);

    let merged = transfer::merge(&src, &base).unwrap();
    assert_eq!(merged.edition, Edition::Scholar);
    assert_eq!(merged.block(21).len(), 4096);
    assert_eq!(character::summary(&merged, 0).name, "Nele");
    assert!(!character::summary(&merged, 5).occupied);
}

#[test]
fn reports_chunk_version_differences() {
    let mut blocks = testdata::save_blocks(&[(0, "Nele", 37)], 64);
    blocks[HEADER_ENTRY] = testdata::header_block(&[(0, "Nele", 37)], 0x6A);
    let src = SaveFile::from_blocks(Edition::Original, &blocks);
    let dst = save(Edition::Scholar, &[]);
    let check = transfer::compare(&src, &dst);
    assert!(check.compatible);
    assert_eq!(check.version_notes.len(), 1);
    assert_eq!((check.version_notes[0].chunk, check.version_notes[0].source, check.version_notes[0].target), (4, 0x6A, 0x6D));
}

#[test]
fn rewrites_the_account() {
    let mut s = save(Edition::Scholar, &[(0, "Nele", 37)]);
    let other = 0x0110_0001_0000_1337;
    header::set_steam_id(s.block_mut(HEADER_ENTRY), other).unwrap();
    assert_eq!(header::steam_id(s.block(HEADER_ENTRY)), Some(other));
    assert!(header::set_steam_id(s.block_mut(HEADER_ENTRY), 42).is_err());
}

/// Runs against a real decrypted save when `DS2_SAMPLE_DIR` points at a folder of
/// `USERDATA_00`..`USERDATA_22` (e.g. the reference editor's `decrypted_output`).
#[test]
fn real_sample_when_available() {
    let Some(dir) = std::env::var_os("DS2_SAMPLE_DIR") else { return };
    let dir = std::path::PathBuf::from(dir);
    let blocks: Vec<Vec<u8>> =
        (0..23).map(|i| std::fs::read(dir.join(format!("USERDATA_{i:02}"))).expect("sample entry")).collect();
    for ed in Edition::ALL {
        let s = SaveFile::from_blocks(ed, &blocks);
        assert_eq!(s.source_edition, ed);
        let c = character::read(&s, 0);
        assert!(c.layout_ok);
        assert_eq!(c.attributes.level(), c.level as i32);
        assert_eq!(header::read_record(s.block(HEADER_ENTRY), 0).unwrap().name, c.name);
        assert!(c.inventory.unknown < c.inventory.items.len() / 10, "unknown items: {}", c.inventory.unknown);
        let occupied = (0..10).filter(|&i| character::summary(&s, i).occupied).count();
        assert!(occupied > 0);
        assert!(header::steam_id(s.block(HEADER_ENTRY)).is_some());
        eprintln!("{ed:?}: {} lv{} {} items, {occupied} characters", c.name, c.level, c.inventory.items.len());
    }
}

/// Writes real-looking `.sl2` files from a decrypted sample for manual testing:
/// `DS2_SAMPLE_DIR=<USERDATA folder> DS2_SAMPLE_OUT=<folder> cargo test --lib -- --ignored write_sample_saves`
#[test]
#[ignore]
fn write_sample_saves() {
    let dir = std::path::PathBuf::from(std::env::var_os("DS2_SAMPLE_DIR").expect("DS2_SAMPLE_DIR"));
    let out = std::path::PathBuf::from(std::env::var_os("DS2_SAMPLE_OUT").expect("DS2_SAMPLE_OUT"));
    let blocks: Vec<Vec<u8>> =
        (0..23).map(|i| std::fs::read(dir.join(format!("USERDATA_{i:02}"))).expect("sample entry")).collect();
    std::fs::create_dir_all(&out).unwrap();
    for ed in Edition::ALL {
        std::fs::write(out.join(ed.file_name()), testdata::container(ed, &blocks)).unwrap();
    }
}

/// Prints what the engine reads from a real save, for checking a new file by hand:
/// `DS2_INSPECT=<file.sl2> cargo test --lib -- --ignored --nocapture inspect_file`
#[test]
#[ignore]
fn inspect_file() {
    let path = std::env::var_os("DS2_INSPECT").expect("DS2_INSPECT");
    let raw = std::fs::read(&path).unwrap();
    let s = SaveFile::parse(raw.clone()).unwrap();
    eprintln!("edition {:?}, {} bytes, checksum failures {:?}", s.source_edition, raw.len(), s.checksum_failures());
    eprintln!("bnd4 header {:02x?}", &raw[..0x40]);
    eprintln!("account {:?}, last slot {:?}", header::steam_id(s.block(HEADER_ENTRY)), header::last_slot(s.block(HEADER_ENTRY)));
    for e in s.entries() {
        let chunks = super::chunks::walk(s.block(e.index))
            .map(|c| c.iter().map(|c| format!("{:x}/{:x}/{}", c.kind, c.version, c.size)).collect::<Vec<_>>().join(" "))
            .unwrap_or_else(|| "-".into());
        eprintln!("#{:2} {:<12} off {:>8} size {:>8} data {:>8}  {chunks}", e.index, e.name, e.offset, e.size, s.block(e.index).len());
    }
    for i in 0..10 {
        let sm = character::summary(&s, i);
        if sm.occupied || sm.ghost {
            let c = character::read(&s, i);
            eprintln!(
                "slot {i}: {} lv{} class {} journey {} souls {} items {} unknown {} layout_ok {} map {}",
                c.name, c.level, c.class, c.journey, c.souls, c.inventory.items.len(), c.inventory.unknown, c.layout_ok, c.map
            );
        }
    }
    assert_eq!(s.encode(), raw, "re-encoding an unchanged real save must reproduce it byte for byte");
}

#[test]
fn conform_gives_a_save_the_targets_system_data() {
    let src = save(Edition::Original, &[(0, "Nele", 37)]);
    let mut bnd = b"BND4".to_vec();
    bnd.resize(0x200, 3);
    bnd[0x18..0x20].copy_from_slice(b"02000100");
    let mut reg = dummy_regulation(bnd);
    reg.set_stamp(0x65);
    let fmt = transfer::TargetFormat { edition: Edition::Scholar, regulation: reg, capacity: 8192, title_version: 0x6E, entry: None };
    let mut out = transfer::convert(&src, Edition::Scholar);
    transfer::conform(&mut out, &fmt).unwrap();
    let reread = SaveFile::parse(out.encode()).unwrap();
    assert_eq!(reread.source_edition, Edition::Scholar);
    assert_eq!(reread.block(21).len(), 8192);
    assert_eq!(transfer::regulation_info(&reread).unwrap().stamp, 0x65);
    let title = super::chunks::find(reread.block(SUMMARY_ENTRY), header::TITLE_CHUNK).unwrap();
    assert_eq!(title.version, 0x6E);
    assert_eq!(character::summary(&reread, 0).name, "Nele");
    assert!(reread.checksum_failures().is_empty());
}

fn dummy_regulation(bnd: Vec<u8>) -> super::regulation::Regulation {
    use std::io::Write;
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&bnd).unwrap();
    let packed = enc.finish().unwrap();
    let mut entry = (packed.len() as u32).to_le_bytes().to_vec();
    entry.extend_from_slice(&(bnd.len() as u32).to_le_bytes());
    entry.extend_from_slice(&packed);
    super::regulation::Regulation::from_entry(&entry).unwrap()
}

/// Conforms a real save to a real target, for checking by hand:
/// `DS2_CONFORM_SRC=<save> DS2_CONFORM_BASE=<save the target game wrote> DS2_GAME_DIR=<game> DS2_CONFORM_OUT=<file>`
#[test]
#[ignore]
fn conform_real_files() {
    use super::regulation::Regulation;
    let var = |k: &str| std::env::var_os(k).map(std::path::PathBuf::from).unwrap_or_else(|| panic!("{k}"));
    let src = SaveFile::parse(std::fs::read(var("DS2_CONFORM_SRC")).unwrap()).unwrap();
    let base = SaveFile::parse(std::fs::read(var("DS2_CONFORM_BASE")).unwrap()).unwrap();
    let from_save = transfer::TargetFormat::from_save(&base).unwrap();
    let from_install = transfer::TargetFormat::from_install(base.source_edition, &var("DS2_GAME_DIR")).unwrap();
    eprintln!("base regulation {:?}", from_save.regulation.info());
    eprintln!("install regulation {:?}", from_install.regulation.info());
    assert_eq!(from_install.regulation, from_save.regulation, "installed regulation + exe stamp must equal what the game writes");
    assert_eq!((from_install.capacity, from_install.title_version), (from_save.capacity, from_save.title_version));
    eprintln!("source regulation {:?}", transfer::regulation_info(&src));

    let mut out = transfer::convert(&src, base.source_edition);
    transfer::conform(&mut out, &from_install).unwrap();
    let account = header::steam_id(base.block(HEADER_ENTRY)).unwrap();
    header::set_steam_id(out.block_mut(HEADER_ENTRY), account).unwrap();
    let bytes = out.encode();
    let reread = SaveFile::parse(bytes.clone()).unwrap();
    assert_eq!(bytes.len(), base.raw().len(), "same container size as a save the game wrote");
    for i in 0..reread.block_count() {
        assert_eq!(reread.entries()[i].size, base.entries()[i].size, "entry {i} size");
        assert_eq!(reread.entries()[i].offset, base.entries()[i].offset, "entry {i} offset");
    }
    assert_eq!(Regulation::from_entry(reread.block(21)).unwrap(), from_save.regulation);
    assert_eq!(&bytes[..0x40], &base.raw()[..0x40], "BND4 header");
    eprintln!("ok: {} bytes, account {account}, {}", bytes.len(), character::summary(&reread, 0).name);
    std::fs::write(var("DS2_CONFORM_OUT"), bytes).unwrap();
}
