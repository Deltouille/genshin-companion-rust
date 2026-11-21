//! An account's collection in figures, for the overview: its characters
//! (how many of the game's, by rarity, element, weapon, region and version;
//! constellations, levels and talents; what raising them cost) and its weapons
//! (how many of the game's, by rarity and type, with their copies, refinements
//! and holders).
//!
//! The game's characters are counted as the archive lists them: the Traveler
//! once per element. Nothing here is stored: it is worked out when shown.

use std::collections::BTreeMap;

use super::{Collection, MAX_CONSTELLATION, MAX_REFINEMENT, MAX_TALENT};
use crate::archive::ArchiveData;
use crate::i18n::Text;
use crate::planner::{CharacterGoal, Level, MaterialId, PlannerData, Requirement, Role};

/// The elements, in the game's order.
pub const ELEMENTS: [&str; 7] = ["Pyro", "Hydro", "Anemo", "Electro", "Dendro", "Cryo", "Geo"];

/// The weapon types, in the game's order.
pub const WEAPON_TYPES: [&str; 5] = ["Sword", "Claymore", "Polearm", "Bow", "Catalyst"];

/// The regions, in the order the story reaches them, by their English names.
pub const REGIONS: [&str; 8] = [
    "Mondstadt",
    "Liyue",
    "Inazuma",
    "Sumeru",
    "Fontaine",
    "Natlan",
    "Nod-Krai",
    "Snezhnaya",
];

/// The brackets of [`CharacterStats::levels`], each by its lowest level: 90,
/// 80 to 89, 70 to 79, and below 70.
pub const LEVEL_BRACKETS: [u8; 4] = [90, 80, 70, 1];

/// The brackets of [`CharacterStats::talents`], each by its lowest level: 10,
/// 9, 8, 6 and 7, and 1 to 5.
pub const TALENT_BRACKETS: [u8; 5] = [10, 9, 8, 6, 1];

/// The bracket `value` falls in, of `brackets` (each by its lowest value,
/// highest first).
fn bracket(brackets: &[u8], value: u8) -> usize {
    (brackets.iter().position(|&lowest| value >= lowest)).unwrap_or(brackets.len() - 1)
}

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

/// The characters released up to a game version, and how many of them the
/// account owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionTally {
    /// "4.7".
    pub version: String,
    /// Released in this version or before.
    pub released: usize,
    /// Of those, owned.
    pub owned: usize,
}

/// Owned characters at each constellation, C0 to C6.
pub type Constellations = [usize; MAX_CONSTELLATION as usize + 1];

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
    /// By region, in [`REGIONS`]' order, then a region the list doesn't
    /// know yet, then `None`: the characters of no region (the Traveler, Aloy).
    pub regions: Vec<(Option<Text>, Tally)>,
    /// Owned 5★ characters at each constellation.
    pub five_constellations: Constellations,
    /// Owned 4★ characters at each constellation.
    pub four_constellations: Constellations,
    /// Owned characters in each of [`LEVEL_BRACKETS`].
    pub levels: [usize; LEVEL_BRACKETS.len()],
    /// The talents of every owned character in each of [`TALENT_BRACKETS`].
    pub talents: [usize; TALENT_BRACKETS.len()],
    /// The levels of each talent (normal attack, skill, burst) added up over
    /// the owned characters, for [`CharacterStats::average_talent`].
    talent_totals: [usize; 3],
    /// Each version in which characters came out, oldest first: how many
    /// were out by then, and owned among them.
    pub by_version: Vec<VersionTally>,
    /// What raising the owned characters to where they are cost, from level
    /// 1 with talents at 1: Mora included, the EXP as Hero's Wit.
    pub invested: BTreeMap<MaterialId, u64>,
}

impl CharacterStats {
    /// The figures of `collection` against the game's characters, with the
    /// archive's regions and versions. A character the game data lacks (a
    /// newer copy entered it) is left out.
    pub fn of(collection: &Collection, planner: &PlannerData, archive: &ArchiveData) -> Self {
        let mut stats = Self {
            all: Tally::default(),
            five: Tally::default(),
            four: Tally::default(),
            elements: ELEMENTS.map(|e| (e, Tally::default())).to_vec(),
            weapons: WEAPON_TYPES.map(|w| (w, Tally::default())).to_vec(),
            regions: Vec::new(),
            five_constellations: Constellations::default(),
            four_constellations: Constellations::default(),
            levels: [0; LEVEL_BRACKETS.len()],
            talents: [0; TALENT_BRACKETS.len()],
            talent_totals: [0; 3],
            by_version: Vec::new(),
            invested: BTreeMap::new(),
        };
        let mut regions: Vec<(Option<Text>, Tally)> = Vec::new();
        let mut versions: Vec<((u32, u32), &str, bool)> = Vec::new();
        let mut invested = Requirement::default();
        // The Traveler's level is one for every element: counted once.
        let mut traveler_level: Option<(Level, &crate::planner::Character)> = None;
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
            let page = archive.character(&character.key);
            let region = page.and_then(|page| page.region.clone());
            match regions.iter_mut().find(|(r, _)| *r == region) {
                Some((_, tally)) => tally.add(owned.is_some()),
                None => {
                    let mut tally = Tally::default();
                    tally.add(owned.is_some());
                    regions.push((region, tally));
                }
            }
            if let Some(version) = page.and_then(|page| page.version.as_deref()) {
                versions.push((version_order(version), version, owned.is_some()));
            }
            let Some(owned) = owned else {
                continue;
            };
            let constellation = owned.constellation.min(MAX_CONSTELLATION) as usize;
            match character.rarity {
                5 => stats.five_constellations[constellation] += 1,
                4 => stats.four_constellations[constellation] += 1,
                _ => {}
            }
            stats.levels[bracket(&LEVEL_BRACKETS, owned.level.level)] += 1;
            for (slot, &talent) in owned.talents.iter().enumerate() {
                let talent = talent.min(MAX_TALENT);
                stats.talents[bracket(&TALENT_BRACKETS, talent)] += 1;
                stats.talent_totals[slot] += usize::from(talent);
            }
            let mut goal = CharacterGoal {
                from: Level::START,
                to: owned.level,
                talents: owned.talents.map(|t| (1, t.min(MAX_TALENT))),
            };
            if character.key.starts_with("traveler") {
                if traveler_level.is_none_or(|(level, _)| owned.level > level) {
                    traveler_level = Some((owned.level, character));
                }
                goal.to = Level::START;
            }
            invested.merge(&planner.character_cost(character, &goal));
        }
        if let Some((level, character)) = traveler_level {
            let goal = CharacterGoal {
                from: Level::START,
                to: level,
                talents: [(1, 1); 3],
            };
            invested.merge(&planner.character_cost(character, &goal));
        }
        stats.invested = invested.materials_with_exp();

        // The regions in the story's order, those the list lacks after, and
        // the characters of none last.
        let rank = |region: &Option<Text>| match region {
            Some(region) => REGIONS
                .iter()
                .position(|known| *known == region.english())
                .unwrap_or(REGIONS.len()),
            None => REGIONS.len() + 1,
        };
        regions.sort_by_key(|(region, _)| rank(region));
        stats.regions = regions;

        versions.sort_by_key(|(order, _, _)| *order);
        let (mut released, mut owned) = (0, 0);
        for (order, version, is_owned) in versions {
            released += 1;
            owned += usize::from(is_owned);
            match stats.by_version.last_mut() {
                Some(last) if version_order(&last.version) == order => {
                    (last.released, last.owned) = (released, owned);
                }
                _ => stats.by_version.push(VersionTally {
                    version: version.to_owned(),
                    released,
                    owned,
                }),
            }
        }
        stats
    }

    /// The average level of talent `slot` (0 normal attack, 1 skill, 2
    /// burst) over the owned characters; `None` when there are none.
    pub fn average_talent(&self, slot: usize) -> Option<f64> {
        let total = *self.talent_totals.get(slot)?;
        (self.all.owned > 0).then(|| total as f64 / self.all.owned as f64)
    }

    /// How many of the materials of `role` (talent books, weekly boss
    /// materials…) raising the owned characters took, every tier together.
    pub fn invested_in(&self, role: Role, planner: &PlannerData) -> u64 {
        (planner.families(role).iter().flatten())
            .filter_map(|id| self.invested.get(id))
            .sum()
    }

    /// Owned characters at the highest level.
    pub fn max_level(&self) -> usize {
        self.levels[0]
    }

    /// Talents at the highest level, over every owned character.
    pub fn crowns(&self) -> usize {
        self.talents[0]
    }
}

/// A version's place among the others: "4.7" as (4, 7). One that doesn't
/// read as such goes last.
fn version_order(version: &str) -> (u32, u32) {
    let mut parts = version.split('.').map(|part| part.trim().parse::<u32>());
    match (parts.next(), parts.next()) {
        (Some(Ok(major)), Some(Ok(minor))) => (major, minor),
        (Some(Ok(major)), None) => (major, 0),
        _ => (u32::MAX, u32::MAX),
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
                name: weapon.name.to_string(),
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
        let (planner, archive) = (PlannerData::embedded(), ArchiveData::embedded());
        let empty = CharacterStats::of(&Collection::default(), planner, archive);
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
        let stats = CharacterStats::of(&collection, planner, archive);
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
        assert_eq!(stats.five_constellations, [0, 0, 1, 0, 0, 0, 0]);
        assert_eq!(stats.four_constellations, [1, 0, 0, 0, 0, 0, 0]);
        assert_eq!(stats.levels, [1, 0, 0, 1]);
        assert_eq!((stats.max_level(), stats.crowns()), (1, 2));
        // Ganyu's 10, 10 and 9; Bennett's three at 1.
        assert_eq!(stats.talents, [2, 1, 0, 0, 3]);
        assert_eq!(stats.average_talent(0), Some(5.5));
        assert_eq!(stats.average_talent(2), Some(5.0));
        assert_eq!(empty.average_talent(0), None);
        // Ganyu is of Liyue, Bennett of Mondstadt; the Traveler of none.
        let region = |name: Option<&str>| {
            (stats.regions.iter())
                .find(|(r, _)| r.as_ref().map(Text::english) == name)
                .unwrap()
                .1
        };
        assert_eq!(region(Some("Liyue")).owned, 1);
        assert_eq!(region(Some("Mondstadt")).owned, 1);
        assert_eq!(region(Some("Inazuma")).owned, 0);
        assert!(region(None).total >= 7);
        assert_eq!(
            stats.regions[0].0.as_ref().map(Text::english),
            Some("Mondstadt")
        );
        assert_eq!(stats.regions.last().unwrap().0, None);
        assert_eq!(
            stats.regions.iter().map(|(_, t)| t.total).sum::<usize>(),
            stats.all.total
        );
        // Every character has a known element and weapon.
        let sum = |list: &[(&str, Tally)]| list.iter().map(|(_, t)| t.total).sum::<usize>();
        assert_eq!(sum(&stats.elements), stats.all.total);
        assert_eq!(sum(&stats.weapons), stats.all.total);
    }

    #[test]
    fn versions_add_up_the_characters_released_by_then() {
        let (planner, archive) = (PlannerData::embedded(), ArchiveData::embedded());
        let collection = Collection {
            characters: vec![OwnedCharacter::new("ganyu"), OwnedCharacter::new("furina")],
            weapons: Vec::new(),
        };
        let stats = CharacterStats::of(&collection, planner, archive);
        let versions = &stats.by_version;
        assert_eq!(versions[0].version, "1.0");
        // Ganyu came in 1.2, Furina in 4.2.
        let at = |version: &str| versions.iter().find(|v| v.version == version).unwrap();
        assert_eq!(at("1.1").owned, 0);
        assert_eq!(at("1.2").owned, 1);
        assert_eq!(at("4.2").owned, 2);
        for pair in versions.windows(2) {
            assert!(version_order(&pair[0].version) < version_order(&pair[1].version));
            assert!(pair[0].released < pair[1].released);
        }
        let last = versions.last().unwrap();
        assert_eq!((last.released, last.owned), (stats.all.total, 2));
        assert_eq!(version_order("10.1"), (10, 1));
        assert!(version_order("Luna I") > version_order("9.9"));
    }

    #[test]
    fn what_raising_the_characters_cost_is_the_planner_s() {
        let (planner, archive) = (PlannerData::embedded(), ArchiveData::embedded());
        let raised = |key: &str| OwnedCharacter {
            level: Level::MAX,
            talents: [MAX_TALENT; 3],
            ..OwnedCharacter::new(key)
        };
        let ganyu = Collection {
            characters: vec![raised("ganyu"), OwnedCharacter::new("bennett")],
            weapons: Vec::new(),
        };
        let stats = CharacterStats::of(&ganyu, planner, archive);
        let entry = crate::planner::PlanEntry {
            id: 1,
            target: crate::planner::PlanTarget::Character {
                key: "ganyu".to_owned(),
                goal: CharacterGoal::FULL,
            },
        };
        // Bennett at level 1, talents at 1, cost nothing.
        assert_eq!(stats.invested, planner.entry_totals(&entry).materials);
        assert!(stats.invested[&crate::planner::MORA] > 0);
        // Ganyu's three talents to 10: three crowns, and her books (Diligence).
        assert_eq!(stats.invested[&crate::planner::CROWN], 3);
        let books = stats.invested_in(Role::TalentBook, planner);
        let diligence: u64 = (104313..=104315).map(|id| stats.invested[&id]).sum();
        assert_eq!(books, diligence);
        assert_eq!(stats.invested_in(Role::WeeklyMaterial, planner), 18);

        // The Traveler levels once for every element; each element's talents
        // are its own.
        let one = Collection {
            characters: vec![raised("travelergeo")],
            weapons: Vec::new(),
        };
        let two = Collection {
            characters: vec![raised("travelergeo"), raised("traveleranemo")],
            weapons: Vec::new(),
        };
        let (one, two) = (
            CharacterStats::of(&one, planner, archive).invested,
            CharacterStats::of(&two, planner, archive).invested,
        );
        let hero_s_wit = crate::planner::EXP_BOOKS[0].0;
        assert_eq!(one[&hero_s_wit], two[&hero_s_wit]);
        assert!(two[&crate::planner::MORA] > one[&crate::planner::MORA]);
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
