//! What changed in a character's inventory since the save was read or written: each 16-byte
//! record compared, by its place in the block, with the same record in the block as it is on
//! disk. Only bytes are compared, so a change made anywhere (the inventory tab, the JSON
//! document, the byte editor, a transfer) shows the same way.

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;

use super::inventory::{self, Item, KEYS, MAIN, RECORD_LEN};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    /// A record that was empty, or held another item, now holds this one.
    Added,
    /// The same item with another count, durability, reinforcement, infusion or charges (an
    /// item added to a stack already held counts here).
    Modified,
    /// A record that held an item is empty now.
    Removed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDiff {
    /// `quantity`, `durability`, `reinforcement`, `infusion`, `charges`, or `bytes` when only
    /// bytes the editor doesn't name differ.
    pub field: &'static str,
    pub from: Value,
    pub to: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordChange {
    pub offset: usize,
    pub kind: ChangeKind,
    /// What the record held on disk: the item a removed record held, the one an added item
    /// replaced (if any), the item a modified record held.
    pub before: Option<Item>,
    pub fields: Vec<FieldDiff>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryChanges {
    /// In record order.
    pub records: Vec<RecordChange>,
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
}

fn hex(bytes: &[u8]) -> Value {
    Value::String(bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" "))
}

fn json<T: Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// The record offsets of both inventory regions that fit in a block of `len` bytes.
pub fn record_offsets(len: usize) -> impl Iterator<Item = usize> {
    MAIN.step_by(RECORD_LEN).chain(KEYS.step_by(RECORD_LEN)).filter(move |o| o + RECORD_LEN <= len)
}

/// `saved` and `now` are the character block on disk and in memory.
pub fn compare(saved: &[u8], now: &[u8]) -> InventoryChanges {
    let by_offset = |block: &[u8]| -> HashMap<usize, Item> { inventory::read(block).items.into_iter().map(|i| (i.offset, i)).collect() };
    let (old, new) = (by_offset(saved), by_offset(now));
    let mut out = InventoryChanges::default();
    for offset in record_offsets(saved.len().min(now.len())) {
        let range = offset..offset + RECORD_LEN;
        if saved[range.clone()] == now[range.clone()] {
            continue;
        }
        let (kind, fields) = match (old.get(&offset), new.get(&offset)) {
            // Both empty (the id is 0): stray bytes in a free record are no item.
            (None, None) => continue,
            (None, Some(_)) => (ChangeKind::Added, Vec::new()),
            (Some(_), None) => (ChangeKind::Removed, Vec::new()),
            (Some(a), Some(b)) if a.id != b.id => (ChangeKind::Added, Vec::new()),
            (Some(a), Some(b)) => {
                let mut fields = Vec::new();
                let mut diff = |field, from: Value, to: Value| {
                    if from != to {
                        fields.push(FieldDiff { field, from, to });
                    }
                };
                diff("quantity", json(&a.quantity), json(&b.quantity));
                diff("durability", json(&a.durability), json(&b.durability));
                diff("reinforcement", json(&a.reinforcement), json(&b.reinforcement));
                diff("infusion", json(&a.infusion), json(&b.infusion));
                diff("charges", json(&a.charges), json(&b.charges));
                if fields.is_empty() {
                    fields.push(FieldDiff { field: "bytes", from: hex(&saved[range.clone()]), to: hex(&now[range]) });
                }
                (ChangeKind::Modified, fields)
            }
        };
        match kind {
            ChangeKind::Added => out.added += 1,
            ChangeKind::Modified => out.modified += 1,
            ChangeKind::Removed => out.removed += 1,
        }
        out.records.push(RecordChange { offset, kind, before: old.get(&offset).cloned(), fields });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::character::{self, CharacterEdit};
    use super::super::file::{character_entry, SaveFile};
    use super::super::inventory::InventoryOp;
    use super::super::raw;
    use super::super::testdata;
    use super::super::Edition;
    use super::*;

    const ESTUS: usize = MAIN.start;
    const LONGSWORD: usize = MAIN.start + 16;
    const SHARDS: usize = MAIN.start + 48;

    fn save() -> SaveFile {
        SaveFile::from_blocks(Edition::Scholar, &testdata::save_blocks(&[(0, "Nele", 37)], 64))
    }

    fn edit(s: &mut SaveFile, ops: Vec<InventoryOp>) {
        character::apply(s, 0, &CharacterEdit { inventory: ops, ..Default::default() }).unwrap();
    }

    fn changes(saved: &SaveFile, now: &SaveFile) -> InventoryChanges {
        compare(saved.block(character_entry(0)), now.block(character_entry(0)))
    }

    fn kinds(c: &InventoryChanges) -> Vec<(usize, ChangeKind)> {
        c.records.iter().map(|r| (r.offset, r.kind)).collect()
    }

    #[test]
    fn nothing_changed_is_clean() {
        let s = save();
        let c = changes(&s, &s);
        assert!(c.records.is_empty() && (c.added, c.modified, c.removed) == (0, 0, 0));
    }

    #[test]
    fn added_modified_removed() {
        let saved = save();
        let mut s = saved.clone();
        edit(
            &mut s,
            vec![
                InventoryOp::Remove { offset: ESTUS },
                InventoryOp::SetReinforcement { offset: LONGSWORD, level: 7 },
                InventoryOp::SetQuantity { offset: SHARDS, quantity: 25 },
                // The freed Estus record is the first free one: the dagger lands there.
                InventoryOp::Add { id: 1_000_000, quantity: 1, reinforcement: 3, infusion: 0 },
                InventoryOp::Add { id: 1_000_000, quantity: 1, reinforcement: 0, infusion: 0 },
            ],
        );
        let c = changes(&saved, &s);
        assert_eq!(
            kinds(&c),
            [(ESTUS, ChangeKind::Added), (LONGSWORD, ChangeKind::Modified), (MAIN.start + 32, ChangeKind::Added), (SHARDS, ChangeKind::Modified)]
        );
        assert_eq!((c.added, c.modified, c.removed), (2, 2, 0));
        // A dagger where the Estus Flask was replaces it; the flask is the record's "before".
        assert_eq!(c.records[0].before.as_ref().map(|i| i.name.as_str()), Some("Estus Flask"));
        let longsword = &c.records[1].fields;
        assert_eq!(longsword, &[FieldDiff { field: "reinforcement", from: json(&5), to: json(&7) }]);
        let shards = &c.records[3].fields;
        assert_eq!(shards, &[FieldDiff { field: "quantity", from: json(&Some(10)), to: json(&Some(25)) }]);
    }

    #[test]
    fn removed_and_stack_merge() {
        let saved = save();
        let mut s = saved.clone();
        edit(
            &mut s,
            vec![InventoryOp::Remove { offset: ESTUS }, InventoryOp::Add { id: 60_970_000, quantity: 5, reinforcement: 0, infusion: 0 }],
        );
        let c = changes(&saved, &s);
        assert_eq!(kinds(&c), [(ESTUS, ChangeKind::Removed), (SHARDS, ChangeKind::Modified)]);
        assert_eq!(c.records[0].before.as_ref().map(|i| i.id), Some(60_155_000));
        assert!(c.records[0].fields.is_empty());
        // Shards added to the stack already held: the same record, a larger count.
        assert_eq!(c.records[1].fields, [FieldDiff { field: "quantity", from: json(&Some(10)), to: json(&Some(15)) }]);
    }

    #[test]
    fn raw_writes_are_marked_too() {
        let saved = save();
        let mut s = saved.clone();
        let entry = character_entry(0);
        // Durability of the longsword, an unnamed byte of the shards' record, and an item id
        // typed into a free record.
        raw::write(&mut s, entry, LONGSWORD + 8, &10f32.to_le_bytes()).unwrap();
        raw::write(&mut s, entry, SHARDS + 5, &[0x77]).unwrap();
        raw::write(&mut s, entry, MAIN.start + 64, &1_000_000u32.to_le_bytes()).unwrap();
        // Bytes in a record that stays empty are no item.
        raw::write(&mut s, entry, MAIN.start + 80 + 9, &[1]).unwrap();
        let c = changes(&saved, &s);
        assert_eq!(kinds(&c), [(LONGSWORD, ChangeKind::Modified), (SHARDS, ChangeKind::Modified), (MAIN.start + 64, ChangeKind::Added)]);
        assert_eq!(c.records[0].fields[0].field, "durability");
        assert_eq!(c.records[1].fields[0].field, "bytes");
        assert_eq!(c.records[1].fields[0].from, Value::String("10 54 A2 03 00 00 00 00 0A 00 00 00 00 00 00 00".into()));
        assert_eq!(c.records[1].fields[0].to, Value::String("10 54 A2 03 00 77 00 00 0A 00 00 00 00 00 00 00".into()));
    }

    #[test]
    fn a_written_save_is_clean_again() {
        let saved = save();
        let mut s = saved.clone();
        edit(&mut s, vec![InventoryOp::SetQuantity { offset: SHARDS, quantity: 50 }]);
        assert_eq!(changes(&saved, &s).modified, 1);
        // After writing, the file on disk is what memory holds: compared against that, clean.
        let written = SaveFile::parse(s.encode()).unwrap();
        assert!(changes(&written, &s).records.is_empty());
    }
}
