//! The inventory lives in the character block's chunk 0xA as fixed 16-byte records:
//! `[u32 item id][u32 0][+8 count or f32 durability][+12 reinforcement][+13 infusion]`.
//! Key items and gestures sit in a second, smaller region of the same chunk. Records are
//! not compacted: an emptied record (all zero) is simply skipped by the game.

use std::ops::Range;

use serde::{Deserialize, Serialize};

use super::bytes::{f32_at, put_f32, put_u16, u16_at, u32_at, u8_at};
use super::error::{Result, SaveError};
use super::items::{catalog, Category, INFUSIONS};

pub const RECORD_LEN: usize = 16;
pub const MAIN: Range<usize> = 0x1E2C..0x10E1C;
pub const KEYS: Range<usize> = 0x10E30..0x11DF0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Region {
    Main,
    Keys,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    /// Offset of the record in the character block; the handle edits refer to.
    pub offset: usize,
    pub region: Region,
    pub id: u32,
    pub name: String,
    pub category: Category,
    pub armor_slot: Option<String>,
    pub quantity: Option<u16>,
    pub durability: Option<f32>,
    pub reinforcement: u8,
    pub infusion: Option<String>,
    /// Estus Flask charges: (current, max).
    pub charges: Option<(u8, u8)>,
    /// The game's icon id, served as `/icons/<icon>.webp`.
    pub icon: Option<u32>,
    /// Set when the count can be edited: the most the player can hold.
    pub max_quantity: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub items: Vec<Item>,
    pub free_main: usize,
    pub free_keys: usize,
    pub unknown: usize,
}

pub fn read(block: &[u8]) -> Inventory {
    let mut items = Vec::new();
    let (mut free_main, mut free_keys, mut unknown) = (0, 0, 0);
    for (region, range) in [(Region::Main, MAIN), (Region::Keys, KEYS)] {
        for offset in range.step_by(RECORD_LEN) {
            if offset + RECORD_LEN > block.len() {
                break;
            }
            let id = u32_at(block, offset);
            if id == 0 {
                match region {
                    Region::Main => free_main += 1,
                    Region::Keys => free_keys += 1,
                }
                continue;
            }
            let def = catalog().get(id);
            if def.is_none() {
                unknown += 1;
            }
            let category = def.map_or(Category::Unknown, |d| d.cat);
            let name = def.map_or_else(|| format!("#{id}"), |d| d.name.clone());
            let stack = category.stackable() || category == Category::Gesture;
            let infusion = u8_at(block, offset + 13);
            let charges = (name == "Estus Flask")
                .then(|| (u8_at(block, offset + 10), u8_at(block, offset + 11)))
                .filter(|&(_, max)| max > 0);
            items.push(Item {
                offset,
                region,
                id,
                name,
                category,
                armor_slot: def.and_then(|d| d.slot.clone()),
                quantity: (stack || category == Category::Unknown).then(|| u16_at(block, offset + 8)),
                durability: (!stack && category != Category::Unknown).then(|| f32_at(block, offset + 8)),
                reinforcement: if category.reinforceable() { u8_at(block, offset + 12) } else { 0 },
                infusion: (matches!(category, Category::Weapon | Category::Shield) && infusion != 0)
                    .then(|| INFUSIONS.get(infusion as usize).map_or(format!("#{infusion}"), |s| s.to_string())),
                charges,
                icon: def.and_then(|d| d.icon),
                max_quantity: def
                    .filter(|d| region == Region::Main && stack && d.max_quantity() > 1)
                    .map(|d| d.max_quantity()),
            });
        }
    }
    Inventory { items, free_main, free_keys, unknown }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum InventoryOp {
    #[serde(rename_all = "camelCase")]
    SetQuantity { offset: usize, quantity: u16 },
    #[serde(rename_all = "camelCase")]
    SetReinforcement { offset: usize, level: u8 },
    #[serde(rename_all = "camelCase")]
    Remove { offset: usize },
    #[serde(rename_all = "camelCase")]
    Add { id: u32, quantity: u16, reinforcement: u8, infusion: u8 },
}

pub fn apply(block: &mut [u8], op: &InventoryOp) -> Result<()> {
    match *op {
        InventoryOp::SetQuantity { offset, quantity } => {
            let (cat, region, max) = existing(block, offset)?;
            if !cat.stackable() || region == Region::Keys || max <= 1 {
                return Err(SaveError::invalid("This item has no count."));
            }
            put_u16(block, offset + 8, quantity.clamp(1, max));
        }
        InventoryOp::SetReinforcement { offset, level } => {
            let (cat, _, _) = existing(block, offset)?;
            if !cat.reinforceable() {
                return Err(SaveError::invalid("This item can't be reinforced."));
            }
            block[offset + 12] = level.min(10);
        }
        InventoryOp::Remove { offset } => {
            if existing(block, offset)?.1 == Region::Keys {
                return Err(SaveError::invalid("Key items and gestures can't be removed."));
            }
            block[offset..offset + RECORD_LEN].fill(0);
        }
        InventoryOp::Add { id, quantity, reinforcement, infusion } => add(block, id, quantity, reinforcement, infusion)?,
    }
    Ok(())
}

/// The category, region and count limit of the record at `offset`.
fn existing(block: &[u8], offset: usize) -> Result<(Category, Region, u16)> {
    let in_region = |r: &Range<usize>| r.contains(&offset) && (offset - r.start) % RECORD_LEN == 0;
    let region = if in_region(&MAIN) {
        Region::Main
    } else if in_region(&KEYS) {
        Region::Keys
    } else {
        return Err(SaveError::invalid("No inventory record at that position."));
    };
    if offset + RECORD_LEN > block.len() {
        return Err(SaveError::invalid("No inventory record at that position."));
    }
    let id = u32_at(block, offset);
    if id == 0 {
        return Err(SaveError::invalid("That inventory record is empty."));
    }
    let def = catalog().get(id);
    Ok((def.map_or(Category::Unknown, |d| d.cat), region, def.map_or(0, |d| d.max_quantity())))
}

fn add(block: &mut [u8], id: u32, quantity: u16, reinforcement: u8, infusion: u8) -> Result<()> {
    let def = catalog().get(id).ok_or_else(|| SaveError::invalid(format!("Unknown item id {id}.")))?;
    let cat = def.cat;
    if !cat.addable() {
        return Err(SaveError::invalid("Key items and gestures can't be added."));
    }
    let records = || MAIN.step_by(RECORD_LEN).filter(|o| o + RECORD_LEN <= block.len());

    if cat.stackable() {
        if let Some(o) = records().find(|&o| u32_at(block, o) == id) {
            let q = u16_at(block, o + 8).saturating_add(quantity).clamp(1, def.max_quantity());
            put_u16(block, o + 8, q);
            return Ok(());
        }
    }
    let free = records()
        .find(|&o| block[o..o + RECORD_LEN].iter().all(|&b| b == 0))
        .ok_or_else(|| SaveError::new("inventoryFull", "The inventory has no free record."))?;
    // An item of the same kind already held tells the right durability; otherwise use the
    // defaults the reference editor ships with. The game clamps durability to the item's
    // maximum when it loads the record.
    let durability = records()
        .find(|&o| u32_at(block, o) == id)
        .or_else(|| records().find(|&o| catalog().get(u32_at(block, o)).is_some_and(|d| d.cat == cat)))
        .map(|o| f32_at(block, o + 8))
        .unwrap_or(match cat {
            Category::Armor => 255.0,
            Category::Ring => 120.0,
            _ => 40.0,
        });

    let rec = &mut block[free..free + RECORD_LEN];
    rec.fill(0);
    rec[0..4].copy_from_slice(&id.to_le_bytes());
    if cat.stackable() {
        put_u16(rec, 8, quantity.clamp(1, def.max_quantity()));
    } else {
        put_f32(rec, 8, durability);
    }
    if cat.reinforceable() {
        rec[12] = reinforcement.min(10);
    }
    if matches!(cat, Category::Weapon | Category::Shield) && (infusion as usize) < INFUSIONS.len() {
        rec[13] = infusion;
    }
    Ok(())
}
