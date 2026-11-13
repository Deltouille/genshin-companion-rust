//! What an account owns: its characters and weapons as the player entered
//! them (or a GOOD file or the wish history did, once confirmed). Its
//! materials are the planner's [`Inventory`](crate::planner::Inventory).
//!
//! Only what the player states is kept; stats and costs are worked out from
//! the game data when shown, never stored.

mod stats;
mod sync;

pub use stats::{
    CharacterStats, ELEMENTS, OwnedWeaponStats, OwnedWeaponSummary, Tally, WEAPON_TYPES,
};
pub use sync::{Change, SyncNote, SyncPlan, plan_sync};

use crate::planner::{CharacterGoal, Level, WeaponGoal};

/// The highest constellation.
pub const MAX_CONSTELLATION: u8 = 6;
/// The highest base talent level (constellations add 3 on top in the game).
pub const MAX_TALENT: u8 = 10;
/// The highest refinement.
pub const MAX_REFINEMENT: u8 = 5;

/// A character an account owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedCharacter {
    /// The planner's key (genshin-db's), such as `ganyu`.
    pub key: String,
    pub level: Level,
    /// 0 to [`MAX_CONSTELLATION`].
    pub constellation: u8,
    /// Normal attack, skill and burst, 1 to [`MAX_TALENT`], without the levels
    /// constellations add.
    pub talents: [u8; 3],
    pub favourite: bool,
}

impl OwnedCharacter {
    /// `key` as a new character arrives: level 1, C0, talents at 1.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            level: Level::START,
            constellation: 0,
            talents: [1; 3],
            favourite: false,
        }
    }

    /// The values brought back into the game's ranges.
    pub fn clamped(mut self) -> Self {
        self.level = Level::clamped(self.level.level, self.level.phase);
        self.constellation = self.constellation.min(MAX_CONSTELLATION);
        self.talents = self.talents.map(|t| t.clamp(1, MAX_TALENT));
        self
    }
}

/// One copy of a weapon an account owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedWeapon {
    /// The database's id; 0 for a copy not stored yet.
    pub id: i64,
    /// The planner's key, such as `amosbow`.
    pub key: String,
    pub level: Level,
    /// 1 to [`MAX_REFINEMENT`].
    pub refinement: u8,
    /// The character holding it (a planner key); one weapon each.
    pub holder: Option<String>,
    pub favourite: bool,
}

impl OwnedWeapon {
    /// A new copy of `key`: level 1, refinement 1, held by nobody.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            id: 0,
            key: key.into(),
            level: Level::START,
            refinement: 1,
            holder: None,
            favourite: false,
        }
    }

    /// The values brought back into the game's ranges.
    pub fn clamped(mut self) -> Self {
        self.level = Level::clamped(self.level.level, self.level.phase);
        self.refinement = self.refinement.clamp(1, MAX_REFINEMENT);
        self.holder = self.holder.filter(|key| !key.trim().is_empty());
        self
    }
}

/// Everything an account owns, as stored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Collection {
    /// By key.
    pub characters: Vec<OwnedCharacter>,
    /// In the order they were added.
    pub weapons: Vec<OwnedWeapon>,
}

impl Collection {
    pub fn character(&self, key: &str) -> Option<&OwnedCharacter> {
        self.characters.iter().find(|c| c.key == key)
    }

    /// The copies of weapon `key`.
    pub fn copies<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a OwnedWeapon> + 'a {
        self.weapons.iter().filter(move |w| w.key == key)
    }

    /// The weapon `character` holds.
    pub fn weapon_of(&self, character: &str) -> Option<&OwnedWeapon> {
        self.weapons
            .iter()
            .find(|w| w.holder.as_deref() == Some(character))
    }

    pub fn is_empty(&self) -> bool {
        self.characters.is_empty() && self.weapons.is_empty()
    }

    /// Where planning character `key` starts: the level and talents entered
    /// for it, up to 90/90 with its talents to 10; from level 1 when it isn't
    /// owned.
    pub fn character_goal(&self, key: &str) -> CharacterGoal {
        match self.character(key) {
            Some(owned) => CharacterGoal {
                from: owned.level,
                to: Level::MAX,
                talents: owned.talents.map(|t| (t, t.max(MAX_TALENT))),
            },
            None => CharacterGoal::FULL,
        }
    }

    /// Where planning weapon `key` (of `rarity` stars) starts: its most
    /// levelled copy that can still go up; from level 1 when none can (the
    /// plan is then for a new copy), up to its rarity's top.
    pub fn weapon_goal(&self, key: &str, rarity: u8) -> WeaponGoal {
        let full = WeaponGoal::full(rarity);
        let from = (self.copies(key))
            .map(|copy| copy.level)
            .filter(|level| *level < full.to)
            .max()
            .unwrap_or(full.from);
        WeaponGoal { from, to: full.to }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_out_of_range_are_brought_back() {
        let character = OwnedCharacter {
            constellation: 9,
            talents: [0, 12, 5],
            ..OwnedCharacter::new("ganyu")
        }
        .clamped();
        assert_eq!(character.constellation, 6);
        assert_eq!(character.talents, [1, 10, 5]);

        let weapon = OwnedWeapon {
            refinement: 0,
            holder: Some("  ".into()),
            ..OwnedWeapon::new("amosbow")
        }
        .clamped();
        assert_eq!(weapon.refinement, 1);
        assert_eq!(weapon.holder, None);
    }
}

#[cfg(test)]
mod goals {
    use super::*;

    #[test]
    fn planning_starts_from_what_is_owned() {
        let collection = Collection {
            characters: vec![OwnedCharacter {
                level: Level::new(80, 6).unwrap(),
                talents: [6, 9, 10],
                ..OwnedCharacter::new("ganyu")
            }],
            weapons: vec![
                OwnedWeapon {
                    level: Level::MAX,
                    ..OwnedWeapon::new("favoniussword")
                },
                OwnedWeapon {
                    level: Level::new(40, 1).unwrap(),
                    ..OwnedWeapon::new("favoniussword")
                },
                OwnedWeapon {
                    level: Level::MAX,
                    ..OwnedWeapon::new("amosbow")
                },
            ],
        };
        let ganyu = collection.character_goal("ganyu");
        assert_eq!(ganyu.from, Level::new(80, 6).unwrap());
        assert_eq!(ganyu.talents, [(6, 10), (9, 10), (10, 10)]);
        assert_eq!(collection.character_goal("amber"), CharacterGoal::FULL);

        // The copy that can still go up, not the one at 90.
        let sword = collection.weapon_goal("favoniussword", 4);
        assert_eq!(sword.from, Level::new(40, 1).unwrap());
        assert_eq!(sword.to, Level::MAX);
        // Every copy at the top, or none: a new copy, from level 1.
        assert_eq!(collection.weapon_goal("amosbow", 5), WeaponGoal::full(5));
        assert_eq!(collection.weapon_goal("thecatch", 4), WeaponGoal::full(4));
    }
}
