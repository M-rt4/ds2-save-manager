//! The save as a readable JSON document: every field the editor knows, by name. A document
//! goes back into a save as a difference against the save's own document, through the same
//! checked writers the other tabs use, so a document exported and imported unchanged
//! changes no byte. Bytes the editor does not understand never appear in it and stay as
//! they are.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::bytes::put_u32;
use super::character::{self, off, Attributes, Character, CharacterEdit, MAX_SOULS};
use super::chunks;
use super::edition::Edition;
use super::error::{Result, SaveError};
use super::file::{character_entry, SaveFile, HEADER_ENTRY, SLOT_COUNT};
use super::header;
use super::inventory::{InventoryOp, Item, Region};
use super::items::{catalog, Category, INFUSIONS};
use super::maps;
use super::transfer;

pub const FORMAT: &str = "ds2-save-manager/1";
const COVENANTS: u8 = 9;
const MAX_REINFORCEMENT: u8 = 10;

/// Fields left out of an imported document are left as they are.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Document {
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edition: Option<Edition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regulation: Option<RegulationDoc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_slot: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub characters: Option<Vec<CharacterDoc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu: Option<Vec<MenuDoc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<BlockDoc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegulationDoc {
    pub version: String,
    pub stamp: u8,
    pub game_version: String,
    pub params: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterDoc {
    pub slot: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Follows the attributes (their sum minus 53).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Attributes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub souls: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soul_memory: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journey: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deaths: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hp: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub female: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hollowing: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub covenant: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub covenants: Option<Vec<CovenantDoc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inventory: Option<Vec<ItemDoc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CovenantDoc {
    pub id: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovered: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<u8>,
}

/// One inventory record. `record` (its place in the character block) names an item the
/// character has; an entry without it is a new item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemDoc {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<usize>,
    pub id: u32,
    /// For reading only; ignored on import.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reinforcement: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub infusion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durability: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charges: Option<(u8, u8)>,
    /// For reading only; ignored on import.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<Category>,
    /// For reading only; ignored on import.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuDoc {
    pub slot: usize,
    pub name: String,
    pub level: u16,
    pub class: u16,
    pub play_time: u32,
    pub map: u32,
    pub map_label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BlockDoc {
    pub index: usize,
    pub name: String,
    pub size: usize,
    pub chunks: Vec<ChunkDoc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChunkDoc {
    pub kind: u32,
    pub version: u32,
    pub offset: usize,
    pub size: usize,
}

fn item_doc(i: &Item) -> ItemDoc {
    ItemDoc {
        record: Some(i.offset),
        id: i.id,
        name: Some(i.name.clone()),
        quantity: i.quantity,
        reinforcement: Some(i.reinforcement),
        infusion: i.infusion.clone(),
        durability: i.durability,
        charges: i.charges,
        category: Some(i.category),
        region: Some(i.region),
    }
}

fn character_doc(c: &Character) -> CharacterDoc {
    CharacterDoc {
        slot: c.slot,
        name: Some(c.name.clone()),
        level: Some(c.level),
        attributes: Some(c.attributes),
        souls: Some(c.souls),
        soul_memory: Some(c.soul_memory),
        journey: Some(c.journey),
        deaths: Some(c.deaths),
        hp: Some(c.hp),
        class: Some(c.class),
        female: Some(c.female),
        hollowing: Some(c.hollowing),
        covenant: Some(c.covenant),
        covenants: Some(c.covenants.iter().map(|v| CovenantDoc { id: v.id, discovered: Some(v.discovered), rank: Some(v.rank) }).collect()),
        inventory: Some(c.inventory.items.iter().map(item_doc).collect()),
    }
}

fn characters(save: &SaveFile) -> Vec<Character> {
    (0..SLOT_COUNT).filter(|&s| character::summary(save, s).occupied).map(|s| character::read(save, s)).collect()
}

pub fn export(save: &SaveFile) -> Document {
    let menu_block = save.block(HEADER_ENTRY);
    Document {
        format: FORMAT.to_string(),
        edition: Some(save.source_edition),
        account: header::steam_id(menu_block).map(|a| a.to_string()),
        regulation: transfer::regulation_info(save).map(|r| RegulationDoc {
            version: r.version,
            stamp: r.stamp,
            game_version: r.game_version,
            params: r.params,
        }),
        last_slot: header::last_slot(menu_block),
        characters: Some(characters(save).iter().map(character_doc).collect()),
        menu: Some(
            (0..SLOT_COUNT)
                .filter_map(|slot| {
                    let r = header::read_record(menu_block, slot)?;
                    Some(MenuDoc { slot, map_label: maps::label(r.map), name: r.name, level: r.level, class: r.class, play_time: r.play_time, map: r.map })
                })
                .collect(),
        ),
        blocks: Some(
            save.entries()
                .iter()
                .map(|e| BlockDoc {
                    index: e.index,
                    name: e.name.clone(),
                    size: save.block(e.index).len(),
                    chunks: chunks::walk(save.block(e.index))
                        .unwrap_or_default()
                        .into_iter()
                        .map(|c| ChunkDoc { kind: c.kind, version: c.version, offset: c.offset, size: c.size })
                        .collect(),
                })
                .collect(),
        ),
    }
}

/// Where in a document something is.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    /// e.g. `characters[slot 2].inventory[record 7724].quantity`
    pub path: String,
    pub slot: Option<usize>,
    pub record: Option<usize>,
    /// The key the change is under, for finding its line.
    pub field: String,
}

impl Place {
    fn top(field: &str) -> Self {
        Self { path: field.to_string(), slot: None, record: None, field: field.to_string() }
    }

    fn character(slot: usize, field: &str) -> Self {
        Self { path: format!("characters[slot {slot}].{field}"), slot: Some(slot), record: None, field: field.to_string() }
    }

    fn item(slot: usize, record: Option<usize>, index: usize, field: &str) -> Self {
        let at = record.map_or(format!("new item {index}"), |r| format!("record {r}"));
        Self { path: format!("characters[slot {slot}].inventory[{at}].{field}"), slot: Some(slot), record, field: field.to_string() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
    #[serde(flatten)]
    pub place: Place,
    pub from: String,
    pub to: String,
    /// False for what the editor only shows; such a change is not written.
    pub applies: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    #[serde(flatten)]
    pub place: Place,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyntaxError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub syntax: Option<SyntaxError>,
    pub problems: Vec<Problem>,
    pub changes: Vec<FieldChange>,
}

pub fn parse(text: &str) -> std::result::Result<Document, SyntaxError> {
    serde_json::from_str(text).map_err(|e| SyntaxError { line: e.line(), column: e.column(), message: e.to_string() })
}

/// What a document asks for, against what the save holds.
pub fn preview(save: &SaveFile, text: &str) -> Preview {
    let doc = match parse(text) {
        Ok(d) => d,
        Err(e) => return Preview { syntax: Some(e), ..Default::default() },
    };
    let mut plan = plan(save, &doc);
    // The writers have the last word (a full inventory, say): try them on a copy.
    if plan.problems.is_empty() {
        if let Err(e) = write(&mut save.clone(), &plan.writes) {
            plan.problems.push(Problem { place: Place::top(""), message: e.message });
        }
    }
    Preview { syntax: None, problems: plan.problems, changes: plan.changes }
}

/// Writes what a document changes into `save`, all or nothing, and lists it.
pub fn apply(save: &mut SaveFile, text: &str) -> Result<Vec<FieldChange>> {
    let doc = parse(text).map_err(|e| SaveError::new("documentSyntax", format!("Line {}, column {}: {}", e.line, e.column, e.message)))?;
    let plan = plan(save, &doc);
    if let Some(p) = plan.problems.first() {
        return Err(SaveError::new("documentInvalid", format!("{}: {}", p.place.path, p.message)));
    }
    let mut work = save.clone();
    write(&mut work, &plan.writes)?;
    *save = work;
    Ok(plan.changes)
}

#[derive(Default)]
struct Extras {
    deaths: Option<u32>,
    covenant: Option<u8>,
    /// `(id, discovered, rank)`.
    covenants: Vec<(u8, Option<bool>, Option<u8>)>,
}

#[derive(Default)]
struct Writes {
    account: Option<u64>,
    characters: Vec<(usize, CharacterEdit, Extras)>,
}

#[derive(Default)]
struct Plan {
    changes: Vec<FieldChange>,
    problems: Vec<Problem>,
    writes: Writes,
}

impl Plan {
    fn change<T: Serialize>(&mut self, place: Place, from: &T, to: &T, applies: bool) {
        self.changes.push(FieldChange { place, from: json(from), to: json(to), applies });
    }

    fn problem(&mut self, place: Place, message: impl Into<String>) {
        self.problems.push(Problem { place, message: message.into() });
    }

    /// A field the editor only shows: noted when it differs, never written.
    fn shown<T: Serialize + PartialEq>(&mut self, place: Place, current: &T, edited: &Option<T>) {
        if let Some(v) = edited.as_ref().filter(|v| *v != current) {
            self.change(place, current, v, false);
        }
    }
}

fn json<T: Serialize>(v: &T) -> String {
    let s = serde_json::to_string(v).unwrap_or_default();
    if s.chars().count() > 120 {
        format!("{}…", s.chars().take(118).collect::<String>())
    } else {
        s
    }
}

fn plan(save: &SaveFile, doc: &Document) -> Plan {
    let mut p = Plan::default();
    let current = export(save);
    if doc.format != FORMAT {
        p.problem(Place::top("format"), format!("Not a DS2 Save Manager document; \"format\" must be \"{FORMAT}\"."));
        return p;
    }
    p.shown(Place::top("edition"), &current.edition.unwrap_or(save.source_edition), &doc.edition);
    p.shown(Place::top("regulation"), &current.regulation, &doc.regulation.clone().map(Some));
    p.shown(Place::top("lastSlot"), &current.last_slot, &doc.last_slot.map(Some));
    p.shown(Place::top("menu"), current.menu.as_ref().unwrap(), &doc.menu);
    p.shown(Place::top("blocks"), current.blocks.as_ref().unwrap(), &doc.blocks);

    if let Some(text) = doc.account.as_ref().filter(|a| Some(*a) != current.account.as_ref()) {
        match text.trim().parse::<u64>() {
            Ok(id) if id >> 32 == header::STEAM_ID_HIGH && id as u32 != 0 && current.account.is_some() => {
                p.writes.account = Some(id);
                p.change(Place::top("account"), &current.account, &Some(text.clone()), true);
            }
            _ if current.account.is_none() => p.problem(Place::top("account"), "This save has no readable account field."),
            _ => p.problem(Place::top("account"), "A Steam account id is a 17-digit number starting with 7656119."),
        }
    }

    if let Some(edited) = &doc.characters {
        let have = characters(save);
        let mut seen = HashSet::new();
        for c in edited {
            if !seen.insert(c.slot) {
                p.problem(Place::character(c.slot, "slot"), "This slot appears twice.");
            } else if !have.iter().any(|h| h.slot == c.slot) {
                p.problem(Place::character(c.slot, "slot"), format!("Slot {} has no character; characters can't be added here.", c.slot + 1));
            }
        }
        for h in &have {
            match edited.iter().find(|c| c.slot == h.slot) {
                Some(c) => plan_character(&mut p, h, c),
                None => p.problem(Place::character(h.slot, "slot"), "A character can't be removed here; put its entry back."),
            }
        }
    }
    p
}

fn plan_character(p: &mut Plan, cur: &Character, ed: &CharacterDoc) {
    let slot = cur.slot;
    let at = |field: &str| Place::character(slot, field);
    let mut edit = CharacterEdit::default();
    let mut extras = Extras::default();

    if let Some(name) = ed.name.as_ref().filter(|n| n.trim() != cur.name) {
        let n = name.trim();
        if n.is_empty() || n.chars().count() > off::NAME_CHARS || n.chars().any(char::is_control) {
            p.problem(at("name"), "Names are 1 to 16 characters.");
        } else {
            edit.name = Some(n.to_string());
            p.change(at("name"), &cur.name, &n.to_string(), true);
        }
    }
    if let Some(souls) = ed.souls.filter(|v| *v != cur.souls) {
        if souls > MAX_SOULS {
            p.problem(at("souls"), format!("Souls go up to {MAX_SOULS}."));
        } else {
            edit.souls = Some(souls);
            p.change(at("souls"), &cur.souls, &souls, true);
        }
    }
    let mut level = cur.level as i32;
    if let Some(a) = ed.attributes.filter(|a| *a != cur.attributes) {
        if a.to_array().iter().any(|v| !(1..=99).contains(v)) {
            p.problem(at("attributes"), "Attributes are between 1 and 99.");
        } else if a.level() < 1 {
            p.problem(at("attributes"), "Those attributes give a level below 1.");
        } else {
            level = a.level();
            edit.attributes = Some(a);
            p.change(at("attributes"), &cur.attributes, &a, true);
        }
    }
    // The level follows the attributes; a level that matches the new ones is no change.
    if let Some(l) = ed.level.filter(|l| *l != cur.level && *l as i32 != level) {
        p.change(at("level"), &cur.level, &l, false);
    }
    if let Some(j) = ed.journey.filter(|j| *j != cur.journey) {
        if !(1..=99).contains(&j) {
            p.problem(at("journey"), "Journey is between 1 and 99.");
        } else {
            edit.journey = Some(j);
            p.change(at("journey"), &cur.journey, &j, true);
        }
    }
    if let Some(d) = ed.deaths.filter(|d| *d != cur.deaths) {
        if d > MAX_SOULS {
            p.problem(at("deaths"), format!("Deaths go up to {MAX_SOULS}."));
        } else {
            extras.deaths = Some(d);
            p.change(at("deaths"), &cur.deaths, &d, true);
        }
    }
    plan_covenants(p, cur, ed, &mut extras);
    p.shown(at("soulMemory"), &cur.soul_memory, &ed.soul_memory);
    p.shown(at("hp"), &cur.hp, &ed.hp);
    p.shown(at("class"), &cur.class, &ed.class);
    p.shown(at("female"), &cur.female, &ed.female);
    p.shown(at("hollowing"), &cur.hollowing, &ed.hollowing);
    if let Some(items) = &ed.inventory {
        edit.inventory = plan_inventory(p, cur, items);
    }

    let edits_character = edit.name.is_some() || edit.souls.is_some() || edit.attributes.is_some() || edit.journey.is_some() || !edit.inventory.is_empty();
    if edits_character || extras.deaths.is_some() || extras.covenant.is_some() || !extras.covenants.is_empty() {
        if !cur.layout_ok {
            p.problem(at("slot"), "This character uses a layout the editor does not recognise; it can't be changed.");
        }
        p.writes.characters.push((slot, edit, extras));
    }
}

fn plan_covenants(p: &mut Plan, cur: &Character, ed: &CharacterDoc, extras: &mut Extras) {
    let slot = cur.slot;
    let mut discovered: HashMap<u8, bool> = cur.covenants.iter().map(|c| (c.id, c.discovered)).collect();
    if let Some(list) = &ed.covenants {
        let mut seen = HashSet::new();
        for c in list {
            let place = |field: &str| Place::character(slot, &format!("covenants[{}].{field}", c.id));
            let Some(now) = cur.covenants.iter().find(|v| v.id == c.id) else {
                p.problem(place("id"), format!("Covenants are numbered 1 to {COVENANTS}."));
                continue;
            };
            if !seen.insert(c.id) {
                p.problem(place("id"), "This covenant appears twice.");
                continue;
            }
            let joined = c.discovered.filter(|d| *d != now.discovered);
            let rank = c.rank.filter(|r| *r != now.rank);
            if let Some(d) = joined {
                discovered.insert(c.id, d);
                p.change(place("discovered"), &now.discovered, &d, true);
            }
            if let Some(r) = rank {
                if r > 3 {
                    p.problem(place("rank"), "Ranks are 0 to 3.");
                    continue;
                }
                p.change(place("rank"), &now.rank, &r, true);
            }
            if joined.is_some() || rank.is_some() {
                extras.covenants.push((c.id, joined, rank));
            }
        }
    }
    if let Some(c) = ed.covenant.filter(|c| *c != cur.covenant) {
        if c > COVENANTS {
            p.problem(Place::character(slot, "covenant"), format!("Covenants are numbered 1 to {COVENANTS} (0 is none)."));
        } else if c != 0 && !discovered.get(&c).copied().unwrap_or(false) {
            p.problem(Place::character(slot, "covenant"), format!("Covenant {c} hasn't been joined; set its \"discovered\" to true too."));
        } else {
            extras.covenant = Some(c);
            p.change(Place::character(slot, "covenant"), &cur.covenant, &c, true);
        }
    }
}

fn plan_inventory(p: &mut Plan, cur: &Character, items: &[ItemDoc]) -> Vec<InventoryOp> {
    let slot = cur.slot;
    let held: HashMap<usize, &Item> = cur.inventory.items.iter().map(|i| (i.offset, i)).collect();
    let (mut sets, mut removes, mut adds) = (Vec::new(), Vec::new(), Vec::new());
    let mut kept = HashSet::new();

    for (index, it) in items.iter().enumerate() {
        let at = |field: &str| Place::item(slot, it.record, index, field);
        let Some(record) = it.record else {
            if let Some(op) = plan_new_item(p, it, at) {
                adds.push(op);
            }
            continue;
        };
        let Some(now) = held.get(&record) else {
            p.problem(at("record"), format!("This character has no item at record {record}. Leave \"record\" out to add a new item."));
            continue;
        };
        if !kept.insert(record) {
            p.problem(at("record"), "This record appears twice.");
            continue;
        }
        if it.id != now.id {
            p.problem(at("id"), "An item's id can't change; remove the entry and add the other item.");
            continue;
        }
        if let Some(q) = it.quantity.filter(|q| Some(*q) != now.quantity) {
            match now.max_quantity {
                Some(max) if (1..=max).contains(&q) => {
                    sets.push(InventoryOp::SetQuantity { offset: record, quantity: q });
                    p.change(at("quantity"), &now.quantity, &Some(q), true);
                }
                Some(max) => p.problem(at("quantity"), format!("{} holds 1 to {max}.", now.name)),
                None => p.problem(at("quantity"), format!("The count of {} can't be changed.", now.name)),
            }
        }
        if let Some(r) = it.reinforcement.filter(|r| *r != now.reinforcement) {
            if !now.category.reinforceable() {
                p.problem(at("reinforcement"), format!("{} can't be reinforced.", now.name));
            } else if r > MAX_REINFORCEMENT {
                p.problem(at("reinforcement"), format!("Reinforcement goes up to +{MAX_REINFORCEMENT}."));
            } else {
                sets.push(InventoryOp::SetReinforcement { offset: record, level: r });
                p.change(at("reinforcement"), &now.reinforcement, &r, true);
            }
        }
        p.shown(at("infusion"), &now.infusion, &it.infusion.clone().map(Some));
        p.shown(at("durability"), &now.durability, &it.durability.map(Some));
        p.shown(at("charges"), &now.charges, &it.charges.map(Some));
    }
    let mut gone: Vec<&&Item> = held.values().filter(|i| !kept.contains(&i.offset)).collect();
    gone.sort_by_key(|i| i.offset);
    for i in gone {
        let at = Place::item(slot, Some(i.offset), 0, "record");
        if i.region == Region::Keys {
            p.problem(at, format!("{} is a key item or gesture; those can't be removed.", i.name));
        } else {
            removes.push(InventoryOp::Remove { offset: i.offset });
            p.change(at, &i.name, &"—".to_string(), true);
        }
    }
    sets.into_iter().chain(removes).chain(adds).collect()
}

fn plan_new_item(p: &mut Plan, it: &ItemDoc, at: impl Fn(&str) -> Place) -> Option<InventoryOp> {
    let Some(def) = catalog().get(it.id) else {
        p.problem(at("id"), format!("There is no item with id {}.", it.id));
        return None;
    };
    if !def.cat.addable() {
        p.problem(at("id"), format!("{} is a key item or gesture; those can't be added.", def.name));
        return None;
    }
    let quantity = it.quantity.unwrap_or(1);
    let max = if def.cat.stackable() { def.max_quantity() } else { 1 };
    if !(1..=max).contains(&quantity) {
        p.problem(at("quantity"), format!("{} holds 1 to {max}.", def.name));
        return None;
    }
    let reinforcement = it.reinforcement.unwrap_or(0);
    if reinforcement > 0 && !def.cat.reinforceable() {
        p.problem(at("reinforcement"), format!("{} can't be reinforced.", def.name));
        return None;
    }
    if reinforcement > MAX_REINFORCEMENT {
        p.problem(at("reinforcement"), format!("Reinforcement goes up to +{MAX_REINFORCEMENT}."));
        return None;
    }
    let infusion = match &it.infusion {
        None => 0,
        Some(name) => match INFUSIONS.iter().position(|i| i.eq_ignore_ascii_case(name.trim())) {
            Some(i) if i == 0 || matches!(def.cat, Category::Weapon | Category::Shield) => i as u8,
            Some(_) => {
                p.problem(at("infusion"), format!("{} can't be infused.", def.name));
                return None;
            }
            None => {
                p.problem(at("infusion"), format!("Infusions are {}.", INFUSIONS.join(", ")));
                return None;
            }
        },
    };
    let mut to = def.name.clone();
    if quantity > 1 {
        to += &format!(" ×{quantity}");
    }
    if reinforcement > 0 {
        to += &format!(" +{reinforcement}");
    }
    p.change(at("id"), &"—".to_string(), &to, true);
    Some(InventoryOp::Add { id: it.id, quantity, reinforcement, infusion })
}

fn write(save: &mut SaveFile, writes: &Writes) -> Result<()> {
    for (slot, edit, extras) in &writes.characters {
        let edits = edit.name.is_some() || edit.souls.is_some() || edit.attributes.is_some() || edit.journey.is_some() || !edit.inventory.is_empty();
        if edits {
            character::apply(save, *slot, edit)?;
        }
        write_extras(save, *slot, extras)?;
    }
    if let Some(id) = writes.account {
        header::set_steam_id(save.block_mut(HEADER_ENTRY), id)?;
    }
    Ok(())
}

/// Deaths and covenants: fixed offsets in the character block's first chunk, written only
/// into a block whose layout is the known one.
fn write_extras(save: &mut SaveFile, slot: usize, x: &Extras) -> Result<()> {
    if x.deaths.is_none() && x.covenant.is_none() && x.covenants.is_empty() {
        return Ok(());
    }
    let entry = character_entry(slot);
    if !character::layout_ok(save.block(entry)) {
        return Err(SaveError::new("layout", "This character block has an unknown layout; it can't be edited."));
    }
    let block = save.block_mut(entry);
    if let Some(d) = x.deaths {
        put_u32(block, off::DEATHS, d);
    }
    for &(id, discovered, rank) in &x.covenants {
        let i = (id - 1) as usize;
        if let Some(d) = discovered {
            block[off::COVENANT_DISCOVERED + i] = d as u8;
        }
        if let Some(r) = rank {
            block[off::COVENANT_RANK + i] = r;
        }
    }
    if let Some(c) = x.covenant {
        block[off::COVENANT] = c;
    }
    Ok(())
}

/// Pretty JSON that keeps each small record (an item, a covenant, the attributes) on one
/// line, so a character's inventory reads as a list.
pub fn to_text(doc: &Document) -> String {
    let compact = serde_json::to_string(doc).expect("a document serializes");
    let mut out = String::new();
    let node = Node::parse(&mut compact.as_bytes(), &mut 0);
    node.print(&mut out, 0);
    out.push('\n');
    out
}

/// A JSON value with its members in their original order (serde_json's own `Value` sorts
/// keys), parsed from serde_json's compact output.
enum Node {
    Scalar(String),
    Array(Vec<Node>),
    Object(Vec<(String, Node)>),
}

impl Node {
    fn parse(s: &mut &[u8], i: &mut usize) -> Node {
        match s[*i] {
            b'{' => {
                *i += 1;
                let mut members = Vec::new();
                while s[*i] != b'}' {
                    if s[*i] == b',' {
                        *i += 1;
                    }
                    let key = Self::string(s, i);
                    *i += 1; // ':'
                    members.push((key, Self::parse(s, i)));
                }
                *i += 1;
                Node::Object(members)
            }
            b'[' => {
                *i += 1;
                let mut items = Vec::new();
                while s[*i] != b']' {
                    if s[*i] == b',' {
                        *i += 1;
                    }
                    items.push(Self::parse(s, i));
                }
                *i += 1;
                Node::Array(items)
            }
            b'"' => Node::Scalar(Self::string(s, i)),
            _ => {
                let start = *i;
                while !matches!(s[*i], b',' | b'}' | b']') {
                    *i += 1;
                }
                Node::Scalar(String::from_utf8_lossy(&s[start..*i]).into_owned())
            }
        }
    }

    /// A string token, quotes and escapes kept as they are.
    fn string(s: &[u8], i: &mut usize) -> String {
        let start = *i;
        *i += 1;
        while s[*i] != b'"' {
            if s[*i] == b'\\' {
                *i += 1;
            }
            *i += 1;
        }
        *i += 1;
        String::from_utf8_lossy(&s[start..*i]).into_owned()
    }

    fn scalar(&self) -> bool {
        matches!(self, Node::Scalar(_))
    }

    /// Arrays of scalars, and objects holding only scalars and such arrays, fit on a line.
    fn one_line(&self) -> bool {
        match self {
            Node::Scalar(_) => true,
            Node::Array(items) => items.iter().all(Node::scalar),
            Node::Object(members) => members.iter().all(|(_, v)| v.scalar() || matches!(v, Node::Array(a) if a.iter().all(Node::scalar))),
        }
    }

    fn inline(&self, out: &mut String) {
        match self {
            Node::Scalar(s) => out.push_str(s),
            Node::Array(items) => {
                out.push('[');
                for (n, v) in items.iter().enumerate() {
                    if n > 0 {
                        out.push_str(", ");
                    }
                    v.inline(out);
                }
                out.push(']');
            }
            Node::Object(members) => {
                out.push_str(if members.is_empty() { "{" } else { "{ " });
                for (n, (k, v)) in members.iter().enumerate() {
                    if n > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(k);
                    out.push_str(": ");
                    v.inline(out);
                }
                out.push_str(if members.is_empty() { "}" } else { " }" });
            }
        }
    }

    fn print(&self, out: &mut String, depth: usize) {
        if depth > 0 && self.one_line() {
            return self.inline(out);
        }
        let pad = "  ".repeat(depth + 1);
        match self {
            Node::Scalar(s) => out.push_str(s),
            Node::Array(items) if items.is_empty() => out.push_str("[]"),
            Node::Object(members) if members.is_empty() => out.push_str("{}"),
            Node::Array(items) => {
                out.push('[');
                for (n, v) in items.iter().enumerate() {
                    out.push_str(if n > 0 { ",\n" } else { "\n" });
                    out.push_str(&pad);
                    v.print(out, depth + 1);
                }
                out.push('\n');
                out.push_str(&"  ".repeat(depth));
                out.push(']');
            }
            Node::Object(members) => {
                out.push('{');
                for (n, (k, v)) in members.iter().enumerate() {
                    out.push_str(if n > 0 { ",\n" } else { "\n" });
                    out.push_str(&pad);
                    out.push_str(k);
                    out.push_str(": ");
                    v.print(out, depth + 1);
                }
                out.push('\n');
                out.push_str(&"  ".repeat(depth));
                out.push('}');
            }
        }
    }
}
