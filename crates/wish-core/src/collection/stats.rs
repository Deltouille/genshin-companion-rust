//! An account's collection in figures, for the overview: its characters
//! (how many of the game's, by rarity, element and weapon; constellations,
//! levels and talents) and its weapons (how many of the game's, by rarity and
//! type, with their copies, refinements and holders).
//!
//! The game's characters are counted as the archive lists them: the Traveler
//! once per element. Nothing here is stored: it is worked out when shown.

use super::{Collection, MAX_CONSTELLATION, MAX_REFINEMENT, MAX_TALENT};
use crate::planner::{Level, PlannerData};

/// The elements, in the game's order.
pub const ELEMENTS: [&str; 7] = ["Pyro", "Hydro", "Anemo", "Electro", "Dendro", "Cryo", "Geo"];

/// The weapon types, in the game's order.
pub const WEAPON_TYPES: [&str; 5] = ["Sword", "Claymore", "Polearm", "Bow", "Catalyst"];

/// How many of a set the account owns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub owned: usize,
    pub total: usize,
}

impl Tally {
    fn add(&mut self, owned: bool) {
        self.total += 1;
        if owned {
            self.owned += 1;
        }
    }
}

/// The account's characters in figures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharacterStats {
    pub all: Tally,
    pub five: Tally,
    pub four: Tally,
    /// By element, in [`ELEMENTS`]' order.
    pub elements: Vec<(&'static str, Tally)>,
    /// By weapon, in [`WEAPON_TYPES`]' order.
    pub weapons: Vec<(&'static str, Tally)>,
    /// How many owned characters are at each constellation, C0 to C6.
    pub constellations: [usize; MAX_CONSTELLATION as usize + 1],
    /// Owned characters at the highest level.
    pub max_level: usize,
    /// Talents at the highest level, over every owned character.
    pub crowns: usize,
}

impl CharacterStats {
    /// The figures of `collection` against the game's characters. A
    /// character the game data lacks (a newer copy entered it) is left out.
    pub fn of(collection: &Collection, planner: &PlannerData) -> Self {
        let mut stats = Self {
            all: Tally::default(),
            five: Tally::default(),
            four: Tally::default(),
            elements: ELEMENTS.map(|e| (e, Tally::default())).to_vec(),
            weapons: WEAPON_TYPES.map(|w| (w, Tally::default())).to_vec(),
            constellations: [0; MAX_CONSTELLATION as usize + 1],
            max_level: 0,
            crowns: 0,
        };
        for character in planner.characters() {
            let owned = collection.character(&character.key);
            stats.all.add(owned.is_some());
            match character.rarity {
                5 => stats.five.add(owned.is_some()),
                4 => stats.four.add(owned.is_some()),
                _ => {}
            }
            if let Some((_, tally)) = stats
                .elements
                .iter_mut()
                .find(|(element, _)| *element == character.element)
            {
                tally.add(owned.is_some());
            }
            if let Some((_, tally)) = stats
                .weapons
                .iter_mut()
                .find(|(weapon, _)| *weapon == character.weapon)
            {
                tally.add(owned.is_some());
            }
            if let Some(owned) = owned {
                stats.constellations[owned.constellation.min(MAX_CONSTELLATION) as usize] += 1;
                if owned.level.level >= Level::MAX.level {
                    stats.max_level += 1;
                }
                stats.crowns += owned.talents.iter().filter(|&&t| t >= MAX_TALENT).count();
            }
        }
        stats
    }
}

/// One weapon the account owns, its copies together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedWeaponSummary {
    /// The planner's key.
    pub key: String,
    pub name: String,
    pub rarity: u8,
    pub copies: usize,
    /// The highest refinement among the copies.
    pub refinement: u8,
    /// The characters holding a copy (planner keys), in the copies' order.
    pub holders: Vec<String>,
}

/// The account's weapons in figures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedWeaponStats {
    /// Weapons owned (one however many copies), of the game's.
    pub all: Tally,
    pub five: Tally,
    pub four: Tally,
    /// By type, in [`WEAPON_TYPES`]' order.
    pub types: Vec<(&'static str, Tally)>,
    /// Copies, all weapons together.
    pub copies: usize,
    /// Copies at the highest refinement.
    pub refined_fully: usize,
    /// Copies a character holds.
    pub held: usize,
    /// Each weapon owned: the rarest first, then the most refined, then by
    /// name.
    pub weapons: Vec<OwnedWeaponSummary>,
}

impl OwnedWeaponStats {
    /// The figures of `collection`'s weapons against the game's. A weapon
    /// the game data doesn't have (a newer copy entered it) is left out.
    pub fn of(collection: &Collection, planner: &PlannerData) -> Self {
        let mut stats = Self {
            all: Tally::default(),
            five: Tally::default(),
            four: Tally::default(),
            types: WEAPON_TYPES.map(|t| (t, Tally::default())).to_vec(),
            copies: 0,
            refined_fully: 0,
            held: 0,
            weapons: Vec::new(),
        };
        for weapon in planner.weapons() {
            let copies: Vec<_> = collection.copies(&weapon.key).collect();
            let owned = !copies.is_empty();
            stats.all.add(owned);
            match weapon.rarity {
                5 => stats.five.add(owned),
                4 => stats.four.add(owned),
                _ => {}
            }
            if let Some((_, tally)) = stats
                .types
                .iter_mut()
                .find(|(kind, _)| *kind == weapon.weapon)
            {
                tally.add(owned);
            }
            if !owned {
                continue;
            }
            stats.copies += copies.len();
            stats.refined_fully += copies
                .iter()
                .filter(|c| c.refinement >= MAX_REFINEMENT)
                .count();
            stats.held += copies.iter().filter(|c| c.holder.is_some()).count();
            stats.weapons.push(OwnedWeaponSummary {
                key: weapon.key.clone(),
                name: weapon.name.clone(),
                rarity: weapon.rarity,
                copies: copies.len(),
                refinement: copies.iter().map(|c| c.refinement).max().unwrap_or(1),
                holders: copies.iter().filter_map(|c| c.holder.clone()).collect(),
            });
        }
        stats.weapons.sort_by(|a, b| {
            (b.rarity, b.refinement, &a.name).cmp(&(a.rarity, a.refinement, &b.name))
        });
        stats
    }

    /// The weapons owned of `rarity` stars.
    pub fn of_rarity(&self, rarity: u8) -> impl Iterator<Item = &OwnedWeaponSummary> {
        self.weapons.iter().filter(move |w| w.rarity == rarity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collection::{OwnedCharacter, OwnedWeapon};

    #[test]
    fn characters_are_counted_against_the_game_s() {
        let planner = PlannerData::embedded();
        let empty = CharacterStats::of(&Collection::default(), planner);
        assert_eq!(empty.all.owned, 0);
        assert_eq!(empty.all.total, planner.characters().len());
        assert_eq!(empty.five.total + empty.four.total, empty.all.total);

        let collection = Collection {
            characters: vec![
                OwnedCharacter {
                    level: Level::MAX,
                    constellation: 2,
                    talents: [10, 10, 9],
                    ..OwnedCharacter::new("ganyu")
                },
                OwnedCharacter::new("bennett"),
                // Not in the game data: left out.
                OwnedCharacter::new("nobody"),
            ],
            weapons: Vec::new(),
        };
        let stats = CharacterStats::of(&collection, planner);
        assert_eq!(stats.all.owned, 2);
        assert_eq!((stats.five.owned, stats.four.owned), (1, 1));
        let owned_of = |list: &[(&str, Tally)], name: &str| {
            list.iter().find(|(n, _)| *n == name).unwrap().1.owned
        };
        assert_eq!(owned_of(&stats.elements, "Cryo"), 1);
        assert_eq!(owned_of(&stats.elements, "Pyro"), 1);
        assert_eq!(owned_of(&stats.elements, "Geo"), 0);
        assert_eq!(owned_of(&stats.weapons, "Bow"), 1);
        assert_eq!(owned_of(&stats.weapons, "Sword"), 1);
        assert_eq!(stats.constellations, [1, 0, 1, 0, 0, 0, 0]);
        assert_eq!((stats.max_level, stats.crowns), (1, 2));
        // Every character has a known element and weapon.
        let sum = |list: &[(&str, Tally)]| list.iter().map(|(_, t)| t.total).sum::<usize>();
        assert_eq!(sum(&stats.elements), stats.all.total);
        assert_eq!(sum(&stats.weapons), stats.all.total);
    }

    #[test]
    fn weapons_are_counted_by_kind_with_their_copies() {
        let planner = PlannerData::embedded();
        let empty = OwnedWeaponStats::of(&Collection::default(), planner);
        assert_eq!(empty.all.owned, 0);
        assert_eq!(empty.all.total, planner.weapons().len());

        let copy = |key: &str, refinement, holder: Option<&str>| OwnedWeapon {
            refinement,
            holder: holder.map(str::to_owned),
            ..OwnedWeapon::new(key)
        };
        let collection = Collection {
            characters: Vec::new(),
            weapons: vec![
                copy("favoniussword", 3, Some("bennett")),
                copy("favoniussword", 5, None),
                copy("amosbow", 1, Some("ganyu")),
                // Not in the game data: left out.
                copy("nowhere", 1, None),
            ],
        };
        let stats = OwnedWeaponStats::of(&collection, planner);
        assert_eq!(
            (stats.all.owned, stats.five.owned, stats.four.owned),
            (2, 1, 1)
        );
        assert_eq!((stats.copies, stats.refined_fully, stats.held), (3, 1, 2));
        let owned_of = |kind: &str| {
            stats
                .types
                .iter()
                .find(|(k, _)| *k == kind)
                .unwrap()
                .1
                .owned
        };
        assert_eq!(
            (owned_of("Sword"), owned_of("Bow"), owned_of("Polearm")),
            (1, 1, 0)
        );
        let summary: Vec<(&str, usize, u8, Vec<String>)> = stats
            .weapons
            .iter()
            .map(|w| (w.key.as_str(), w.copies, w.refinement, w.holders.clone()))
            .collect();
        assert_eq!(
            summary,
            [
                ("amosbow", 1, 1, vec!["ganyu".to_owned()]),
                ("favoniussword", 2, 5, vec!["bennett".to_owned()]),
            ]
        );
    }
}
