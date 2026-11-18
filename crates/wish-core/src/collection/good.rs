//! A GOOD file (Genshin Open Object Description, the format Genshin Optimizer
//! documents at <https://frzyc.github.io/genshin-optimizer/#/doc>), as
//! Irminsul, Inventory Kamera and Genshin Optimizer write it: an account's
//! characters, every weapon copy and the bag's materials, by key. Read and set
//! against the account's collection and inventory here; nothing is written
//! ([`Store::apply_good`](crate::Store::apply_good) does, once the player
//! confirmed the preview).
//!
//! How a file is read (checked on a real Irminsul export, 2026-11-17: 113
//! characters, 1,940 weapon copies, 1,349 entries in the bag):
//!
//! - keys are the English names without apostrophes, any other symbol a word
//!   break, each word capitalised ("Night-Wind's Mystic Augury" is
//!   `NightWindsMysticAugury`), so they are matched against `planner.json`'s
//!   names put the same way ([`good_key`]); the Traveler is
//!   `Traveler<Element>`, and a weapon's holder plain `Traveler`;
//! - a character's level comes with its ascension phase, its talents without
//!   the constellations' +3; a level above 90 counts as 90/90, and is said;
//! - every weapon copy, held or not, fodder included;
//! - the materials are the whole bag: those the planner uses take the file's
//!   count, and one the bag lacks is none left, Mora apart (the files carry
//!   none). Food, gadgets and the like are left out, counted in a note.
//!
//! The version (1 to 3 today) isn't checked: each one only added fields, and
//! the fields read here are in all of them.
//!
//! Like a HoYoLAB sync, characters and weapon copies are only added or
//! raised, never lowered nor removed.

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Deserialize;

use super::sync::{Change, SyncNote, SyncPlan, character_change};
use super::{
    Collection, MAX_CONSTELLATION, MAX_REFINEMENT, MAX_TALENT, OwnedCharacter, OwnedWeapon,
};
use crate::error::{Error, Result};
use crate::planner::{CustomItem, Inventory, Level, MORA, MaterialId, PlannerData};

/// A GOOD file, as far as the app reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GoodFile {
    /// The app that wrote it, such as `Irminsul`.
    pub source: String,
    pub characters: Vec<GoodCharacter>,
    pub weapons: Vec<GoodWeapon>,
    /// The bag, by key; `None` when the file carries no materials (or an
    /// empty list, which a scan of the characters alone writes).
    pub materials: Option<BTreeMap<String, u64>>,
}

/// A character of a GOOD file.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct GoodCharacter {
    pub key: String,
    pub level: u8,
    #[serde(default)]
    pub constellation: u8,
    /// The ascension phase, 0 to 6.
    #[serde(default)]
    pub ascension: u8,
    /// Without the constellations' +3.
    #[serde(default)]
    pub talent: GoodTalents,
}

/// A character's talent levels in a GOOD file, without the constellations'
/// bonus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct GoodTalents {
    pub auto: u8,
    pub skill: u8,
    pub burst: u8,
}

impl Default for GoodTalents {
    fn default() -> Self {
        Self {
            auto: 1,
            skill: 1,
            burst: 1,
        }
    }
}

/// A weapon copy of a GOOD file.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct GoodWeapon {
    pub key: String,
    pub level: u8,
    /// The ascension phase, 0 to 6.
    #[serde(default)]
    pub ascension: u8,
    #[serde(default = "first_refinement")]
    pub refinement: u8,
    /// The holder's key; empty for nobody.
    #[serde(default)]
    pub location: String,
}

fn first_refinement() -> u8 {
    1
}

/// What a file says it is: GOOD names itself in `format`.
#[derive(Deserialize)]
struct Probe {
    #[serde(default)]
    format: Option<String>,
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(default)]
    source: String,
    #[serde(default)]
    characters: Vec<GoodCharacter>,
    #[serde(default)]
    weapons: Vec<GoodWeapon>,
    #[serde(default)]
    materials: Option<BTreeMap<String, u64>>,
}

impl GoodFile {
    /// Reads a GOOD file: a file's bytes, or the clipboard's text. What isn't
    /// a GOOD file at all is [`Error::NotGoodFile`]; one that is, with a
    /// value of the wrong kind, the JSON error.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
        match serde_json::from_slice::<Probe>(bytes) {
            Ok(Probe {
                format: Some(format),
            }) if format == "GOOD" => {}
            _ => return Err(Error::NotGoodFile),
        }
        let raw: RawFile = serde_json::from_slice(bytes)?;
        Ok(Self {
            source: raw.source,
            characters: raw.characters,
            weapons: raw.weapons,
            materials: raw.materials.filter(|bag| !bag.is_empty()),
        })
    }
}

/// A material whose count a GOOD import changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterialChange {
    pub id: MaterialId,
    pub before: u64,
    pub after: u64,
}

/// What importing a GOOD file would change.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GoodPlan {
    /// The characters and weapon copies, and every note (the materials'
    /// too).
    pub collection: SyncPlan,
    /// The materials whose count changes, in the planner's order.
    pub materials: Vec<MaterialChange>,
}

impl GoodPlan {
    /// How many characters, weapon copies and materials it changes.
    pub fn changes(&self) -> usize {
        self.collection.changes() + self.materials.len()
    }
}

/// A name as GOOD keys it: apostrophes dropped, accents taken off, any other
/// symbol a word break, each word capitalised.
pub fn good_key(name: &str) -> String {
    let mut key = String::with_capacity(name.len());
    let mut word_start = true;
    for c in name.chars() {
        if matches!(c, '\'' | '’') {
            continue;
        }
        let c = unaccented(c);
        if c.is_ascii_alphanumeric() {
            key.push(if word_start {
                c.to_ascii_uppercase()
            } else {
                c
            });
            word_start = false;
        } else {
            word_start = true;
        }
    }
    key
}

fn unaccented(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'ç' => 'c',
        'Ç' => 'C',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'È' | 'É' | 'Ê' | 'Ë' => 'E',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
        'ñ' => 'n',
        'Ñ' => 'N',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
        _ => c,
    }
}

/// The game data by GOOD key: the planner's keys of characters and weapons,
/// the ids of materials (placeholders apart). Two names that key alike keep
/// the first.
struct Keys<'a> {
    characters: HashMap<String, &'a str>,
    weapons: HashMap<String, &'a str>,
    materials: HashMap<String, MaterialId>,
}

impl<'a> Keys<'a> {
    fn new(planner: &'a PlannerData) -> Self {
        fn index<T: Copy>(pairs: impl Iterator<Item = (String, T)>) -> HashMap<String, T> {
            let mut map = HashMap::new();
            for (key, value) in pairs {
                map.entry(key).or_insert(value);
            }
            map
        }
        Self {
            characters: index(
                (planner.characters().iter()).map(|c| (good_key(c.name.english()), c.key.as_str())),
            ),
            weapons: index(
                (planner.weapons().iter()).map(|w| (good_key(w.name.english()), w.key.as_str())),
            ),
            materials: index(
                (planner.materials().iter())
                    .filter(|m| !m.is_placeholder())
                    .map(|m| (good_key(m.name.english()), m.id)),
            ),
        }
    }
}

/// Sets `file` against `current`, the account's collection, and `inventory`,
/// its materials.
pub fn plan_good(
    file: &GoodFile,
    current: &Collection,
    inventory: &Inventory,
    planner: &PlannerData,
) -> GoodPlan {
    let keys = Keys::new(planner);
    let mut notes = Vec::new();
    let characters = plan_characters(file, &keys, current, planner, &mut notes);
    let weapons = plan_weapons(file, &keys, current, &mut notes);
    let materials = plan_materials(file, &keys, inventory, planner, &mut notes);
    GoodPlan {
        collection: SyncPlan {
            characters,
            weapons,
            notes,
        },
        materials,
    }
}

/// A level and phase as the collection keeps them: 90/90 above 90 (`true`
/// then), the phase's nearest level when the two don't go together.
fn read_level(level: u8, ascension: u8) -> (Level, bool) {
    if level > Level::MAX.level {
        return (Level::MAX, true);
    }
    let level = level.max(1);
    let read = Level::new(level, ascension).unwrap_or_else(|| Level::clamped(level, ascension));
    (read, false)
}

fn plan_characters(
    file: &GoodFile,
    keys: &Keys,
    current: &Collection,
    planner: &PlannerData,
    notes: &mut Vec<SyncNote>,
) -> Vec<Change<OwnedCharacter>> {
    let mut changes = Vec::new();
    for character in &file.characters {
        let Some(&key) = keys.characters.get(&character.key) else {
            notes.push(SyncNote::UnknownGoodCharacter {
                key: character.key.clone(),
            });
            continue;
        };
        let (level, above) = read_level(character.level, character.ascension);
        let talent = character.talent;
        let synced = OwnedCharacter {
            key: key.to_owned(),
            level,
            constellation: character.constellation.min(MAX_CONSTELLATION),
            talents: [talent.auto, talent.skill, talent.burst].map(|t| t.clamp(1, MAX_TALENT)),
            favourite: false,
        };
        let change = character_change(current.character(key), synced, true);
        // Said only when the file's level is the one kept.
        if above && change.after().level == level {
            notes.push(SyncNote::AboveMaxLevel {
                name: (planner.character(key))
                    .map_or(key, |c| c.name.english())
                    .to_owned(),
                level: character.level,
            });
        }
        changes.push(change);
    }
    changes
}

/// Each copy of the file set against the stored ones, in the file's order:
/// the copy its holder already holds, else a stored copy just like it, else
/// the most levelled one below it (raised); else a new copy. A stored copy
/// is taken once, and one whose holder holds that weapon in the file too is
/// left to that holder. Held copies go first, then the others from the most
/// levelled down, so the best stored copies meet the best of the file.
fn plan_weapons(
    file: &GoodFile,
    keys: &Keys,
    current: &Collection,
    notes: &mut Vec<SyncNote>,
) -> Vec<Change<OwnedWeapon>> {
    // A weapon's holder `Traveler` is the file's Traveler, whatever the element.
    let traveler = (file.characters.iter())
        .filter(|c| c.key.starts_with("Traveler"))
        .find_map(|c| keys.characters.get(&c.key).copied());
    let holder = |location: &str| -> Option<String> {
        match location {
            "" => None,
            "Traveler" => traveler.map(str::to_owned),
            key => keys.characters.get(key).map(|key| (*key).to_owned()),
        }
    };
    let mut unknown: BTreeMap<&str, usize> = BTreeMap::new();
    let synced: Vec<OwnedWeapon> = (file.weapons.iter())
        .filter_map(|weapon| {
            let Some(&key) = keys.weapons.get(&weapon.key) else {
                *unknown.entry(&weapon.key).or_default() += 1;
                return None;
            };
            Some(OwnedWeapon {
                id: 0,
                key: key.to_owned(),
                level: read_level(weapon.level, weapon.ascension).0,
                refinement: weapon.refinement.clamp(1, MAX_REFINEMENT),
                holder: holder(&weapon.location),
                favourite: false,
            })
        })
        .collect();
    notes.extend(
        unknown
            .into_iter()
            .map(|(key, copies)| SyncNote::UnknownGoodWeapon {
                key: key.to_owned(),
                copies,
            }),
    );

    // Who holds which weapon in the file.
    let holds: HashSet<(&str, &str)> = (synced.iter())
        .filter_map(|w| Some((w.holder.as_deref()?, w.key.as_str())))
        .collect();
    let mut stored: HashMap<&str, Vec<&OwnedWeapon>> = HashMap::new();
    for weapon in &current.weapons {
        stored.entry(weapon.key.as_str()).or_default().push(weapon);
    }
    let mut order: Vec<usize> = (0..synced.len()).collect();
    order.sort_by_key(|&i| {
        let w = &synced[i];
        (w.holder.is_none(), Reverse(w.level), Reverse(w.refinement))
    });
    let mut claimed: HashSet<i64> = HashSet::new();
    let mut changes: Vec<Option<Change<OwnedWeapon>>> = vec![None; synced.len()];
    for i in order {
        let copy = &synced[i];
        let copies = stored.get(copy.key.as_str()).map_or(&[][..], Vec::as_slice);
        changes[i] = Some(match stored_copy(copy, copies, &claimed, &holds) {
            Some(before) => {
                claimed.insert(before.id);
                raise(before, copy)
            }
            None => Change::Added(copy.clone()),
        });
    }
    changes.into_iter().flatten().collect()
}

/// The stored copy `copy` stands for, if any (see [`plan_weapons`]).
fn stored_copy<'a>(
    copy: &OwnedWeapon,
    copies: &[&'a OwnedWeapon],
    claimed: &HashSet<i64>,
    holds: &HashSet<(&str, &str)>,
) -> Option<&'a OwnedWeapon> {
    let free = |w: &OwnedWeapon| !claimed.contains(&w.id);
    if copy.holder.is_some()
        && let Some(held) = (copies.iter()).find(|w| free(w) && w.holder == copy.holder)
    {
        return Some(held);
    }
    let open = |w: &OwnedWeapon| {
        free(w)
            && !(w.holder.as_deref())
                .is_some_and(|holder| holds.contains(&(holder, w.key.as_str())))
    };
    let alike = (copies.iter())
        .find(|w| open(w) && w.level == copy.level && w.refinement == copy.refinement);
    alike
        .or_else(|| {
            (copies.iter())
                .filter(|w| open(w) && w.level <= copy.level && w.refinement <= copy.refinement)
                .max_by_key(|w| (w.level, w.refinement))
        })
        .copied()
}

/// `before` as the file has it: raised, never lowered, held by the file's
/// holder (nobody included), its favourite kept.
fn raise(before: &OwnedWeapon, copy: &OwnedWeapon) -> Change<OwnedWeapon> {
    let mut after = before.clone();
    after.level = after.level.max(copy.level);
    after.refinement = after.refinement.max(copy.refinement);
    after.holder = copy.holder.clone();
    if after == *before {
        Change::Unchanged(after)
    } else {
        Change::Raised {
            before: before.clone(),
            after,
        }
    }
}

fn plan_materials(
    file: &GoodFile,
    keys: &Keys,
    inventory: &Inventory,
    planner: &PlannerData,
    notes: &mut Vec<SyncNote>,
) -> Vec<MaterialChange> {
    let Some(bag) = &file.materials else {
        notes.push(SyncNote::NoMaterials);
        return Vec::new();
    };
    let mut counts: HashMap<MaterialId, u64> = HashMap::new();
    let mut other = 0;
    for (key, count) in bag {
        match keys.materials.get(key) {
            Some(id) => {
                counts.insert(*id, *count);
            }
            None => other += 1,
        }
    }
    let mut changes = Vec::new();
    for material in planner.materials().iter().filter(|m| !m.is_placeholder()) {
        let after = match counts.get(&material.id) {
            Some(count) => *count,
            None if material.id == MORA => {
                notes.push(SyncNote::NoMora);
                continue;
            }
            // The bag is whole: a material it lacks is none left.
            None => 0,
        };
        let before = inventory.owned(&CustomItem::Material(material.id));
        if before != after {
            changes.push(MaterialChange {
                id: material.id,
                before,
                after,
            });
        }
    }
    if other > 0 {
        notes.push(SyncNote::OtherItems { count: other });
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ganyu at 80 ascended, Skirk above 90, the Cryo Traveler, a Manekina;
    /// Amos' Bow held by Ganyu, a Favonius Sword by the Traveler, three Cool
    /// Steels, a bow the data lacks; Hero's Wit, Teachings of Freedom, a
    /// steak, and no Mora.
    const SAMPLE: &str = r#"{
        "format": "GOOD", "version": 3, "source": "Irminsul",
        "characters": [
            {"key": "Ganyu", "level": 80, "constellation": 2, "ascension": 6,
             "talent": {"auto": 10, "skill": 7, "burst": 7}},
            {"key": "Skirk", "level": 100, "constellation": 3, "ascension": 6,
             "talent": {"auto": 10, "skill": 10, "burst": 10}},
            {"key": "TravelerCryo", "level": 90, "constellation": 6, "ascension": 6,
             "talent": {"auto": 1, "skill": 1, "burst": 1}},
            {"key": "Manekina", "level": 20, "constellation": 0, "ascension": 0,
             "talent": {"auto": 1, "skill": 1, "burst": 1}}
        ],
        "artifacts": [{"setKey": "GladiatorsFinale", "slotKey": "flower", "level": 20}],
        "weapons": [
            {"key": "AmosBow", "level": 90, "ascension": 6, "refinement": 2,
             "location": "Ganyu", "lock": true},
            {"key": "FavoniusSword", "level": 60, "ascension": 3, "refinement": 5,
             "location": "Traveler", "lock": true},
            {"key": "CoolSteel", "level": 1, "ascension": 0, "refinement": 1,
             "location": "", "lock": false},
            {"key": "CoolSteel", "level": 1, "ascension": 0, "refinement": 1,
             "location": "", "lock": false},
            {"key": "CoolSteel", "level": 1, "ascension": 0, "refinement": 1,
             "location": "", "lock": false},
            {"key": "ANewBow", "level": 1, "ascension": 0, "refinement": 1,
             "location": "", "lock": false}
        ],
        "materials": {"HerosWit": 121, "TeachingsOfFreedom": 62, "DeliciousSteak": 142}
    }"#;

    fn planner() -> &'static PlannerData {
        PlannerData::embedded()
    }

    fn material(name: &str) -> MaterialId {
        (planner().materials().iter())
            .find(|m| m.name.english() == name)
            .unwrap()
            .id
    }

    fn level(level: u8, phase: u8) -> Level {
        Level::new(level, phase).unwrap()
    }

    /// The collection and inventory after `plan` is written, ids given.
    fn applied(
        plan: &GoodPlan,
        current: &Collection,
        inventory: &Inventory,
    ) -> (Collection, Inventory) {
        let mut weapons = current.weapons.clone();
        let mut next = weapons.iter().map(|w| w.id).max().unwrap_or(0);
        for change in &plan.collection.weapons {
            match change {
                Change::Added(weapon) => {
                    next += 1;
                    weapons.push(OwnedWeapon {
                        id: next,
                        ..weapon.clone()
                    });
                }
                Change::Raised { after, .. } => {
                    let at = weapons.iter().position(|w| w.id == after.id).unwrap();
                    weapons[at] = after.clone();
                }
                Change::Unchanged(_) => {}
            }
        }
        let mut characters = current.characters.clone();
        for change in &plan.collection.characters {
            characters.retain(|c| c.key != change.after().key);
            characters.push(change.after().clone());
        }
        let mut inventory = inventory.clone();
        for change in &plan.materials {
            inventory.set(&CustomItem::Material(change.id), change.after);
        }
        (
            Collection {
                characters,
                weapons,
            },
            inventory,
        )
    }

    #[test]
    fn keys_are_the_english_names_run_together() {
        assert_eq!(good_key("Hero's Wit"), "HerosWit");
        assert_eq!(
            good_key("Night-Wind's Mystic Augury"),
            "NightWindsMysticAugury"
        );
        assert_eq!(
            good_key("Saurian-Crowned Warrior's Golden Whistle"),
            "SaurianCrownedWarriorsGoldenWhistle"
        );
        assert_eq!(good_key("Traveler (Cryo)"), "TravelerCryo");
        assert_eq!(good_key("Tamayuratei no Ohanashi"), "TamayurateiNoOhanashi");
        assert_eq!(good_key("Pâte de Fruit"), "PateDeFruit");
        assert_eq!(good_key("\"The Catch\""), "TheCatch");
    }

    #[test]
    fn only_a_good_file_is_read() {
        let file = GoodFile::parse(SAMPLE.as_bytes()).unwrap();
        assert_eq!(file.source, "Irminsul");
        assert_eq!((file.characters.len(), file.weapons.len()), (4, 6));
        assert_eq!(file.materials.as_ref().map(BTreeMap::len), Some(3));

        let bare =
            GoodFile::parse(b"\xEF\xBB\xBF{\"format\": \"GOOD\", \"materials\": {}}").unwrap();
        assert_eq!(bare.materials, None, "an empty bag says nothing");
        for other in [
            &br#"{"format": "UIGF"}"#[..],
            br#"[1, 2]"#,
            b"not json",
            br#"{"info": {}}"#,
        ] {
            assert!(
                matches!(GoodFile::parse(other), Err(Error::NotGoodFile)),
                "{}",
                String::from_utf8_lossy(other)
            );
        }
        // A GOOD file with a value of the wrong kind says what is wrong.
        let wrong = GoodFile::parse(br#"{"format": "GOOD", "characters": [{"key": 3}]}"#);
        assert!(matches!(wrong, Err(Error::Json(_))), "{wrong:?}");
    }

    #[test]
    fn a_first_import_brings_everything_the_data_knows() {
        let file = GoodFile::parse(SAMPLE.as_bytes()).unwrap();
        let inventory = Inventory {
            materials: BTreeMap::from([(MORA, 1_000_000), (material("Guide to Freedom"), 5)]),
            named: BTreeMap::new(),
        };
        let plan = plan_good(&file, &Collection::default(), &inventory, planner());

        let characters: Vec<&OwnedCharacter> = plan
            .collection
            .characters
            .iter()
            .map(Change::after)
            .collect();
        assert_eq!(
            characters
                .iter()
                .map(|c| c.key.as_str())
                .collect::<Vec<_>>(),
            ["ganyu", "skirk", "travelercryo"]
        );
        assert_eq!(characters[0].level, level(80, 6), "ascended at 80");
        assert_eq!(characters[0].constellation, 2);
        assert_eq!(characters[0].talents, [10, 7, 7]);
        assert_eq!(characters[1].level, Level::MAX);

        let weapons: Vec<&OwnedWeapon> =
            plan.collection.weapons.iter().map(Change::after).collect();
        assert_eq!(weapons.len(), 5, "every copy, the unknown bow apart");
        assert_eq!(weapons[0].key, "amosbow");
        assert_eq!(weapons[0].holder.as_deref(), Some("ganyu"));
        assert_eq!(weapons[0].refinement, 2);
        assert_eq!(weapons[1].holder.as_deref(), Some("travelercryo"));
        assert_eq!(weapons[1].level, level(60, 3));
        assert!(weapons[2..].iter().all(|w| w.key == "coolsteel"));

        // Hero's Wit and Teachings of Freedom from the file, the Guide the
        // bag lacks down to 0, the Mora kept.
        let changed: BTreeMap<MaterialId, (u64, u64)> = (plan.materials.iter())
            .map(|c| (c.id, (c.before, c.after)))
            .collect();
        assert_eq!(
            changed,
            BTreeMap::from([
                (104003, (0, 121)),
                (material("Teachings of Freedom"), (0, 62)),
                (material("Guide to Freedom"), (5, 0)),
            ])
        );

        let notes = &plan.collection.notes;
        for note in [
            SyncNote::UnknownGoodCharacter {
                key: "Manekina".to_owned(),
            },
            SyncNote::AboveMaxLevel {
                name: "Skirk".to_owned(),
                level: 100,
            },
            SyncNote::UnknownGoodWeapon {
                key: "ANewBow".to_owned(),
                copies: 1,
            },
            SyncNote::NoMora,
            SyncNote::OtherItems { count: 1 },
        ] {
            assert!(notes.contains(&note), "{note:?} in {notes:?}");
        }
        assert_eq!(plan.changes(), 3 + 5 + 3);
    }

    #[test]
    fn the_same_file_twice_changes_nothing() {
        let file = GoodFile::parse(SAMPLE.as_bytes()).unwrap();
        let first = plan_good(
            &file,
            &Collection::default(),
            &Inventory::default(),
            planner(),
        );
        let (collection, inventory) =
            applied(&first, &Collection::default(), &Inventory::default());
        let second = plan_good(&file, &collection, &inventory, planner());
        assert_eq!(second.changes(), 0, "{second:#?}");
    }

    #[test]
    fn what_the_player_entered_is_raised_never_lowered() {
        let file = GoodFile::parse(SAMPLE.as_bytes()).unwrap();
        let collection = Collection {
            characters: vec![OwnedCharacter {
                level: Level::MAX,
                talents: [9, 9, 9],
                favourite: true,
                ..OwnedCharacter::new("ganyu")
            }],
            weapons: vec![
                // Entered without a holder: Ganyu's in the file.
                OwnedWeapon {
                    id: 1,
                    ..OwnedWeapon::new("amosbow")
                },
                // Above every Cool Steel of the file: kept apart.
                OwnedWeapon {
                    id: 2,
                    level: Level::MAX,
                    ..OwnedWeapon::new("coolsteel")
                },
            ],
        };
        let plan = plan_good(&file, &collection, &Inventory::default(), planner());
        let Change::Raised { after, .. } = &plan.collection.characters[0] else {
            panic!("{:?}", plan.collection.characters[0]);
        };
        assert_eq!(after.level, Level::MAX, "not lowered to 80");
        assert_eq!(after.talents, [10, 9, 9]);
        assert!(after.favourite);
        let Change::Raised { after, .. } = &plan.collection.weapons[0] else {
            panic!("{:?}", plan.collection.weapons[0]);
        };
        assert_eq!(after.id, 1);
        assert_eq!(after.holder.as_deref(), Some("ganyu"));
        assert_eq!(after.level, Level::MAX);
        // Three new Cool Steels; the one at 90 isn't touched.
        let added = (plan.collection.weapons.iter())
            .filter(|c| matches!(c, Change::Added(w) if w.key == "coolsteel"))
            .count();
        assert_eq!(added, 3);
    }

    #[test]
    fn a_weapon_follows_its_holder_in_the_file() {
        // Bennett held the Favonius Sword; in the file, the Traveler does,
        // and Bennett holds nothing the data knows.
        let file = GoodFile::parse(SAMPLE.as_bytes()).unwrap();
        let collection = Collection {
            characters: Vec::new(),
            weapons: vec![OwnedWeapon {
                id: 4,
                level: level(60, 3),
                refinement: 5,
                holder: Some("bennett".to_owned()),
                ..OwnedWeapon::new("favoniussword")
            }],
        };
        let plan = plan_good(&file, &collection, &Inventory::default(), planner());
        let Change::Raised { before, after } = &plan.collection.weapons[1] else {
            panic!("{:?}", plan.collection.weapons[1]);
        };
        assert_eq!(after.id, 4);
        assert_eq!(before.holder.as_deref(), Some("bennett"));
        assert_eq!(after.holder.as_deref(), Some("travelercryo"));
    }

    #[test]
    fn a_file_without_materials_leaves_the_inventory() {
        let mut file = GoodFile::parse(SAMPLE.as_bytes()).unwrap();
        file.materials = None;
        let inventory = Inventory {
            materials: BTreeMap::from([(104003, 9)]),
            named: BTreeMap::new(),
        };
        let plan = plan_good(&file, &Collection::default(), &inventory, planner());
        assert!(plan.materials.is_empty());
        assert!(plan.collection.notes.contains(&SyncNote::NoMaterials));
    }

    #[test]
    fn notes_are_sentences() {
        let notes = [
            SyncNote::UnknownGoodCharacter {
                key: "Manekina".to_owned(),
            },
            SyncNote::UnknownGoodWeapon {
                key: "ANewBow".to_owned(),
                copies: 2,
            },
            SyncNote::OtherItems { count: 879 },
            SyncNote::NoMora,
            SyncNote::NoMaterials,
        ];
        for note in notes {
            let text = note.to_string();
            assert!(text.ends_with('.'), "{text}");
        }
        assert!(
            (SyncNote::UnknownGoodWeapon {
                key: "ANewBow".to_owned(),
                copies: 2,
            })
            .to_string()
            .contains("ANewBow")
        );
    }
}
