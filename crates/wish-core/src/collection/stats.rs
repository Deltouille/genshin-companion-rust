//! An account's collection in figures, for the overview: its characters
//! (how many of the game's, by rarity, element and weapon; constellations,
//! levels and talents) and the weapons its wishes brought.
//!
//! The game's characters are counted as the archive lists them: the Traveler
//! once per element. Nothing here is stored: it is worked out when shown.

use std::cmp::Reverse;
use std::collections::HashMap;

use super::{Collection, MAX_CONSTELLATION, MAX_REFINEMENT, MAX_TALENT};
use crate::model::{ItemType, Rarity, Wish};
use crate::planner::{Level, PlannerData, Weapon};

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

/// A 4★ or 5★ weapon the account's wishes brought.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WishedWeapon {
    /// As the wishes name it.
    pub name: String,
    /// The game data's key (the planner's), when it knows the name.
    pub key: Option<String>,
    pub rarity: Rarity,
    /// One of [`WEAPON_TYPES`], when the game data knows the name.
    pub weapon_type: Option<&'static str>,
    pub copies: u32,
}

impl WishedWeapon {
    /// The refinement the copies make, R1 to R5. A floor: copies from the
    /// shop, events or the forge aren't wishes.
    pub fn refinement(&self) -> u8 {
        self.copies.clamp(1, u32::from(MAX_REFINEMENT)) as u8
    }
}

/// The weapons an account's wishes brought.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WishedWeapons {
    /// The 4★ and 5★ ones: the 5★ first, then the most copies, then by name.
    pub weapons: Vec<WishedWeapon>,
    /// How many 3★ weapons came out.
    pub three_stars: u32,
}

impl WishedWeapons {
    /// The weapons among `wishes`, told apart by name.
    pub fn of<'a>(wishes: impl IntoIterator<Item = &'a Wish>, planner: &PlannerData) -> Self {
        let known: HashMap<&str, &Weapon> = planner
            .weapons()
            .iter()
            .map(|weapon| (weapon.name.as_str(), weapon))
            .collect();
        let mut three_stars = 0;
        let mut copies: HashMap<&str, (Rarity, u32)> = HashMap::new();
        for wish in wishes {
            if wish.item_type != ItemType::Weapon {
                continue;
            }
            match wish.rarity {
                Rarity::Three => three_stars += 1,
                rarity => copies.entry(&wish.name).or_insert((rarity, 0)).1 += 1,
            }
        }
        let mut weapons: Vec<WishedWeapon> = copies
            .into_iter()
            .map(|(name, (rarity, copies))| {
                let weapon = known.get(name);
                WishedWeapon {
                    name: name.to_owned(),
                    key: weapon.map(|w| w.key.clone()),
                    rarity,
                    weapon_type: weapon
                        .and_then(|w| WEAPON_TYPES.iter().find(|t| **t == w.weapon).copied()),
                    copies,
                }
            })
            .collect();
        weapons.sort_by(|a, b| {
            (Reverse(a.rarity), Reverse(a.copies), &a.name).cmp(&(
                Reverse(b.rarity),
                Reverse(b.copies),
                &b.name,
            ))
        });
        Self {
            weapons,
            three_stars,
        }
    }

    /// The weapons of `rarity`.
    pub fn of_rarity(&self, rarity: Rarity) -> impl Iterator<Item = &WishedWeapon> {
        self.weapons.iter().filter(move |w| w.rarity == rarity)
    }

    /// How many copies of `rarity` came out.
    pub fn copies(&self, rarity: Rarity) -> u32 {
        match rarity {
            Rarity::Three => self.three_stars,
            rarity => self.of_rarity(rarity).map(|w| w.copies).sum(),
        }
    }

    /// Every weapon wished, 3★ included.
    pub fn total(&self) -> u32 {
        self.three_stars + self.weapons.iter().map(|w| w.copies).sum::<u32>()
    }

    /// The 4★ and 5★ weapons with copies enough for R5.
    pub fn refined_fully(&self) -> usize {
        self.weapons
            .iter()
            .filter(|w| w.refinement() == MAX_REFINEMENT)
            .count()
    }

    /// By type, in [`WEAPON_TYPES`]' order: how many 4★ and 5★ weapons, and
    /// their copies.
    pub fn by_type(&self) -> Vec<(&'static str, usize, u32)> {
        WEAPON_TYPES
            .map(|kind| {
                let of_kind = self.weapons.iter().filter(|w| w.weapon_type == Some(kind));
                let (count, copies) = of_kind.fold((0, 0), |(n, c), w| (n + 1, c + w.copies));
                (kind, count, copies)
            })
            .to_vec()
    }

    pub fn is_empty(&self) -> bool {
        self.weapons.is_empty() && self.three_stars == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collection::OwnedCharacter;
    use crate::model::{GachaType, Source, WishId, WishTime};

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

    fn wish(seq: u32, name: &str, item_type: ItemType, rarity: Rarity) -> Wish {
        let time: WishTime = "2024-05-01 12:00:00".parse().unwrap();
        let gacha_type = GachaType::WeaponEvent;
        Wish {
            id: WishId::paimon(gacha_type.pool(), time, seq),
            gacha_type,
            time,
            seq,
            name: name.to_owned(),
            item_type,
            rarity,
            item_key: None,
            manual_pity: None,
            source: Source::Paimon,
        }
    }

    #[test]
    fn wished_weapons_count_copies_by_name() {
        let mut wishes = Vec::new();
        let mut add = |name: &str, item_type, rarity, copies| {
            for _ in 0..copies {
                wishes.push(wish(wishes.len() as u32, name, item_type, rarity));
            }
        };
        add("Amos' Bow", ItemType::Weapon, Rarity::Five, 3);
        add("Favonius Sword", ItemType::Weapon, Rarity::Four, 6);
        add("Slingshot", ItemType::Weapon, Rarity::Three, 4);
        add("A Blade Nobody Knows", ItemType::Weapon, Rarity::Four, 1);
        add("Ganyu", ItemType::Character, Rarity::Five, 1);

        let weapons = WishedWeapons::of(&wishes, PlannerData::embedded());
        let summary: Vec<(&str, Option<&str>, u32, u8)> = weapons
            .weapons
            .iter()
            .map(|w| (w.name.as_str(), w.weapon_type, w.copies, w.refinement()))
            .collect();
        assert_eq!(
            summary,
            [
                ("Amos' Bow", Some("Bow"), 3, 3),
                ("Favonius Sword", Some("Sword"), 6, 5),
                ("A Blade Nobody Knows", None, 1, 1),
            ]
        );
        assert_eq!(weapons.weapons[0].key.as_deref(), Some("amosbow"));
        assert_eq!(weapons.three_stars, 4);
        assert_eq!(weapons.total(), 14);
        assert_eq!(weapons.copies(Rarity::Four), 7);
        assert_eq!(weapons.refined_fully(), 1);
        let by_type = weapons.by_type();
        assert!(by_type.contains(&("Bow", 1, 3)));
        assert!(by_type.contains(&("Sword", 1, 6)));
        assert!(by_type.contains(&("Catalyst", 0, 0)));
    }
}
