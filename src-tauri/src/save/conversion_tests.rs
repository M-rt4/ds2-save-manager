//! Edge cases of `conversion::build`: what a written save carries, where its format and account
//! come from, and what is refused.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::character;
use super::chunks;
use super::conversion::{build, read_target, Built, Change, Formats, Target};
use super::file::{character_entry, SaveFile, HEADER_ENTRY, SUMMARY_ENTRY};
use super::header;
use super::raw;
use super::regulation::{self, Regulation};
use super::testdata::{self, STEAM};
use super::transfer::{self, TargetFormat};
use super::{Edition, Result};

const SCHOLAR_REG: (&str, u8) = ("02000100", 101);
const ORIGINAL_REG: (&str, u8) = ("01001500", 112);
/// Another player's account.
const OTHER: u64 = 0x0110_0001_0000_1337;
const MB: usize = 0x10_0000;

fn regulation(version: &str, stamp: u8, body: &[u8]) -> Regulation {
    let mut bnd = b"BND4".to_vec();
    bnd.resize(0x40, 0);
    bnd[0x18..0x20].copy_from_slice(version.as_bytes());
    bnd[regulation::STAMP_OFFSET] = stamp;
    bnd.extend_from_slice(body);
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&bnd).unwrap();
    let packed = enc.finish().unwrap();
    let mut entry = (packed.len() as u32).to_le_bytes().to_vec();
    entry.extend_from_slice(&(bnd.len() as u32).to_le_bytes());
    entry.extend_from_slice(&packed);
    Regulation::from_entry(&entry).unwrap()
}

/// What the installed game of `edition` expects.
fn format(edition: Edition) -> TargetFormat {
    let (version, stamp) = if edition == Edition::Scholar { SCHOLAR_REG } else { ORIGINAL_REG };
    let (capacity, title_version) = transfer::edition_layout(edition);
    TargetFormat { edition, regulation: regulation(version, stamp, &[7; 0x200]), capacity, title_version, entry: None }
}

/// A save as the game of `fmt.edition` writes it, from decrypted blocks.
fn written_by(fmt: &TargetFormat, blocks: &[Vec<u8>]) -> SaveFile {
    let mut s = SaveFile::from_blocks(fmt.edition, blocks);
    transfer::conform(&mut s, fmt).unwrap();
    SaveFile::parse(s.encode()).unwrap()
}

fn game_save(edition: Edition, names: &[(usize, &str, u16)]) -> SaveFile {
    written_by(&format(edition), &testdata::save_blocks(names, 64))
}

fn with_account(s: &SaveFile, id: u64) -> SaveFile {
    let mut s = s.clone();
    header::set_steam_id(s.block_mut(HEADER_ENTRY), id).unwrap();
    SaveFile::parse(s.encode()).unwrap()
}

fn in_folder(id: u64, edition: Edition) -> PathBuf {
    Path::new("X:/DarkSoulsII").join(header::steam_folder(id)).join(edition.file_name())
}

fn outside(edition: Edition) -> PathBuf {
    Path::new("X:/elsewhere").join(edition.file_name())
}

#[derive(Default)]
struct Opts<'a> {
    slot: Option<(usize, usize)>,
    install: Option<TargetFormat>,
    other: Option<TargetFormat>,
    account: Option<&'a str>,
    same_file: bool,
}

fn convert(source: &SaveFile, edition: Edition, path: &Path, base: Option<&SaveFile>, o: Opts) -> Result<Built> {
    let other = o.other.clone();
    let mut other_save = move || other.clone().map(|f| (f, PathBuf::from("X:/other/save.sl2")));
    build(
        source,
        &Target { slot: o.slot, edition, path, account: o.account, same_file: o.same_file },
        base,
        Formats { install: o.install.map(|f| (Ok(f), "X:/Game".to_string())), other_save: &mut other_save },
    )
}

/// Reads back what would be written and checks it is laid out as the game of `edition`
/// writes its saves.
fn written(b: &Built, edition: Edition) -> SaveFile {
    let bytes = b.out.encode();
    let s = SaveFile::parse(bytes.clone()).unwrap();
    let (capacity, title_version) = transfer::edition_layout(edition);
    let (version, stamp) = if edition == Edition::Scholar { SCHOLAR_REG } else { ORIGINAL_REG };
    assert_eq!(s.source_edition, edition);
    assert!(s.checksum_failures().is_empty());
    assert_eq!(s.block(regulation::ENTRY).len(), capacity);
    let reg = transfer::regulation_info(&s).unwrap();
    assert_eq!((reg.version.as_str(), reg.stamp), (version, stamp));
    assert_eq!(chunks::find(s.block(SUMMARY_ENTRY), header::TITLE_CHUNK).unwrap().version, title_version);
    let e = s.entries();
    assert!(e.windows(2).all(|w| w[0].offset + w[0].size == w[1].offset), "entries back to back, in order");
    assert_eq!(e.last().map(|l| l.offset + l.size), Some(bytes.len()), "nothing after the last entry");
    s
}

fn name(s: &SaveFile, slot: usize) -> Option<String> {
    let sm = character::summary(s, slot);
    sm.occupied.then_some(sm.name)
}

fn account(s: &SaveFile) -> Option<u64> {
    header::steam_id(s.block(HEADER_ENTRY))
}

#[test]
fn scholar_to_original_fits_the_smaller_entry() {
    let src = game_save(Edition::Scholar, &[(0, "Nele", 37), (3, "Pate", 12)]);
    let untouched = src.encode();
    let path = in_folder(STEAM, Edition::Original);
    let b = convert(&src, Edition::Original, &path, None, Opts { install: Some(format(Edition::Original)), ..Default::default() }).unwrap();
    let s = written(&b, Edition::Original);
    assert_eq!((name(&s, 0).as_deref(), name(&s, 3).as_deref()), (Some("Nele"), Some("Pate")));
    for c in [
        Change::Key { from: Edition::Scholar, to: Edition::Original },
        Change::TitleVersion { from: 0x6E, to: 0x6F },
        Change::EntrySize { entry: regulation::ENTRY, from: 2 * MB, to: MB },
        Change::NewFile { path: path.to_string_lossy().into_owned() },
    ] {
        assert!(b.changes.contains(&c), "{c:?} in {:?}", b.changes);
    }
    assert!(b.changes.iter().any(|c| matches!(c, Change::Regulation { to, .. } if to.version == ORIGINAL_REG.0)));
    assert_eq!(src.encode(), untouched, "the source is never changed");
}

#[test]
fn a_regulation_that_does_not_fit_is_refused() {
    let src = game_save(Edition::Scholar, &[(0, "Nele", 37)]);
    let mut x = 0x2545_F491u32;
    let noise: Vec<u8> = (0..MB + MB / 4)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect();
    let mut big = format(Edition::Original);
    big.regulation = regulation(ORIGINAL_REG.0, ORIGINAL_REG.1, &noise);
    let path = in_folder(STEAM, Edition::Original);
    let err = convert(&src, Edition::Original, &path, None, Opts { install: Some(big), ..Default::default() }).err().unwrap();
    assert_eq!(err.code, "incompatible");
}

#[test]
fn one_character_into_a_target_with_the_other_releases_insides() {
    // A Scholar-keyed file another tool made from an original save: the original's
    // regulation, entry size and title version. Like the user's rejected DS2SOFS0000.sl2.
    let mut rekeyed = format(Edition::Original);
    rekeyed.edition = Edition::Scholar;
    let base = written_by(&rekeyed, &testdata::save_blocks(&[(0, "Vengarl", 50)], 64));
    assert_eq!((base.source_edition, base.block(regulation::ENTRY).len()), (Edition::Scholar, MB));
    // Right regulation, wrong entry size: not a save the game wrote either.
    let mut sized = format(Edition::Scholar);
    sized.capacity = MB;
    let half = written_by(&sized, &testdata::save_blocks(&[(0, "Vengarl", 50)], 64));

    let src = game_save(Edition::Original, &[(0, "Nele", 37)]);
    let path = in_folder(STEAM, Edition::Scholar);
    let slot = Some((0, 2));
    for target in [&base, &half] {
        let err = convert(&src, Edition::Scholar, &path, Some(target), Opts { slot, ..Default::default() }).err().unwrap();
        assert_eq!(err.code, "needsGameInfo", "the target can't tell what the game expects");
    }

    let b = convert(&src, Edition::Scholar, &path, Some(&base), Opts { slot, install: Some(format(Edition::Scholar)), ..Default::default() })
        .unwrap();
    assert_eq!(b.format.kind, "install");
    let s = written(&b, Edition::Scholar);
    assert_eq!((name(&s, 0).as_deref(), name(&s, 2).as_deref()), (Some("Vengarl"), Some("Nele")));
    assert!(b.changes.contains(&Change::EntrySize { entry: regulation::ENTRY, from: MB, to: 2 * MB }));
    assert!(b.changes.contains(&Change::TitleVersion { from: 0x6F, to: 0x6E }));

    let b = convert(&src, Edition::Scholar, &path, Some(&half), Opts { slot, other: Some(format(Edition::Scholar)), ..Default::default() })
        .unwrap();
    assert_eq!(b.format.kind, "otherSave");
    written(&b, Edition::Scholar);
}

#[test]
fn account_order_typed_then_folder_then_target_file_then_source() {
    let src = game_save(Edition::Original, &[(0, "Nele", 37)]);
    let base = with_account(&game_save(Edition::Scholar, &[(4, "Lucatiel", 30)]), OTHER);
    let install = || Some(format(Edition::Scholar));
    let typed = 0x0110_0001_0000_BEEFu64.to_string();
    // Target path, target file, typed account; the account written and where it came from.
    type Case<'a> = (PathBuf, Option<&'a SaveFile>, Option<&'a str>, u64, &'a str);
    let cases: [Case; 4] = [
        (in_folder(STEAM, Edition::Scholar), Some(&base), Some(typed.as_str()), 0x0110_0001_0000_BEEF, "manual"),
        (in_folder(OTHER, Edition::Scholar), None, None, OTHER, "folder"),
        (outside(Edition::Scholar), Some(&base), None, OTHER, "targetFile"),
        (outside(Edition::Scholar), None, None, STEAM, "source"),
    ];
    for (path, base, typed, want, origin) in cases {
        let b = convert(&src, Edition::Scholar, &path, base, Opts { install: install(), account: typed, ..Default::default() }).unwrap();
        let s = written(&b, Edition::Scholar);
        assert_eq!((account(&s), b.accounts.origin), (Some(want), origin), "{origin}");
        assert_eq!(b.accounts.result, Some(want.to_string()));
        if want != STEAM {
            let source_id = format!("{STEAM:016x}");
            assert!(raw::find(&s, None, source_id.as_bytes(), 1).unwrap().0.is_empty(), "no trace of the source's account");
            assert!(b.changes.iter().any(|c| matches!(c, Change::Account { to, .. } if *to == want.to_string())));
        }
    }
    let err = convert(&src, Edition::Scholar, &outside(Edition::Scholar), None, Opts { install: install(), account: Some("123"), ..Default::default() });
    assert_eq!(err.err().unwrap().code, "badAccount");
}

#[test]
fn replacing_only_the_target_files_account_is_reported() {
    // Another player's save sits in our folder; our own characters go into it.
    let src = game_save(Edition::Original, &[(0, "Nele", 37)]);
    let base = with_account(&game_save(Edition::Scholar, &[]), OTHER);
    let path = in_folder(STEAM, Edition::Scholar);
    let b = convert(&src, Edition::Scholar, &path, Some(&base), Opts { install: Some(format(Edition::Scholar)), ..Default::default() }).unwrap();
    assert_eq!(account(&written(&b, Edition::Scholar)), Some(STEAM));
    assert!(b.changes.contains(&Change::Account { from: Some(OTHER.to_string()), to: STEAM.to_string(), origin: "folder" }));
}

#[test]
fn a_target_file_of_the_other_release() {
    // An original save under Scholar's file name.
    let src = game_save(Edition::Original, &[(0, "Nele", 37)]);
    let base = game_save(Edition::Original, &[(1, "Benhart", 20)]);
    let path = in_folder(STEAM, Edition::Scholar);
    let err = convert(&src, Edition::Scholar, &path, Some(&base), Opts::default()).err().unwrap();
    assert_eq!(err.code, "needsGameInfo", "an original file can't tell what Scholar expects");
    let b = convert(&src, Edition::Scholar, &path, Some(&base), Opts { install: Some(format(Edition::Scholar)), ..Default::default() }).unwrap();
    let s = written(&b, Edition::Scholar);
    assert_eq!((name(&s, 0).as_deref(), name(&s, 1)), (Some("Nele"), None));
    assert!(b.changes.contains(&Change::Key { from: Edition::Original, to: Edition::Scholar }));
}

#[test]
fn a_relaid_container_is_the_one_the_game_writes() {
    // A save conformed to the other release must come out exactly as that release's game
    // lays out the same data: entries back to back in index order, the table updated.
    let blocks = testdata::save_blocks(&[(0, "Nele", 37)], 64);
    for (from, to) in [(Edition::Original, Edition::Scholar), (Edition::Scholar, Edition::Original)] {
        let fmt = format(to);
        let mut converted = SaveFile::from_blocks(from, &blocks);
        transfer::conform(&mut converted, &fmt).unwrap();
        let mut expected = blocks.clone();
        expected[regulation::ENTRY] = fmt.regulation.to_entry(fmt.capacity).unwrap();
        let title = chunks::find(&expected[SUMMARY_ENTRY], header::TITLE_CHUNK).unwrap();
        expected[SUMMARY_ENTRY][title.offset + 4..title.offset + 8].copy_from_slice(&fmt.title_version.to_le_bytes());
        assert!(converted.encode() == testdata::container(to, &expected), "{from:?} -> {to:?}");
    }
}

#[test]
fn an_unreadable_target_is_refused_not_overwritten() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("test-scratch").join("conversion-target");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(Edition::Scholar.file_name());
    assert!(read_target(&path).unwrap().is_none(), "no file: a new one is written");
    std::fs::write(&path, b"not a save, a text file someone named .sl2").unwrap();
    assert_eq!(read_target(&path).err().unwrap().code, "targetUnreadable");
    std::fs::write(&path, game_save(Edition::Scholar, &[(0, "Nele", 37)]).raw()).unwrap();
    assert_eq!(read_target(&path).unwrap().map(|s| s.source_edition), Some(Edition::Scholar));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_newer_copy_of_the_same_character_is_an_update() {
    let base = game_save(Edition::Scholar, &[(0, "Nele", 37), (1, "Lucatiel", 20)]);
    let mut blocks = testdata::save_blocks(&[(0, "Nele", 37)], 64);
    blocks[character_entry(0)] = testdata::character_block("Nele", [12; 9]);
    // A character deleted in game: its slot is empty, its bytes are not.
    blocks[character_entry(5)] = testdata::character_block("Gone", [10; 9]);
    let src = written_by(&format(Edition::Original), &blocks);
    let path = in_folder(STEAM, Edition::Scholar);
    let install = || Some(format(Edition::Scholar));

    let b = convert(&src, Edition::Scholar, &path, Some(&base), Opts { install: install(), ..Default::default() }).unwrap();
    let nele = Some("Nele".to_string());
    let slots: Vec<_> = b.changes.iter().filter(|c| matches!(c, Change::Slot { .. })).collect();
    assert_eq!(
        slots,
        [
            &Change::Slot { slot: 0, before: nele.clone(), after: nele.clone() },
            &Change::Slot { slot: 1, before: Some("Lucatiel".into()), after: None },
        ],
        "an update, the slot the source has empty, and nothing about slots empty on both sides"
    );
    assert_eq!(character::read(&written(&b, Edition::Scholar), 0).attributes.vigor, 12);

    let b = convert(&src, Edition::Scholar, &path, Some(&base), Opts { slot: Some((0, 0)), install: install(), ..Default::default() }).unwrap();
    assert_eq!(b.changes.iter().filter(|c| matches!(c, Change::Slot { .. })).count(), 1);
    assert!(b.changes.contains(&Change::Slot { slot: 0, before: nele.clone(), after: nele }));
    assert_eq!(name(&written(&b, Edition::Scholar), 1).as_deref(), Some("Lucatiel"), "one character leaves the others alone");
}

#[test]
fn the_source_file_itself() {
    let s = game_save(Edition::Scholar, &[(0, "Nele", 37), (1, "Pate", 12)]);
    let path = in_folder(STEAM, Edition::Scholar);
    let same = |slot, install| Opts { slot, install, same_file: true, ..Default::default() };

    // As the other release, whole or one character: refused, whatever else is at hand.
    for slot in [None, Some((0, 1))] {
        let err = convert(&s, Edition::Original, &path, Some(&s), same(slot, Some(format(Edition::Original)))).err().unwrap();
        assert_eq!(err.code, "sameFile", "{slot:?}");
    }
    // A repair: the file can't vouch for itself.
    assert_eq!(convert(&s, Edition::Scholar, &path, Some(&s), same(None, None)).err().unwrap().code, "needsGameInfo");
    // Already what the installed game writes: nothing changes, to the byte.
    let b = convert(&s, Edition::Scholar, &path, Some(&s), same(None, Some(format(Edition::Scholar)))).unwrap();
    assert_eq!(b.changes, [Change::KeptTargetSystem { path: path.to_string_lossy().into_owned() }]);
    assert!(b.out.encode() == s.raw());
    // Another player's save from a newer game version gets this account and this game's data.
    let mut newer = format(Edition::Scholar);
    newer.regulation = regulation("02000200", 103, &[7; 0x200]);
    let foreign = with_account(&written_by(&newer, &testdata::save_blocks(&[(0, "Nele", 37)], 64)), OTHER);
    let b = convert(&foreign, Edition::Scholar, &path, Some(&foreign), same(None, Some(format(Edition::Scholar)))).unwrap();
    assert_eq!(account(&written(&b, Edition::Scholar)), Some(STEAM));
    assert!(b.changes.iter().any(|c| matches!(c, Change::Regulation { from: Some(f), .. } if f.stamp == 103)));
    // Moving a character between its own slots needs no install.
    let b = convert(&s, Edition::Scholar, &path, Some(&s), same(Some((0, 5)), None)).unwrap();
    assert_eq!((b.format.kind, name(&written(&b, Edition::Scholar), 5).as_deref()), ("targetFile", Some("Nele")));
}

/// Re-lays a real save's container with entry 21 at the other release's size and back, which
/// must reproduce the file byte for byte:
/// `DS2_RELAYOUT=<save.sl2> cargo test --lib -- --ignored relayout_real_file`
#[test]
#[ignore]
fn relayout_real_file() {
    let raw = std::fs::read(std::env::var_os("DS2_RELAYOUT").expect("DS2_RELAYOUT")).unwrap();
    let s = SaveFile::parse(raw.clone()).unwrap();
    let entry = s.block(regulation::ENTRY).to_vec();
    let other = if entry.len() == 2 * MB { MB } else { 2 * MB };
    let mut resized = s.clone();
    resized.set_block_data(regulation::ENTRY, &vec![0; other]);
    let mut back = SaveFile::parse(resized.encode()).unwrap();
    assert_eq!(back.block(regulation::ENTRY).len(), other);
    back.set_block_data(regulation::ENTRY, &entry);
    assert!(back.encode() == raw, "re-laying the container twice must give the game's own file back");
}

/// Runs a real conversion through `build` without writing anything but `DS2_BUILD_OUT`:
/// `DS2_BUILD_SRC=<save> DS2_BUILD_BASE=<save the target game wrote> DS2_GAME_DIR=<game> DS2_BUILD_OUT=<file>`
/// The result must match the base where the game decides: header, entry table, regulation,
/// title version and account.
#[test]
#[ignore]
fn build_real_files() {
    let var = |k: &str| std::env::var_os(k).map(PathBuf::from).unwrap_or_else(|| panic!("{k}"));
    let src = SaveFile::parse(std::fs::read(var("DS2_BUILD_SRC")).unwrap()).unwrap();
    let base_path = var("DS2_BUILD_BASE");
    let base = SaveFile::parse(std::fs::read(&base_path).unwrap()).unwrap();
    let edition = base.source_edition;
    let install = TargetFormat::from_install(edition, &var("DS2_GAME_DIR")).unwrap();
    let mut no_other = || None;
    let b = build(
        &src,
        &Target { slot: None, edition, path: &base_path, account: None, same_file: false },
        Some(&base),
        Formats { install: Some((Ok(install), "game".into())), other_save: &mut no_other },
    )
    .unwrap();
    for c in &b.changes {
        eprintln!("{c:?}");
    }
    let bytes = b.out.encode();
    let s = SaveFile::parse(bytes.clone()).unwrap();
    assert_eq!(&bytes[..0x40], &base.raw()[..0x40], "BND4 header");
    assert_eq!(bytes.len(), base.raw().len());
    for (a, e) in s.entries().iter().zip(base.entries()) {
        assert_eq!((a.offset, a.size), (e.offset, e.size), "entry {}", a.index);
    }
    assert_eq!(Regulation::from_entry(s.block(regulation::ENTRY)).unwrap(), Regulation::from_entry(base.block(regulation::ENTRY)).unwrap());
    assert_eq!(chunks::find(s.block(SUMMARY_ENTRY), header::TITLE_CHUNK), chunks::find(base.block(SUMMARY_ENTRY), header::TITLE_CHUNK));
    assert_eq!(account(&s), account(&base));
    assert!(s.checksum_failures().is_empty());
    for slot in 0..10 {
        assert_eq!(s.block(character_entry(slot)), src.block(character_entry(slot)), "slot {slot} carried over unchanged");
    }
    std::fs::write(var("DS2_BUILD_OUT"), bytes).unwrap();
}

#[test]
fn refuses_a_file_named_for_the_other_release() {
    let source = game_save(Edition::Original, &[(0, "Nele", 37)]);
    let o = || Opts { install: Some(format(Edition::Scholar)), ..Default::default() };
    // A Scholar save written under the original release's file name opens in neither game.
    let wrong = in_folder(STEAM, Edition::Original);
    let err = convert(&source, Edition::Scholar, &wrong, None, o()).err().expect("refused");
    assert_eq!(err.code, "nameEdition");
    // Any other name is the user's choice.
    let custom = Path::new("X:/elsewhere/backup.sl2");
    assert!(convert(&source, Edition::Scholar, custom, None, o()).is_ok());
}

#[test]
fn copies_the_games_own_regulation_entry_when_it_has_one() {
    let base = game_save(Edition::Scholar, &[(3, "Lucatiel", 30)]);
    let mut fmt = TargetFormat::from_save(&base).unwrap();
    // Mark the game's entry so a re-compressed one can't pass for it.
    let mut entry = fmt.entry.clone().unwrap();
    let last = entry.len() - 1;
    entry[last] = 0xA5;
    fmt.entry = Some(entry.clone());
    let source = game_save(Edition::Original, &[(0, "Nele", 37)]);
    let path = outside(Edition::Scholar);
    let b = convert(&source, Edition::Scholar, &path, None, Opts { other: Some(fmt), ..Default::default() }).unwrap();
    assert_eq!(written(&b, Edition::Scholar).block(regulation::ENTRY), &entry[..]);
}
