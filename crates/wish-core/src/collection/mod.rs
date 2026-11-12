//! What an account owns: its characters and weapons as the player entered
//! them (or a GOOD file or the wish history did, once confirmed). Its
//! materials are the planner's [`Inventory`](crate::planner::Inventory).
//!
//! Only what the player states is kept; stats and costs are worked out from
//! the game data when shown, never stored.

mod stats;

pub use stats::{CharacterStats, ELEMENTS, Tally, WEAPON_TYPES, WishedWeapon, WishedWeapons};

use crate::planner::Level;

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
