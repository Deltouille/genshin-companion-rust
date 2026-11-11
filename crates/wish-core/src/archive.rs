//! The archive: what the character and weapon pages show, from genshin-db
//! (extracted with the planner data by `tools/extract-genshin-db.mjs`).
//! A character's profile, stats at any level, talents with their values at
//! any level, constellations; a weapon's description, ATK and secondary stat
//! at any level, passive at each refinement.
//!
//! Embedded, and overridable from the game-data folder like the other data.
//! Names, rarity, element and costs are in [`crate::planner::PlannerData`],
//! keyed alike.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::data::DataSource;
use crate::error::{Error, Result};
use crate::planner::{Level, MAX_LEVEL};

pub const ARCHIVE_FILE: &str = "archive.json";

const EMBEDDED_ARCHIVE: &str = include_str!("../data/archive.json");

/// The highest talent level the data has values for (10, plus 3 from
/// constellations, plus 2 from rare buffs).
pub const MAX_TALENT_LEVEL: u8 = 15;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CharacterPage {
    /// The planner's key for the same character.
    pub key: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub region: Option<String>,
    pub affiliation: Option<String>,
    pub birthday: Option<String>,
    /// The constellation's name (Sinae Unicornis).
    pub constellation: Option<String>,
    /// The game version that brought the character ("1.2").
    pub version: Option<String>,
    /// The game's file name of the wish art.
    pub splash: Option<String>,
    pub stats: BaseStats,
    pub talents: Vec<Talent>,
    pub constellations: Vec<Constellation>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct BaseStats {
    base: Triple,
    curve: Curves,
    /// The stat ascension raises besides HP, ATK and DEF ("CRIT DMG").
    pub special: String,
    crit_rate: f64,
    crit_dmg: f64,
    /// What each ascension phase adds: HP, ATK, DEF, the special stat.
    promotion: Vec<[f64; 4]>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct Triple {
    hp: f64,
    attack: f64,
    defense: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct Curves {
    hp: String,
    attack: String,
    defense: String,
}

/// A character's stats at a level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    pub hp: f64,
    pub attack: f64,
    pub defense: f64,
    /// The special stat: a fraction (0.384 for 38.4%), or a flat amount for
    /// Elemental Mastery.
    pub special: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Talent {
    /// "Normal Attack", "Elemental Skill", "Alternate Sprint",
    /// "Elemental Burst" or "Passive".
    pub kind: String,
    pub name: String,
    /// Plain text; `**…**` marks a term the game highlights.
    pub description: String,
    pub icon: Option<String>,
    /// Each row's label and value template (`{param1:F1P}`).
    rows: Vec<(String, String)>,
    /// Each parameter's value at talent levels 1 to 15.
    params: BTreeMap<String, Vec<f64>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Constellation {
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
}

/// A weapon's page, keyed like the planner's weapons.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct WeaponPage {
    pub key: String,
    pub description: Option<String>,
    /// The version it came out in ("1.0").
    pub version: Option<String>,
    /// The game's file name of its wish art (`UI_Gacha_EquipIcon_Bow_Amos`).
    pub art: Option<String>,
    pub stats: WeaponStats,
    pub passive: Option<Passive>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct WeaponStats {
    /// ATK at level 1, and its growth curve.
    pub attack: f64,
    pub attack_curve: String,
    /// The secondary stat's name as the game shows it ("CRIT Rate"); 1★ and
    /// 2★ weapons have none.
    pub special: Option<String>,
    pub special_base: f64,
    pub special_curve: Option<String>,
    /// A share (shown as a percentage) for every secondary stat but
    /// Elemental Mastery.
    pub special_percent: bool,
    /// What each ascension phase adds to ATK.
    pub promotion: Vec<f64>,
}

/// A weapon's passive: its name, and its text at refinements 1 to 5.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Passive {
    pub name: String,
    pub refinements: Vec<String>,
}

/// A weapon's ATK and secondary stat at a level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponStatsAt {
    pub attack: f64,
    pub special: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct ArchiveData {
    pub source: DataSource,
    /// Each growth curve's multiplier at levels 1 to 90.
    curves: HashMap<String, Vec<f64>>,
    characters: Vec<CharacterPage>,
    by_key: HashMap<String, usize>,
    weapons: Vec<WeaponPage>,
    weapon_by_key: HashMap<String, usize>,
}

static EMBEDDED: LazyLock<ArchiveData> = LazyLock::new(|| {
    ArchiveData::from_json(EMBEDDED_ARCHIVE)
        .expect("embedded archive data is validated by the extraction script and tests")
});

impl ArchiveData {
    /// The data compiled into this build.
    pub fn embedded() -> &'static ArchiveData {
        &EMBEDDED
    }

    /// Loads `archive.json` from `dir` when present, else the embedded copy.
    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join(ARCHIVE_FILE);
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
            curves: HashMap<String, Vec<f64>>,
            characters: Vec<CharacterPage>,
            // Absent from copies made before weapons had pages.
            #[serde(default)]
            weapons: Vec<WeaponPage>,
        }
        let file: File = serde_json::from_str(json)
            .map_err(|e| Error::GameData(format!("{ARCHIVE_FILE}: {e}")))?;
        let data = Self {
            by_key: file
                .characters
                .iter()
                .enumerate()
                .map(|(i, c)| (c.key.clone(), i))
                .collect(),
            weapon_by_key: (file.weapons.iter().enumerate())
                .map(|(i, w)| (w.key.clone(), i))
                .collect(),
            source: file.source,
            curves: file.curves,
            characters: file.characters,
            weapons: file.weapons,
        };
        data.check()?;
        Ok(data)
    }

    pub fn character(&self, key: &str) -> Option<&CharacterPage> {
        self.by_key.get(key).map(|&i| &self.characters[i])
    }

    pub fn characters(&self) -> &[CharacterPage] {
        &self.characters
    }

    pub fn weapon(&self, key: &str) -> Option<&WeaponPage> {
        self.weapon_by_key.get(key).map(|&i| &self.weapons[i])
    }

    pub fn weapons(&self) -> &[WeaponPage] {
        &self.weapons
    }

    /// `weapon`'s ATK and secondary stat at `level`: base × growth curve at
    /// the level, plus what the ascension phases add to ATK.
    pub fn weapon_stats(&self, weapon: &WeaponPage, level: Level) -> WeaponStatsAt {
        let stats = &weapon.stats;
        let at = usize::from(level.level.clamp(1, MAX_LEVEL)) - 1;
        let curve = |name: &str| self.curves.get(name).map_or(1.0, |values| values[at]);
        let phase = usize::from(level.phase).min(stats.promotion.len().saturating_sub(1));
        WeaponStatsAt {
            attack: stats.attack * curve(&stats.attack_curve)
                + stats.promotion.get(phase).copied().unwrap_or(0.0),
            special: (stats.special_curve.as_deref()).map(|name| stats.special_base * curve(name)),
        }
    }

    /// `character`'s stats at `level`: base × growth curve at the level, plus
    /// what the ascension phases add.
    pub fn stats(&self, character: &CharacterPage, level: Level) -> Stats {
        let stats = &character.stats;
        let at = usize::from(level.level.clamp(1, MAX_LEVEL)) - 1;
        let curve = |name: &str| self.curves[name][at];
        let phase = stats.promotion[usize::from(level.phase).min(stats.promotion.len() - 1)];
        let base_special = match stats.special.as_str() {
            "CRIT Rate" => stats.crit_rate,
            "CRIT DMG" => stats.crit_dmg,
            _ => 0.0,
        };
        Stats {
            hp: stats.base.hp * curve(&stats.curve.hp) + phase[0],
            attack: stats.base.attack * curve(&stats.curve.attack) + phase[1],
            defense: stats.base.defense * curve(&stats.curve.defense) + phase[2],
            special: base_special + phase[3],
        }
    }

    /// Every curve a character uses has 90 levels, every character 7
    /// ascension phases, every talent row parameters known for 15 levels.
    fn check(&self) -> Result<()> {
        let bad = |what: String| Err(Error::GameData(format!("{ARCHIVE_FILE}: {what}")));
        for (name, values) in &self.curves {
            if values.len() != usize::from(MAX_LEVEL) {
                return bad(format!("curve {name} has {} levels", values.len()));
            }
        }
        for character in &self.characters {
            let stats = &character.stats;
            for curve in [&stats.curve.hp, &stats.curve.attack, &stats.curve.defense] {
                if !self.curves.contains_key(curve) {
                    return bad(format!("{} uses unknown curve {curve}", character.key));
                }
            }
            if stats.promotion.len() != 7 {
                return bad(format!(
                    "{} has {} ascension phases",
                    character.key,
                    stats.promotion.len()
                ));
            }
            for talent in &character.talents {
                for (_, template) in &talent.rows {
                    for (param, _) in placeholders(template) {
                        match talent.params.get(param) {
                            Some(values) if values.len() == usize::from(MAX_TALENT_LEVEL) => {}
                            _ => {
                                return bad(format!(
                                    "{}'s {} lacks {param}",
                                    character.key, talent.name
                                ));
                            }
                        }
                    }
                }
            }
        }
        for weapon in &self.weapons {
            let stats = &weapon.stats;
            let curves = std::iter::once(&stats.attack_curve).chain(&stats.special_curve);
            for curve in curves {
                if !self.curves.contains_key(curve) {
                    return bad(format!("weapon {} uses unknown curve {curve}", weapon.key));
                }
            }
            if !(5..=7).contains(&stats.promotion.len()) {
                return bad(format!(
                    "weapon {} has {} ascension phases",
                    weapon.key,
                    stats.promotion.len()
                ));
            }
        }
        Ok(())
    }
}

impl Talent {
    /// Each row's label and value at talent `level` (1 to 15), formatted as
    /// the game shows them: "1-Hit DMG", "31.7%".
    pub fn values(&self, level: u8) -> Vec<(String, String)> {
        let at = usize::from(level.clamp(1, MAX_TALENT_LEVEL)) - 1;
        self.rows
            .iter()
            .map(|(label, template)| {
                let mut value = String::new();
                let mut rest = template.as_str();
                for (param, format) in placeholders(template) {
                    let token = format!("{{{param}:{format}}}");
                    let start = rest.find(&token).expect("found by `placeholders`");
                    value.push_str(&rest[..start]);
                    value.push_str(&format_value(self.params[param][at], format));
                    rest = &rest[start + token.len()..];
                }
                value.push_str(rest);
                (label.clone(), value)
            })
            .collect()
    }

    /// Whether the talent levels up (passives do not).
    pub fn has_levels(&self) -> bool {
        !self.rows.is_empty()
    }
}

/// The `{param3:F1}` placeholders of a template, as (`param3`, `F1`).
fn placeholders(template: &str) -> impl Iterator<Item = (&str, &str)> {
    template.split('{').skip(1).filter_map(|part| {
        let inner = part.split('}').next()?;
        inner.split_once(':')
    })
}

/// A value in one of the game's formats: `P` a whole percentage, `F1P` /
/// `F2P` a percentage with 1 or 2 decimals, `F1` / `F2` a number with up to 1
/// or 2 decimals, `I` a whole number.
pub fn format_value(value: f64, format: &str) -> String {
    let percent = format.ends_with('P');
    let digits = format
        .strip_prefix('F')
        .and_then(|f| f.trim_end_matches('P').parse::<usize>().ok())
        .unwrap_or(0);
    if percent {
        format!("{:.digits$}%", value * 100.0)
    } else {
        let text = format!("{value:.digits$}");
        if text.contains('.') {
            text.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            text
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_format_like_the_game() {
        assert_eq!(format_value(0.31734, "F1P"), "31.7%");
        assert_eq!(format_value(1.24, "P"), "124%");
        assert_eq!(format_value(2.304, "P"), "230%");
        assert_eq!(format_value(6.0, "F1"), "6");
        assert_eq!(format_value(1.5, "F1"), "1.5");
        assert_eq!(format_value(12.0, "I"), "12");
        assert_eq!(format_value(0.0456, "F2P"), "4.56%");
    }
}
