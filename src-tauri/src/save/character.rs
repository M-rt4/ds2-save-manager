//! The character block (entries 1-10). Offsets are absolute in the decrypted block and
//! identical in both releases; they come from the reference editors credited in NOTICE
//! and were checked against real saves (level = sum of attributes - 53 holds on all).

use serde::{Deserialize, Serialize};

use super::bytes::{put_u16, put_u32, put_utf16, u16_at, u32_at, u8_at, utf16_at};
use super::chunks;
use super::error::{Result, SaveError};
use super::file::{character_entry, SaveFile, HEADER_ENTRY, SUMMARY_ENTRY};
use super::header;
use super::inventory::{self, Inventory, InventoryOp};

pub mod off {
    pub const LEVEL: usize = 0x38;
    pub const SOULS: usize = 60;
    pub const SOUL_MEMORY: usize = 64;
    pub const HP: usize = 72;
    pub const DEATHS: usize = 104;
    pub const COVENANT: usize = 189;
    /// Discovered flags (one byte per covenant, in id order) start 2 past the current one,
    /// ranks 12 past it.
    pub const COVENANT_DISCOVERED: usize = COVENANT + 2;
    pub const COVENANT_RANK: usize = COVENANT + 12;
    pub const GENDER: usize = 378;
    pub const HOLLOWING: usize = 379;
    pub const NAME: usize = 960;
    pub const NAME_CHARS: usize = 16;
    pub const CLASS: usize = 1024;
    /// Journey number: 1 is the first playthrough, 2 is NG+, ...
    pub const JOURNEY: usize = 1028;
}

/// Attribute offsets (u16), in the game's own storage order. The menu shows them as
/// VGR END VIT ATN STR DEX ADP INT FTH.
pub const ATTR_OFFSETS: [usize; 9] = [32, 34, 36, 38, 40, 42, 44, 46, 48];
/// DS2's soul level identity: level = sum of the nine attributes - 53.
pub const LEVEL_OFFSET: i32 = 53;

/// The expected chunk table of a character block, `(type, size)`. Edits are refused on a
/// block that doesn't match, since every offset above assumes it.
const LAYOUT: [(u32, usize); 8] = [
    (0x0, 860),
    (0x1, 1136),
    (0x5, 4004),
    (0x6, 1564),
    (0xA, 65724),
    (0xC, 5012),
    (0xE, 1540),
    (0xD, 31196),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attributes {
    pub vigor: u16,
    pub endurance: u16,
    pub vitality: u16,
    pub attunement: u16,
    pub strength: u16,
    pub dexterity: u16,
    pub intelligence: u16,
    pub faith: u16,
    pub adaptability: u16,
}

impl Attributes {
    /// Values in storage order (matching `ATTR_OFFSETS`).
    pub fn to_array(self) -> [u16; 9] {
        [
            self.vigor,
            self.endurance,
            self.vitality,
            self.attunement,
            self.strength,
            self.dexterity,
            self.intelligence,
            self.faith,
            self.adaptability,
        ]
    }

    pub fn from_array(a: [u16; 9]) -> Self {
        Self {
            vigor: a[0],
            endurance: a[1],
            vitality: a[2],
            attunement: a[3],
            strength: a[4],
            dexterity: a[5],
            intelligence: a[6],
            faith: a[7],
            adaptability: a[8],
        }
    }

    pub fn level(self) -> i32 {
        self.to_array().iter().map(|&v| v as i32).sum::<i32>() - LEVEL_OFFSET
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Covenant {
    pub id: u8,
    pub discovered: bool,
    pub rank: u8,
}

/// What the slot list shows. `ghost` is a character still in its block whose menu entry
/// was deleted in game.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotSummary {
    pub slot: usize,
    pub occupied: bool,
    pub ghost: bool,
    pub name: String,
    pub level: u16,
    pub class: u8,
    pub journey: u16,
    pub soul_memory: u32,
    pub play_time: u32,
    pub map: u32,
    pub layout_ok: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    pub slot: usize,
    pub name: String,
    pub level: u16,
    pub attributes: Attributes,
    pub souls: u32,
    pub soul_memory: u32,
    pub hp: u32,
    pub deaths: u32,
    pub class: u8,
    pub covenant: u8,
    pub covenants: Vec<Covenant>,
    pub female: bool,
    pub hollowing: u8,
    pub journey: u16,
    pub play_time: u32,
    pub map: u32,
    pub inventory: Inventory,
    pub layout_ok: bool,
}

pub fn layout_ok(block: &[u8]) -> bool {
    chunks::walk(block).is_some_and(|c| {
        c.len() == LAYOUT.len() && c.iter().zip(LAYOUT).all(|(c, (kind, size))| c.kind == kind && c.size == size)
    })
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| !c.is_control() && c != '\u{fffd}')
}

fn attributes(block: &[u8]) -> Attributes {
    Attributes::from_array(ATTR_OFFSETS.map(|o| u16_at(block, o)))
}

pub fn summary(save: &SaveFile, slot: usize) -> SlotSummary {
    let block = save.block(character_entry(slot));
    let title = header::read_record(save.block(HEADER_ENTRY), slot).unwrap_or_default();
    let name_in_block = utf16_at(block, off::NAME, off::NAME_CHARS);
    let occupied = valid_name(&title.name);
    SlotSummary {
        slot,
        occupied,
        ghost: !occupied && valid_name(&name_in_block) && u16_at(block, off::LEVEL) > 0,
        name: if occupied { title.name } else { name_in_block },
        level: u16_at(block, off::LEVEL),
        class: u8_at(block, off::CLASS),
        journey: u16_at(block, off::JOURNEY).max(1),
        soul_memory: u32_at(block, off::SOUL_MEMORY),
        play_time: title.play_time,
        map: title.map,
        layout_ok: layout_ok(block),
    }
}

pub fn read(save: &SaveFile, slot: usize) -> Character {
    let block = save.block(character_entry(slot));
    let title = header::read_record(save.block(HEADER_ENTRY), slot).unwrap_or_default();
    let covenants = (1..=9u8)
        .map(|id| Covenant {
            id,
            discovered: u8_at(block, off::COVENANT_DISCOVERED + id as usize - 1) == 1,
            rank: u8_at(block, off::COVENANT_RANK + id as usize - 1).min(3),
        })
        .collect();
    Character {
        slot,
        name: utf16_at(block, off::NAME, off::NAME_CHARS),
        level: u16_at(block, off::LEVEL),
        attributes: attributes(block),
        souls: u32_at(block, off::SOULS),
        soul_memory: u32_at(block, off::SOUL_MEMORY),
        hp: u32_at(block, off::HP),
        deaths: u32_at(block, off::DEATHS),
        class: u8_at(block, off::CLASS),
        covenant: u8_at(block, off::COVENANT),
        covenants,
        female: u8_at(block, off::GENDER) == 1,
        hollowing: u8_at(block, off::HOLLOWING),
        journey: u16_at(block, off::JOURNEY).max(1),
        play_time: title.play_time,
        map: title.map,
        inventory: inventory::read(block),
        layout_ok: layout_ok(block),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterEdit {
    pub name: Option<String>,
    pub souls: Option<u32>,
    pub attributes: Option<Attributes>,
    pub journey: Option<u16>,
    #[serde(default)]
    pub inventory: Vec<InventoryOp>,
}

pub const MAX_SOULS: u32 = 999_999_999;

pub fn apply(save: &mut SaveFile, slot: usize, edit: &CharacterEdit) -> Result<()> {
    let entry = character_entry(slot);
    if !layout_ok(save.block(entry)) {
        return Err(SaveError::new("layout", "This character block has an unknown layout; it can't be edited."));
    }
    if !summary(save, slot).occupied {
        return Err(SaveError::invalid("That slot has no character."));
    }

    if let Some(name) = &edit.name {
        let name = name.trim();
        if !valid_name(name) || name.chars().count() > off::NAME_CHARS {
            return Err(SaveError::new("badName", "Names are 1 to 16 characters."));
        }
    }
    if let Some(a) = edit.attributes {
        if a.to_array().iter().any(|&v| !(1..=99).contains(&v)) {
            return Err(SaveError::new("badAttribute", "Attributes are between 1 and 99."));
        }
        if a.level() < 1 {
            return Err(SaveError::new("badAttribute", "Those attributes give a level below 1."));
        }
    }
    if edit.journey.is_some_and(|j| !(1..=99).contains(&j)) {
        return Err(SaveError::invalid("Journey is between 1 and 99."));
    }

    // Character block, edited on a copy so a failing inventory op leaves the save as it was.
    let mut work = save.block(entry).to_vec();
    let block = work.as_mut_slice();
    for op in &edit.inventory {
        inventory::apply(block, op)?;
    }
    if let Some(souls) = edit.souls {
        put_u32(block, off::SOULS, souls.min(MAX_SOULS));
    }
    if let Some(j) = edit.journey {
        put_u16(block, off::JOURNEY, j);
    }
    if let Some(name) = &edit.name {
        put_utf16(block, off::NAME, off::NAME_CHARS + 1, name.trim());
    }
    let mut attr_deltas = None;
    if let Some(a) = edit.attributes {
        let old = attributes(block).to_array();
        let new = a.to_array();
        for (o, v) in ATTR_OFFSETS.iter().zip(new) {
            put_u16(block, *o, v);
        }
        put_u16(block, off::LEVEL, a.level() as u16);
        let mut d = [0i32; 9];
        for i in 0..9 {
            d[i] = new[i] as i32 - old[i] as i32;
        }
        attr_deltas = Some((d, a.level() as u16));
    }
    save.block_mut(entry).copy_from_slice(&work);

    // Load Game menu: entry 0 and its copy in entry 22.
    for e in [HEADER_ENTRY, SUMMARY_ENTRY] {
        if e >= save.block_count() || header::record(save.block(e), slot).is_none() {
            continue;
        }
        let hb = save.block_mut(e);
        if let Some(name) = &edit.name {
            header::set_record_name(hb, slot, name.trim())?;
        }
        if let Some((d, level)) = attr_deltas {
            header::shift_record_attrs(hb, slot, &d)?;
            header::set_record_level(hb, slot, level)?;
        }
    }
    Ok(())
}
