//! How much Original Resin farming what a plan still lacks takes, on average,
//! as genshin-center's planner estimates it ("Average Estimated Resin"): each
//! farmable material counted at what one run gives on average, rounded up to
//! whole runs.
//!
//! - Talent books (Domains of Mastery) and weapon ascension materials
//!   (Domains of Forgery), 20 resin a run: a run gives every tier of one
//!   series, and lower tiers craft up three for one, so a series is counted in
//!   its lowest tier (a 3★ book is three 2★ ones) against what a run gives of
//!   the tiers still needed, at the domain's highest level (IV).
//! - Normal boss materials, 40 resin a run, each boss apart, by world level.
//! - Character EXP (Blossoms of Revelation) and Mora (Blossoms of Wealth), 20
//!   resin a run, by world level.
//!
//! Left out, as genshin-center does: weekly boss materials (three runs a week
//! at half price, not a daily budget), elemental gems (boss runs give them by
//! the way, and the alchemy table converts them), weapon EXP ores (forged,
//! not farmed with resin), enemy drops and local specialties (no resin).
//!
//! The averages are those the Genshin Impact wiki publishes, gathered by the
//! community (the Genshin Data Gathering team), checked against
//! genshin-center's in November 2026.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{EXP_BOOKS, MORA, Material, MaterialId, PlannerData};

/// The Original Resin that regenerates in a day: one every 8 minutes.
pub const RESIN_PER_DAY: u32 = 180;
/// The highest world level.
pub const MAX_WORLD_LEVEL: u8 = 9;

/// What a Domain of Mastery run gives at level IV on average: 2★, 3★ and 4★
/// talent books.
const TALENT_DOMAIN: [f64; 3] = [2.2, 1.98, 0.22];
/// What a Domain of Forgery run gives at level IV on average: 2★ to 5★
/// weapon ascension materials.
const WEAPON_DOMAIN: [f64; 4] = [2.2, 2.418, 0.62, 0.062];
/// A normal boss's material per run on average, by world level 0 to 9. At
/// world level 9 a run gives at least 3, counted as 3.
const BOSS_DROPS: [f64; 10] = [
    1.6185, 1.6185, 1.7037, 1.8741, 2.0445, 2.2149, 2.3852, 2.5556, 2.5556, 3.0,
];
/// A Blossom of Wealth's Mora, by world level.
const LEY_LINE_MORA: [u64; 10] = [
    12_000, 20_000, 28_000, 36_000, 44_000, 52_000, 60_000, 60_000, 60_000, 60_000,
];
/// A Blossom of Revelation's books, in EXP on average, by world level.
const LEY_LINE_EXP: [u64; 10] = [
    25_000, 38_500, 52_500, 67_500, 82_500, 102_500, 122_500, 122_500, 122_500, 122_500,
];

/// Where a material is farmed with resin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResinSource {
    TalentBooks,
    WeaponMaterials,
    BossMaterials,
    CharacterExp,
    Mora,
}

impl ResinSource {
    pub const ALL: [ResinSource; 5] = [
        ResinSource::TalentBooks,
        ResinSource::WeaponMaterials,
        ResinSource::BossMaterials,
        ResinSource::CharacterExp,
        ResinSource::Mora,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ResinSource::TalentBooks => "Talent books",
            ResinSource::WeaponMaterials => "Weapon materials",
            ResinSource::BossMaterials => "Boss materials",
            ResinSource::CharacterExp => "Character EXP",
            ResinSource::Mora => "Mora",
        }
    }

    /// Where it is farmed, as the game names it.
    pub fn place(self) -> &'static str {
        match self {
            ResinSource::TalentBooks => "Domains of Mastery",
            ResinSource::WeaponMaterials => "Domains of Forgery",
            ResinSource::BossMaterials => "Normal bosses",
            ResinSource::CharacterExp => "Blossoms of Revelation",
            ResinSource::Mora => "Blossoms of Wealth",
        }
    }

    /// What one run costs.
    pub fn resin_per_run(self) -> u64 {
        match self {
            ResinSource::BossMaterials => 40,
            _ => 20,
        }
    }

    /// Where `material` is farmed, if with resin. Talent books are the
    /// talent materials a domain gives (not the Crown of Insight); boss
    /// materials the 4★ level-up ones (the weekly bosses' are 5★).
    fn of(material: &Material) -> Option<ResinSource> {
        match material.kind.as_str() {
            "Character Talent Material" if !material.days.is_empty() => {
                Some(ResinSource::TalentBooks)
            }
            "Weapon Ascension Material" => Some(ResinSource::WeaponMaterials),
            "Character Level-Up Material" if material.rarity == 4 => {
                Some(ResinSource::BossMaterials)
            }
            _ => None,
        }
    }
}

/// How the estimate counts: the player's world level, the resin they spend a
/// day, and the sources they farm otherwise (Mora from events, say).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResinOptions {
    pub world_level: u8,
    pub resin_per_day: u32,
    /// The sources left out of the estimate.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub ignored: BTreeSet<ResinSource>,
}

impl Default for ResinOptions {
    /// World level 8, where most players who plan spend their resin, the
    /// resin of a day, every source counted.
    fn default() -> Self {
        Self {
            world_level: 8,
            resin_per_day: RESIN_PER_DAY,
            ignored: BTreeSet::new(),
        }
    }
}

impl ResinOptions {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Whether `source` is counted.
    pub fn counts(&self, source: ResinSource) -> bool {
        !self.ignored.contains(&source)
    }

    /// The world level, within the game's.
    fn level(&self) -> usize {
        usize::from(self.world_level.min(MAX_WORLD_LEVEL))
    }
}

/// The runs one source takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceRuns {
    pub source: ResinSource,
    pub runs: u64,
}

impl SourceRuns {
    pub fn resin(&self) -> u64 {
        self.runs * self.source.resin_per_run()
    }
}

/// The resin farming something takes, by source.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResinEstimate {
    /// The sources counted that take runs, in [`ResinSource::ALL`]'s order.
    pub sources: Vec<SourceRuns>,
}

impl ResinEstimate {
    pub fn resin(&self) -> u64 {
        self.sources.iter().map(SourceRuns::resin).sum()
    }

    /// The days it takes at `resin_per_day`.
    pub fn days(&self, resin_per_day: u32) -> f64 {
        self.resin() as f64 / f64::from(resin_per_day.max(1))
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// The runs of `source`, 0 when it takes none or isn't counted.
    pub fn runs(&self, source: ResinSource) -> u64 {
        (self.sources.iter())
            .find(|s| s.source == source)
            .map_or(0, |s| s.runs)
    }
}

/// A domain series still lacking: in its lowest tier, and the highest tier
/// it lacks.
#[derive(Default)]
struct Series {
    lowest_tiers: f64,
    highest: usize,
}

impl PlannerData {
    /// The resin farming `missing` takes (what an entry or the plan still
    /// lacks: known materials, EXP as books), on average.
    pub fn resin_estimate(
        &self,
        missing: &BTreeMap<MaterialId, u64>,
        options: &ResinOptions,
    ) -> ResinEstimate {
        let level = options.level();
        let (mut mora, mut exp) = (0, 0);
        let mut books: BTreeMap<MaterialId, Series> = BTreeMap::new();
        let mut weapon: BTreeMap<MaterialId, Series> = BTreeMap::new();
        let mut boss_runs = 0;
        for (&id, &count) in missing {
            if count == 0 {
                continue;
            }
            if id == MORA {
                mora += count;
                continue;
            }
            if let Some((_, gives)) = EXP_BOOKS.iter().find(|(book, _)| *book == id) {
                exp += count * gives;
                continue;
            }
            let Some(material) = self.material(id) else {
                continue;
            };
            let series = match ResinSource::of(material) {
                Some(ResinSource::BossMaterials) => {
                    boss_runs += runs(count as f64, BOSS_DROPS[level]);
                    continue;
                }
                Some(ResinSource::TalentBooks) => &mut books,
                Some(ResinSource::WeaponMaterials) => &mut weapon,
                _ => continue,
            };
            // A series by its lowest tier; the tiers apart from it by rarity.
            let lowest = self
                .craft_family(id)
                .and_then(|family| family.first())
                .copied()
                .unwrap_or(id);
            let tier = self
                .material(lowest)
                .map_or(0, |m| usize::from(material.rarity.saturating_sub(m.rarity)));
            let series = series.entry(lowest).or_default();
            series.lowest_tiers += count as f64 * 3f64.powi(tier as i32);
            series.highest = series.highest.max(tier);
        }

        let domain_runs = |series: &BTreeMap<MaterialId, Series>, drops: &[f64]| {
            (series.values())
                .map(|series| {
                    // What a run gives of the tiers up to the highest lacking,
                    // in the lowest: a run's higher tiers aren't needed.
                    let per_run: f64 = (drops.iter().enumerate())
                        .take(series.highest + 1)
                        .map(|(tier, drops)| drops * 3f64.powi(tier as i32))
                        .sum();
                    runs(series.lowest_tiers, per_run)
                })
                .sum::<u64>()
        };
        let all = [
            (
                ResinSource::TalentBooks,
                domain_runs(&books, &TALENT_DOMAIN),
            ),
            (
                ResinSource::WeaponMaterials,
                domain_runs(&weapon, &WEAPON_DOMAIN),
            ),
            (ResinSource::BossMaterials, boss_runs),
            (ResinSource::CharacterExp, exp.div_ceil(LEY_LINE_EXP[level])),
            (ResinSource::Mora, mora.div_ceil(LEY_LINE_MORA[level])),
        ];
        ResinEstimate {
            sources: all
                .into_iter()
                .filter(|&(source, runs)| runs > 0 && options.counts(source))
                .map(|(source, runs)| SourceRuns { source, runs })
                .collect(),
        }
    }
}

/// The runs that give `needed` at `per_run` on average, rounded up. A hair is
/// taken off first, so 20 needed at 2.5 a run is 8 runs, not 9 from rounding.
fn runs(needed: f64, per_run: f64) -> u64 {
    if needed <= 0. || per_run <= 0. {
        return 0;
    }
    (needed / per_run - 1e-9).ceil() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{CharacterGoal, WeaponGoal};

    fn missing_of(requirement: crate::planner::Requirement) -> BTreeMap<MaterialId, u64> {
        requirement.materials_with_exp()
    }

    /// A character from scratch to 90 with every talent at 10, at world
    /// level 8: 9 / 63 / 114 books are 1,224 2★ ones at 10.12 a run; 46 boss
    /// materials at 2.5556; 8,362,650 EXP as 419 Hero's Wit at 122,500 a run.
    #[test]
    fn a_full_character_takes_its_known_runs() {
        let planner = PlannerData::embedded();
        let ganyu = planner.character("ganyu").unwrap();
        let missing = missing_of(planner.character_cost(ganyu, &CharacterGoal::FULL));
        let options = ResinOptions::default();
        let estimate = planner.resin_estimate(&missing, &options);
        assert_eq!(estimate.runs(ResinSource::TalentBooks), 121);
        assert_eq!(estimate.runs(ResinSource::BossMaterials), 18);
        assert_eq!(estimate.runs(ResinSource::CharacterExp), 69);
        let mora = missing[&MORA];
        assert_eq!(
            estimate.runs(ResinSource::Mora),
            mora.div_ceil(60_000),
            "{mora} Mora"
        );
        assert_eq!(estimate.runs(ResinSource::WeaponMaterials), 0);
        assert_eq!(
            estimate.resin(),
            (121 + 69 + mora.div_ceil(60_000)) * 20 + 18 * 40
        );
        assert!((estimate.days(180) - estimate.resin() as f64 / 180.).abs() < 1e-9);

        // World level 9's bosses give 3; a source left out isn't counted.
        let options = ResinOptions {
            world_level: 9,
            ignored: BTreeSet::from([ResinSource::Mora]),
            ..ResinOptions::default()
        };
        let estimate = planner.resin_estimate(&missing, &options);
        assert_eq!(estimate.runs(ResinSource::BossMaterials), 16);
        assert_eq!(estimate.runs(ResinSource::Mora), 0);
        assert!(
            estimate
                .sources
                .iter()
                .all(|s| s.source != ResinSource::Mora)
        );
    }

    #[test]
    fn a_weapon_counts_its_domain_series_in_its_lowest_tier() {
        let planner = PlannerData::embedded();
        let amos = planner.weapon("amosbow").unwrap();
        let missing = missing_of(planner.weapon_cost(amos, &WeaponGoal::full(5)));
        let estimate = planner.resin_estimate(&missing, &ResinOptions::default());
        // A 5★ weapon to 90: 5 / 14 / 14 / 6 of its domain's series, 5 + 42 +
        // 126 + 162 = 335 in 2★ ones, at 16.708 a run.
        assert_eq!(estimate.runs(ResinSource::WeaponMaterials), 21);
        // Its EXP is ores, which take no resin; no talent books nor bosses.
        assert_eq!(estimate.runs(ResinSource::CharacterExp), 0);
        assert_eq!(estimate.runs(ResinSource::TalentBooks), 0);
        assert_eq!(estimate.runs(ResinSource::BossMaterials), 0);
    }

    #[test]
    fn only_the_tiers_still_lacking_set_a_run_s_worth() {
        let planner = PlannerData::embedded();
        let family = planner
            .craft_families()
            .iter()
            .find(|family| {
                planner
                    .material(family[0])
                    .is_some_and(|m| ResinSource::of(m) == Some(ResinSource::TalentBooks))
            })
            .unwrap()
            .to_vec();
        let options = ResinOptions::default();
        // 22 green books alone: 2.2 a run, 10 runs.
        let greens = BTreeMap::from([(family[0], 22)]);
        let estimate = planner.resin_estimate(&greens, &options);
        assert_eq!(estimate.runs(ResinSource::TalentBooks), 10);
        // 2 purple ones: 18 green at 10.12 a run, 2 runs.
        let purples = BTreeMap::from([(family[2], 2)]);
        let estimate = planner.resin_estimate(&purples, &options);
        assert_eq!(estimate.runs(ResinSource::TalentBooks), 2);
        // Weekly boss materials and gems take no counted resin.
        assert!(
            planner
                .resin_estimate(&BTreeMap::new(), &options)
                .is_empty()
        );
    }

    #[test]
    fn options_read_with_defaults_for_what_is_missing() {
        let options: ResinOptions = serde_json::from_str(r#"{"world_level": 9}"#).unwrap();
        assert_eq!(options.world_level, 9);
        assert_eq!(options.resin_per_day, RESIN_PER_DAY);
        let json = serde_json::to_string(&ResinOptions {
            ignored: BTreeSet::from([ResinSource::CharacterExp]),
            ..ResinOptions::default()
        })
        .unwrap();
        assert!(json.contains(r#""ignored":["character-exp"]"#), "{json}");
    }
}
