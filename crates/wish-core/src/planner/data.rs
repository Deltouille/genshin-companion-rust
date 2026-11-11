//! The planner's game data, extracted from genshin-db by
//! `tools/extract-genshin-db.mjs` into the app's own format.
//!
//! Like the wish data, the JSON is embedded in the binary and a copy in the
//! app's game-data folder takes precedence, so a new character does not
//! require a rebuild. A file whose costs name an unknown material is refused:
//! a wrong count would mislead every plan using it.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use chrono::Weekday;
use serde::Deserialize;

use crate::data::DataSource;
use crate::error::{Error, Result};

use super::custom::{Role, Roles};

pub const PLANNER_FILE: &str = "planner.json";

const EMBEDDED_PLANNER: &str = include_str!("../../data/planner.json");

/// The highest level a character or weapon reaches.
pub const MAX_LEVEL: u8 = 90;

/// A material's id in the game (Mora is 202).
pub type MaterialId = u32;

/// So many of a material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(from = "(MaterialId, u32)")]
pub struct Cost {
    pub material: MaterialId,
    pub count: u32,
}

impl From<(MaterialId, u32)> for Cost {
    fn from((material, count): (MaterialId, u32)) -> Self {
        Self { material, count }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Material {
    pub id: MaterialId,
    /// genshin-db's key, such as `philosophiesofdiligence`.
    pub key: String,
    pub name: String,
    pub rarity: u8,
    /// What it is for, as the game says: "Character Talent Material".
    pub kind: String,
    /// The game's inventory order.
    pub sort: u32,
    /// The days its domain gives it, for talent and weapon materials.
    #[serde(deserialize_with = "weekdays")]
    pub days: Vec<Weekday>,
    pub domain: Option<String>,
    /// The game's file name for its icon (`UI_ItemIcon_104315`).
    pub icon: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Character {
    /// genshin-db's key (`kamisatoayaka`); one per element for the Traveler
    /// (`travelergeo`).
    pub key: String,
    pub id: u32,
    pub name: String,
    pub rarity: u8,
    pub element: String,
    /// The weapon type: "Bow", "Sword"…
    pub weapon: String,
    pub icon: Option<String>,
    /// The cost of ascension phases 1 to 6.
    pub ascension: Vec<Vec<Cost>>,
    /// The cost of raising a talent to levels 2 to 10.
    pub talents: Vec<Vec<Cost>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Weapon {
    pub key: String,
    pub id: u32,
    pub name: String,
    pub rarity: u8,
    pub weapon: String,
    pub icon: Option<String>,
    /// The cost of each ascension phase: 4 for 1★ and 2★ weapons (up to
    /// level 70), 6 for the others.
    pub ascension: Vec<Vec<Cost>>,
}

/// Characters, weapons and materials, with the EXP tables.
#[derive(Clone, Debug)]
pub struct PlannerData {
    pub source: DataSource,
    pub(super) characters: Vec<Character>,
    pub(super) weapons: Vec<Weapon>,
    materials: Vec<Material>,
    character_by_key: HashMap<String, usize>,
    weapon_by_key: HashMap<String, usize>,
    material_by_id: HashMap<MaterialId, usize>,
    /// EXP from each level to the next, levels 1 to 89.
    character_exp: Vec<u32>,
    /// The same per weapon rarity, 1★ first.
    weapon_exp: Vec<Vec<u32>>,
    /// The series of materials each role takes, from the characters (see
    /// `custom.rs`).
    pub(super) families: BTreeMap<Role, Vec<Vec<MaterialId>>>,
    /// The character custom ones are modelled on, and its materials by role.
    pub(super) template: Option<(usize, Roles)>,
    /// The weapon of each rarity custom ones are modelled on.
    pub(super) weapon_templates: BTreeMap<u8, (usize, Roles)>,
    /// Materials crafted three for one into the next tier, lowest first.
    craft_families: Vec<Vec<MaterialId>>,
    craft_family_of: HashMap<MaterialId, usize>,
}

static EMBEDDED: LazyLock<PlannerData> = LazyLock::new(|| {
    PlannerData::from_json(EMBEDDED_PLANNER)
        .expect("embedded planner data is validated by the extraction script and tests")
});

impl PlannerData {
    /// The data compiled into this build.
    pub fn embedded() -> &'static PlannerData {
        &EMBEDDED
    }

    /// Loads `planner.json` from `dir` when present, else the embedded copy.
    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join(PLANNER_FILE);
        match fs::read_to_string(&path) {
            Ok(json) => Self::from_json(&json),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::embedded().clone()),
            Err(e) => Err(Error::io("reading", &path)(e)),
        }
    }

    pub fn from_json(json: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct File {
            source: DataSource,
            character_exp: Vec<u32>,
            weapon_exp: Vec<Vec<u32>>,
            characters: Vec<Character>,
            weapons: Vec<Weapon>,
            materials: Vec<Material>,
        }
        let file: File = serde_json::from_str(json)
            .map_err(|e| Error::GameData(format!("{PLANNER_FILE}: {e}")))?;
        let index = |keys: Vec<String>| -> HashMap<String, usize> {
            keys.into_iter().enumerate().map(|(i, k)| (k, i)).collect()
        };
        let mut data = Self {
            source: file.source,
            character_by_key: index(file.characters.iter().map(|c| c.key.clone()).collect()),
            weapon_by_key: index(file.weapons.iter().map(|w| w.key.clone()).collect()),
            material_by_id: file
                .materials
                .iter()
                .enumerate()
                .map(|(i, m)| (m.id, i))
                .collect(),
            characters: file.characters,
            weapons: file.weapons,
            materials: file.materials,
            character_exp: file.character_exp,
            weapon_exp: file.weapon_exp,
            families: BTreeMap::new(),
            template: None,
            weapon_templates: BTreeMap::new(),
            craft_families: Vec::new(),
            craft_family_of: HashMap::new(),
        };
        data.check()?;
        // Crafting first: telling elite drops from common ones uses it.
        data.index_crafting();
        data.index_roles();
        Ok(data)
    }

    pub fn characters(&self) -> &[Character] {
        &self.characters
    }

    pub fn weapons(&self) -> &[Weapon] {
        &self.weapons
    }

    pub fn materials(&self) -> &[Material] {
        &self.materials
    }

    pub fn character(&self, key: &str) -> Option<&Character> {
        self.character_by_key.get(key).map(|&i| &self.characters[i])
    }

    pub fn weapon(&self, key: &str) -> Option<&Weapon> {
        self.weapon_by_key.get(key).map(|&i| &self.weapons[i])
    }

    pub fn material(&self, id: MaterialId) -> Option<&Material> {
        self.material_by_id.get(&id).map(|&i| &self.materials[i])
    }

    /// Every series of materials crafted three for one into the next tier
    /// (gems, talent books, enemy drops, weapon materials), lowest tier first.
    pub fn craft_families(&self) -> &[Vec<MaterialId>] {
        &self.craft_families
    }

    /// The series `id` is crafted in, lowest tier first.
    pub fn craft_family(&self, id: MaterialId) -> Option<&[MaterialId]> {
        let index = *self.craft_family_of.get(&id)?;
        Some(&self.craft_families[index])
    }

    /// Groups the materials crafted at the alchemy table into their series:
    /// those the game files under one kind and one inventory rank, one per
    /// rarity. A group whose rarities don't follow each other is left out.
    fn index_crafting(&mut self) {
        const CRAFTED: [&str; 4] = [
            "Character Ascension Material",
            "Character Talent Material",
            "Character and Weapon Enhancement Material",
            "Weapon Ascension Material",
        ];
        let mut groups: BTreeMap<(&str, u32), Vec<&Material>> = BTreeMap::new();
        for material in &self.materials {
            if CRAFTED.contains(&material.kind.as_str()) {
                groups
                    .entry((material.kind.as_str(), material.sort))
                    .or_default()
                    .push(material);
            }
        }
        let mut families = Vec::new();
        for mut group in groups.into_values() {
            group.sort_by_key(|m| m.rarity);
            let tiers = group.windows(2).all(|w| w[1].rarity == w[0].rarity + 1);
            if group.len() > 1 && tiers {
                families.push(group.iter().map(|m| m.id).collect::<Vec<_>>());
            }
        }
        self.craft_family_of = (families.iter().enumerate())
            .flat_map(|(i, family)| family.iter().map(move |&id| (id, i)))
            .collect();
        self.craft_families = families;
    }

    /// EXP from `level` to the next one for a character.
    pub(crate) fn character_exp(&self, level: u8) -> u32 {
        self.character_exp[usize::from(level) - 1]
    }

    /// EXP from `level` to the next one for a weapon of `rarity` stars.
    pub(crate) fn weapon_exp(&self, rarity: u8, level: u8) -> u32 {
        self.weapon_exp[usize::from(rarity) - 1][usize::from(level) - 1]
    }

    /// Every table complete and every cost a known material.
    fn check(&self) -> Result<()> {
        let bad = |what: String| Err(Error::GameData(format!("{PLANNER_FILE}: {what}")));
        let levels = usize::from(MAX_LEVEL) - 1;
        if self.character_exp.len() != levels {
            return bad(format!("{} character EXP levels", self.character_exp.len()));
        }
        if self.weapon_exp.len() != 5 || self.weapon_exp.iter().any(|t| t.len() != levels) {
            return bad("the weapon EXP tables are incomplete".to_owned());
        }
        let costs = |owner: &str, steps: &[Vec<Cost>]| -> Result<()> {
            for cost in steps.iter().flatten() {
                if self.material(cost.material).is_none() {
                    return bad(format!("{owner} costs unknown material {}", cost.material));
                }
            }
            Ok(())
        };
        for character in &self.characters {
            if character.ascension.len() != 6 || character.talents.len() != 9 {
                return bad(format!("{} lacks ascension or talent costs", character.key));
            }
            if !(4..=5).contains(&character.rarity) {
                return bad(format!("{} has rarity {}", character.key, character.rarity));
            }
            costs(&character.key, &character.ascension)?;
            costs(&character.key, &character.talents)?;
        }
        for weapon in &self.weapons {
            if !(1..=5).contains(&weapon.rarity)
                || weapon.ascension.len() != usize::from(super::phases(weapon.rarity))
            {
                return bad(format!("{} lacks ascension costs", weapon.key));
            }
            costs(&weapon.key, &weapon.ascension)?;
        }
        Ok(())
    }
}

/// `["Monday", "Thursday"]` → weekdays.
fn weekdays<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<Weekday>, D::Error> {
    let names: Vec<String> = Vec::deserialize(deserializer)?;
    names
        .iter()
        .map(|name| {
            name.parse::<Weekday>()
                .map_err(|_| serde::de::Error::custom(format!("unknown day `{name}`")))
        })
        .collect()
}
