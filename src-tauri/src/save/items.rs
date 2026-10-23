//! Item names, from the db_ds2 tables of darthdemono/sl2-analyzer (Apache-2.0, see
//! NOTICE), flattened into data/items.json by scripts/build-item-db.mjs. Both releases
//! share one id space; Scholar-only items simply never appear in an original save.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Weapon,
    Shield,
    Armor,
    Ring,
    Spell,
    Consumable,
    Multiplayer,
    Upgrade,
    Ammo,
    BossSoul,
    Key,
    Gesture,
    Unknown,
}

impl Category {
    /// Records of these categories store a count at +8; the rest store durability.
    pub fn stackable(self) -> bool {
        matches!(
            self,
            Category::Consumable
                | Category::Multiplayer
                | Category::Upgrade
                | Category::Ammo
                | Category::BossSoul
                | Category::Key
                | Category::Spell
        )
    }

    pub fn reinforceable(self) -> bool {
        matches!(self, Category::Weapon | Category::Shield | Category::Armor)
    }

    /// Largest stack the editor will write.
    pub fn max_stack(self) -> u16 {
        match self {
            Category::Ammo => 999,
            Category::Spell | Category::Key | Category::Gesture => 1,
            _ => 99,
        }
    }

    /// Whether the editor may add new records of this category. Key items and gestures
    /// live in a separate region whose rules aren't mapped, so they are read-only.
    pub fn addable(self) -> bool {
        !matches!(self, Category::Key | Category::Gesture | Category::Unknown)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDef {
    pub id: u32,
    pub name: String,
    pub cat: Category,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
}

pub struct Catalog {
    pub items: Vec<ItemDef>,
    by_id: HashMap<u32, usize>,
}

impl Catalog {
    pub fn get(&self, id: u32) -> Option<&ItemDef> {
        self.by_id.get(&id).map(|&i| &self.items[i])
    }
}

pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let items: Vec<ItemDef> =
            serde_json::from_str(include_str!("../../data/items.json")).expect("data/items.json is valid");
        let by_id = items.iter().enumerate().map(|(i, it)| (it.id, i)).collect();
        Catalog { items, by_id }
    })
}

pub const INFUSIONS: [&str; 10] =
    ["", "Fire", "Magic", "Lightning", "Dark", "Poison", "Bleed", "Raw", "Enchanted", "Mundane"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_loads_and_resolves() {
        let c = catalog();
        assert!(c.items.len() > 1000);
        assert_eq!(c.get(1_000_000).unwrap().name, "Dagger");
        assert_eq!(c.get(21_010_100).unwrap().cat, Category::Armor);
    }
}
