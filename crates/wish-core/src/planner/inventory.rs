//! What the player has, as they entered it, one material at a time. The
//! planner sets it against what the plan needs.

use std::collections::BTreeMap;

use super::{CustomItem, MaterialId};

/// The player's materials: those the game data knows by id, the others by
/// the name a plan entry gives them. A material they have none of is absent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    pub materials: BTreeMap<MaterialId, u64>,
    pub named: BTreeMap<String, u64>,
}

impl Inventory {
    /// How many of `item` the player has.
    pub fn owned(&self, item: &CustomItem) -> u64 {
        match item {
            CustomItem::Material(id) => self.materials.get(id).copied(),
            CustomItem::Named(name) => self.named.get(name.trim()).copied(),
        }
        .unwrap_or(0)
    }

    /// Records that the player has `count` of `item`.
    pub fn set(&mut self, item: &CustomItem, count: u64) {
        match item {
            CustomItem::Material(id) if count == 0 => {
                self.materials.remove(id);
            }
            CustomItem::Material(id) => {
                self.materials.insert(*id, count);
            }
            CustomItem::Named(name) if count == 0 => {
                self.named.remove(name.trim());
            }
            CustomItem::Named(name) => {
                self.named.insert(name.trim().to_owned(), count);
            }
        }
    }
}
