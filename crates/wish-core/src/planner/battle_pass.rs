//! The Battle Pass: what its levels still to come bring the plan.
//!
//! Each game version has a pass of 50 levels, with a free track (the
//! Sojourner's Battle Pass) and a second one bought with the Gnostic Hymn. In
//! the game, the player picks a reward plan, three reward types of four (Mora,
//! Character EXP, Weapon EXP, Artifact EXP), and the rewards follow it. They
//! have been the same in every period: `battle_pass.json`, written from the
//! Genshin Impact Wiki by `tools/extract-battle-pass.mjs`, keeps what the
//! planner counts of them (Mora, EXP books, enhancement ores, and the boxes
//! whose talent books or local specialties the player picks).
//!
//! A pass lasts one period: [`BattlePassData::period_days`] after one starts,
//! the next starts at 0. A level entered in an earlier period counts as 0
//! ([`BattlePass::level_at`]).
//!
//! The planner counts the rewards still to come as if they were in the bag
//! ([`PlannerData::allocate_with`]): each kind of box goes to the series the
//! player picked for it ([`BoxPicks`]), or else to the series the plan lacks,
//! in its order.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use chrono::{DateTime, FixedOffset, TimeDelta};
use serde::Deserialize;

use super::{Allocation, Inventory, MaterialId, PlanEntry, PlannerData, Role};
use crate::error::{Error, Result};
use crate::model::text_enum;

const EMBEDDED: &str = include_str!("../../data/battle_pass.json");

/// A pass's highest level.
pub const BATTLE_PASS_LEVELS: u8 = 50;

text_enum! {
    /// The reward plan picked in the game: three reward types of the four,
    /// named after the one left out.
    pub enum RewardPlan {
        NoArtifactExp => "no-artifact-exp",
        NoWeaponExp => "no-weapon-exp",
        NoCharacterExp => "no-character-exp",
        NoMora => "no-mora",
    }
}

impl Default for RewardPlan {
    /// Mora, Character EXP and Weapon EXP: the planner's three.
    fn default() -> Self {
        RewardPlan::NoArtifactExp
    }
}

/// A player's pass, as they entered it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BattlePass {
    /// The level reached in `period`, 0 to [`BATTLE_PASS_LEVELS`].
    pub level: u8,
    /// The start of the period the level was entered in.
    pub period: DateTime<FixedOffset>,
    /// With the Gnostic Hymn (or the Gnostic Chorus, which has its rewards).
    pub premium: bool,
    pub plan: RewardPlan,
    /// Whether the planner counts the rewards still to come.
    pub counted: bool,
    /// The series picked for each kind of box.
    pub picks: BoxPicks,
}

impl BattlePass {
    /// A pass at level 0 in the period of `now`: free, the default plan, not
    /// counted.
    pub fn new(data: &BattlePassData, now: DateTime<FixedOffset>) -> Self {
        Self {
            level: 0,
            period: data.period_at(now),
            premium: false,
            plan: RewardPlan::default(),
            counted: false,
            picks: BoxPicks::default(),
        }
    }

    /// The level at `now`: the one entered, or 0 once a later period started.
    pub fn level_at(&self, data: &BattlePassData, now: DateTime<FixedOffset>) -> u8 {
        if self.period == data.period_at(now) {
            self.level.min(BATTLE_PASS_LEVELS)
        } else {
            0
        }
    }

    /// The pass at `level` in the period of `now`, the rest kept.
    #[must_use = "it returns the changed copy; the original is unchanged"]
    pub fn at_level(self, level: u8, data: &BattlePassData, now: DateTime<FixedOffset>) -> Self {
        Self {
            level: level.min(BATTLE_PASS_LEVELS),
            period: data.period_at(now),
            ..self
        }
    }

    /// What the levels still to come at `now` give.
    pub fn remaining(
        &self,
        data: &BattlePassData,
        now: DateTime<FixedOffset>,
    ) -> BattlePassRewards {
        data.remaining(self.level_at(data, now), self.premium, self.plan)
    }
}

/// A box whose content the player picks when they open it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BoxKind {
    /// "Guides of a Journey": talent books of the middle tier (Guide to …).
    Guides,
    /// "Philosophies of a Journey": talent books of the top tier.
    Philosophies,
    /// "Traveling Bounties": a stack of one local specialty.
    Specialties,
}

impl BoxKind {
    pub const ALL: [BoxKind; 3] = [BoxKind::Guides, BoxKind::Philosophies, BoxKind::Specialties];
}

/// The material the player picked for each kind of box: a Guide (a talent
/// book series' middle tier), a Philosophies (its top tier), a local
/// specialty. `None`: the boxes go to what the plan lacks, in its order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoxPicks {
    pub guides: Option<MaterialId>,
    pub philosophies: Option<MaterialId>,
    pub specialties: Option<MaterialId>,
}

impl BoxPicks {
    /// The pick for `kind`.
    pub fn of(self, kind: BoxKind) -> Option<MaterialId> {
        match kind {
            BoxKind::Guides => self.guides,
            BoxKind::Philosophies => self.philosophies,
            BoxKind::Specialties => self.specialties,
        }
    }

    /// These picks with `pick` for `kind`.
    #[must_use = "it returns the changed copy; the original is unchanged"]
    pub fn with(self, kind: BoxKind, pick: Option<MaterialId>) -> Self {
        match kind {
            BoxKind::Guides => Self {
                guides: pick,
                ..self
            },
            BoxKind::Philosophies => Self {
                philosophies: pick,
                ..self
            },
            BoxKind::Specialties => Self {
                specialties: pick,
                ..self
            },
        }
    }
}

/// How many talent books or specialties each box gives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct BoxSizes {
    pub guides: u64,
    pub philosophies: u64,
    pub specialties: u64,
}

impl BoxSizes {
    pub fn of(self, kind: BoxKind) -> u64 {
        match kind {
            BoxKind::Guides => self.guides,
            BoxKind::Philosophies => self.philosophies,
            BoxKind::Specialties => self.specialties,
        }
    }
}

/// One reward of a level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
enum Reward {
    Material {
        material: MaterialId,
        count: u64,
    },
    Box {
        #[serde(rename = "box")]
        kind: BoxKind,
        count: u64,
    },
}

/// What one level gives on each track.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct Level {
    free: Vec<Reward>,
    /// With the Gnostic Hymn, besides the free track's.
    premium: Vec<Reward>,
}

/// Where the rewards were read.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct BattlePassSource {
    pub wiki: String,
    pub page: String,
    pub revision: u64,
}

/// The pass's rewards per level and reward plan, and its periods.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BattlePassData {
    pub source: BattlePassSource,
    /// The start of a known period; the others are whole numbers of
    /// [`BattlePassData::period_days`] away.
    period_start: DateTime<FixedOffset>,
    /// How long a period lasts, in days.
    pub period_days: u32,
    pub boxes: BoxSizes,
    plans: BTreeMap<RewardPlan, Vec<Level>>,
}

static EMBEDDED_DATA: LazyLock<BattlePassData> = LazyLock::new(|| {
    BattlePassData::from_json(EMBEDDED)
        .expect("the embedded Battle Pass data is checked by its extraction script and the tests")
});

impl BattlePassData {
    /// The data compiled into this build.
    pub fn embedded() -> &'static BattlePassData {
        &EMBEDDED_DATA
    }

    /// Reads a `battle_pass.json`: every reward plan, each with its 50
    /// levels, the boxes' sizes and a period's start.
    pub fn from_json(json: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct File {
            source: BattlePassSource,
            period_start: String,
            period_days: u32,
            boxes: BoxSizes,
            plans: BTreeMap<String, Vec<Level>>,
        }
        let invalid = |problem: String| Error::GameData(format!("battle_pass.json: {problem}"));
        let file: File = serde_json::from_str(json).map_err(|e| invalid(e.to_string()))?;
        let period_start = DateTime::parse_from_rfc3339(&file.period_start)
            .map_err(|e| invalid(format!("period_start: {e}")))?;
        if file.period_days == 0 {
            return Err(invalid("period_days is 0".to_owned()));
        }
        let mut plans = BTreeMap::new();
        for (name, levels) in file.plans {
            let plan: RewardPlan = name.parse().map_err(|_| invalid(format!("plan {name}")))?;
            if levels.len() != usize::from(BATTLE_PASS_LEVELS) {
                return Err(invalid(format!("plan {name} has {} levels", levels.len())));
            }
            plans.insert(plan, levels);
        }
        if let Some(plan) = RewardPlan::ALL.iter().find(|p| !plans.contains_key(p)) {
            return Err(invalid(format!("no plan {plan}")));
        }
        Ok(Self {
            source: file.source,
            period_start,
            period_days: file.period_days,
            boxes: file.boxes,
            plans,
        })
    }

    fn period(&self) -> TimeDelta {
        TimeDelta::days(i64::from(self.period_days))
    }

    /// The start of the period `now` is in.
    pub fn period_at(&self, now: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
        let since = (now - self.period_start).num_seconds();
        let periods = since.div_euclid(self.period().num_seconds());
        // A period count never overflows a few centuries of days.
        let periods = i32::try_from(periods).unwrap_or(0);
        self.period_start + self.period() * periods
    }

    /// When the period `now` is in gives way to the next.
    pub fn next_period(&self, now: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
        self.period_at(now) + self.period()
    }

    /// What the levels after `level` give on the free track, and the Gnostic
    /// Hymn's too when `premium`, with the reward `plan`.
    pub fn remaining(&self, level: u8, premium: bool, plan: RewardPlan) -> BattlePassRewards {
        let mut rewards = BattlePassRewards {
            from_level: level.min(BATTLE_PASS_LEVELS) + 1,
            premium,
            sizes: self.boxes,
            ..BattlePassRewards::default()
        };
        let levels = self.plans.get(&plan).map(Vec::as_slice).unwrap_or(&[]);
        for step in levels.iter().skip(usize::from(level)) {
            let premium_rewards = if premium {
                step.premium.as_slice()
            } else {
                &[]
            };
            for reward in step.free.iter().chain(premium_rewards) {
                match *reward {
                    Reward::Material { material, count } => {
                        *rewards.materials.entry(material).or_default() += count;
                    }
                    Reward::Box { kind, count } => *rewards.boxes.entry(kind).or_default() += count,
                }
            }
        }
        rewards
    }
}

/// What a pass's levels still to come give the planner.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BattlePassRewards {
    /// The first level still to come; past [`BATTLE_PASS_LEVELS`] when none is.
    pub from_level: u8,
    pub premium: bool,
    /// Mora, EXP books and enhancement ores, by id.
    pub materials: BTreeMap<MaterialId, u64>,
    /// The boxes whose content the player picks, how many of each kind.
    pub boxes: BTreeMap<BoxKind, u64>,
    /// What a box of each kind gives.
    pub sizes: BoxSizes,
}

impl BattlePassRewards {
    /// How many boxes of `kind`.
    pub fn boxes_of(&self, kind: BoxKind) -> u64 {
        self.boxes.get(&kind).copied().unwrap_or(0)
    }

    /// Whether the levels still to come give the planner nothing.
    pub fn is_empty(&self) -> bool {
        self.materials.is_empty() && self.boxes.values().all(|&n| n == 0)
    }
}

/// What the pass brings a plan: its rewards, and which series the boxes go
/// to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BattlePassShare {
    pub rewards: BattlePassRewards,
    /// What each kind of box becomes: the materials picked and how many.
    pub picked: BTreeMap<BoxKind, BTreeMap<MaterialId, u64>>,
    /// The boxes left over once the plan lacks nothing they could give: for
    /// the player to pick as they like.
    pub left: BTreeMap<BoxKind, u64>,
}

impl BattlePassShare {
    /// Every material the boxes become, and how many.
    pub fn picked_materials(&self) -> BTreeMap<MaterialId, u64> {
        let mut all = BTreeMap::new();
        for (&id, &count) in self.picked.values().flatten() {
            *all.entry(id).or_default() += count;
        }
        all
    }
}

impl PlannerData {
    /// What a box of `kind` can give, in the game's inventory order: each
    /// talent book series' Guide or Philosophies, or every local specialty;
    /// none of the placeholders custom characters take.
    pub fn box_choices(&self, kind: BoxKind) -> Vec<MaterialId> {
        let known = |id: &MaterialId| self.material(*id).is_some_and(|m| !m.is_placeholder());
        let books = (self.families(Role::TalentBook).iter()).filter(|series| known(&series[0]));
        let choices: Vec<MaterialId> = match kind {
            BoxKind::Guides => books
                .filter(|series| series.len() == 3)
                .map(|series| series[1])
                .collect(),
            BoxKind::Philosophies => books
                .filter(|series| series.len() == 3)
                .map(|series| series[2])
                .collect(),
            BoxKind::Specialties => self
                .families(Role::LocalSpecialty)
                .iter()
                .flatten()
                .copied()
                .collect(),
        };
        choices.into_iter().filter(known).collect()
    }

    /// Shares `inventory` out over `plan`, as [`PlannerData::allocate`] does,
    /// with the pass's `rewards` still to come counted as if they were in it:
    /// its Mora, books and ores as they are, and each kind of box given to the
    /// material `picks` names for it, or else to the series the plan lacks,
    /// in the plan's order. A pick a box can't give (game data that changed)
    /// counts as none.
    pub fn allocate_with(
        &self,
        plan: &[PlanEntry],
        inventory: &Inventory,
        rewards: &BattlePassRewards,
        picks: BoxPicks,
    ) -> (Allocation, BattlePassShare) {
        let mut bag = inventory.clone();
        for (&id, &count) in &rewards.materials {
            *bag.materials.entry(id).or_default() += count;
        }
        let first = self.allocate(plan, &bag);

        // What the plan lacks, each material in the order the plan first
        // lacks it.
        let mut order: Vec<MaterialId> = Vec::new();
        for share in &first.entries {
            for &id in share.missing.materials.keys() {
                if !order.contains(&id) {
                    order.push(id);
                }
            }
        }
        let mut lack: BTreeMap<MaterialId, u64> = first.missing.materials.clone();
        let mut share = BattlePassShare {
            rewards: rewards.clone(),
            ..BattlePassShare::default()
        };
        let mut boxes = rewards.boxes.clone();
        let mut give =
            |kind: BoxKind, id: MaterialId, need: u64, share: &mut BattlePassShare| -> u64 {
                let size = rewards.sizes.of(kind);
                let left = boxes.entry(kind).or_default();
                let mut given = 0;
                while *left > 0 && size > 0 && given < need {
                    *left -= 1;
                    given += size;
                }
                if given > 0 {
                    *share.picked.entry(kind).or_default().entry(id).or_default() += given;
                }
                given
            };
        let pick = |kind: BoxKind| {
            picks
                .of(kind)
                .filter(|id| self.box_choices(kind).contains(id))
        };

        // Talent books, series by series: the top tier from the Philosophies
        // boxes, then from the Guides boxes what the middle tier lacks and
        // what crafting the top tier still needs, three for one.
        let books = self.families(Role::TalentBook);
        let series_of = |id: MaterialId| books.iter().find(|series| series.contains(&id));
        let mut seen: Vec<&Vec<MaterialId>> = Vec::new();
        for id in &order {
            if let Some(series) = series_of(*id)
                && series.len() == 3
                && !seen.contains(&series)
            {
                seen.push(series);
            }
        }
        // A series picked takes every box of its kind.
        let philosophies_picked: Vec<MaterialId> = match pick(BoxKind::Philosophies) {
            Some(id) => vec![id],
            None => seen.iter().map(|series| series[2]).collect(),
        };
        for philosophies in philosophies_picked {
            let need = match pick(BoxKind::Philosophies) {
                Some(_) => u64::MAX,
                None => lack.get(&philosophies).copied().unwrap_or(0),
            };
            let given = give(BoxKind::Philosophies, philosophies, need, &mut share);
            if let Some(lacking) = lack.get_mut(&philosophies) {
                *lacking = lacking.saturating_sub(given);
            }
        }
        match pick(BoxKind::Guides) {
            Some(guide) => {
                give(BoxKind::Guides, guide, u64::MAX, &mut share);
            }
            None => {
                for series in &seen {
                    let (guide, philosophies) = (series[1], series[2]);
                    let need = lack.get(&guide).copied().unwrap_or(0)
                        + 3 * lack.get(&philosophies).copied().unwrap_or(0);
                    give(BoxKind::Guides, guide, need, &mut share);
                }
            }
        }

        // Local specialties, a stack of one each.
        match pick(BoxKind::Specialties) {
            Some(id) => {
                give(BoxKind::Specialties, id, u64::MAX, &mut share);
            }
            None => {
                let specialties = self.box_choices(BoxKind::Specialties);
                for id in order.iter().filter(|id| specialties.contains(id)) {
                    let need = lack.get(id).copied().unwrap_or(0);
                    give(BoxKind::Specialties, *id, need, &mut share);
                }
            }
        }

        share.left = boxes.into_iter().filter(|(_, n)| *n > 0).collect();
        for (id, count) in share.picked_materials() {
            *bag.materials.entry(id).or_default() += count;
        }
        (self.allocate(plan, &bag), share)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{CharacterGoal, CustomItem, EXP_BOOKS, EXP_ORES, MORA, PlanTarget};

    fn data() -> &'static BattlePassData {
        BattlePassData::embedded()
    }

    fn at(time: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(time).unwrap()
    }

    #[test]
    fn a_whole_pass_gives_the_wiki_s_totals() {
        let heros_wit = EXP_BOOKS[0].0;
        let adventurers = EXP_BOOKS[1].0;
        let ore = EXP_ORES[0].0;
        let free = data().remaining(0, false, RewardPlan::NoArtifactExp);
        assert_eq!(free.materials[&MORA], 540_000);
        assert_eq!(free.materials[&adventurers], 48);
        assert_eq!(free.materials[&heros_wit], 42);
        assert_eq!(free.materials[&ore], 108);
        assert!(free.boxes.is_empty(), "{:?}", free.boxes);
        assert_eq!(free.from_level, 1);

        let premium = data().remaining(0, true, RewardPlan::NoArtifactExp);
        assert_eq!(premium.materials[&MORA], 540_000 + 1_620_000);
        assert_eq!(premium.materials[&heros_wit], 42 + 159);
        assert_eq!(premium.materials[&ore], 108 + 324);
        assert_eq!(premium.boxes_of(BoxKind::Guides), 7);
        assert_eq!(premium.boxes_of(BoxKind::Philosophies), 2);
        assert_eq!(premium.boxes_of(BoxKind::Specialties), 2);

        // A plan without Mora gives none; one without weapon EXP no ore.
        assert!(
            !data()
                .remaining(0, true, RewardPlan::NoMora)
                .materials
                .contains_key(&MORA)
        );
        assert!(
            !data()
                .remaining(0, true, RewardPlan::NoWeaponExp)
                .materials
                .contains_key(&ore)
        );
        // Level 50: nothing left.
        assert!(
            data()
                .remaining(50, true, RewardPlan::NoArtifactExp)
                .is_empty()
        );
        // The levels after 45: the last Philosophies box, at 50.
        let end = data().remaining(45, true, RewardPlan::NoArtifactExp);
        assert_eq!(
            (end.from_level, end.boxes_of(BoxKind::Philosophies)),
            (46, 1)
        );
    }

    #[test]
    fn a_new_period_starts_the_pass_again() {
        let data = data();
        // The period of the data, and the next, 42 days later.
        let start = at("2026-11-04T11:00:00+08:00");
        assert_eq!(data.period_at(at("2026-11-20T12:00:00+01:00")), start);
        assert_eq!(data.next_period(start), at("2026-12-16T11:00:00+08:00"));
        assert_eq!(
            data.period_at(at("2026-12-16T03:00:00Z")),
            at("2026-12-16T11:00:00+08:00")
        );
        // Before the data's: periods count back as well.
        assert_eq!(
            data.period_at(at("2026-11-04T02:59:59Z")),
            at("2026-09-23T11:00:00+08:00")
        );

        let pass = BattlePass::new(data, at("2026-11-20T12:00:00+01:00")).at_level(
            23,
            data,
            at("2026-11-20T12:00:00+01:00"),
        );
        assert_eq!(pass.level_at(data, at("2026-12-10T00:00:00Z")), 23);
        assert_eq!(pass.level_at(data, at("2026-12-17T00:00:00Z")), 0);
        assert_eq!(
            pass.remaining(data, at("2026-12-17T00:00:00Z")).from_level,
            1,
            "a new period's levels are all to come"
        );
        assert_eq!(pass.at_level(70, data, start).level, 50);
    }

    #[test]
    fn the_boxes_go_to_what_the_plan_lacks_in_its_order() {
        let planner = PlannerData::embedded();
        let entry = |id, key: &str| PlanEntry {
            id,
            target: PlanTarget::Character {
                key: key.to_owned(),
                goal: CharacterGoal::FULL,
            },
        };
        // Ganyu (Diligence, Qingxin) then Furina (Justice, Lakelight Lily).
        let plan = [entry(1, "ganyu"), entry(2, "furina")];
        let rewards = data().remaining(0, true, RewardPlan::NoArtifactExp);
        let (with, share) =
            planner.allocate_with(&plan, &Inventory::default(), &rewards, BoxPicks::default());
        let without = planner.allocate(&plan, &Inventory::default());

        let name = |id: MaterialId| planner.material(id).unwrap().name.english().to_owned();
        let picked: BTreeMap<String, u64> = (share.picked_materials().into_iter())
            .map(|(id, n)| (name(id), n))
            .collect();
        // Ganyu comes first and lacks 114 Philosophies and 63 Guides: both
        // Philosophies boxes and the 7 Guides boxes go to her series; Furina's
        // gets none.
        assert_eq!(picked["Philosophies of Diligence"], 10);
        assert_eq!(picked["Guide to Diligence"], 35);
        assert_eq!(
            picked.keys().filter(|k| k.starts_with("Guide to")).count(),
            1,
            "{picked:?}"
        );
        assert_eq!(
            picked.values().sum::<u64>(),
            2 * 5 + 7 * 5 + 2 * 15,
            "every box picked: {picked:?}"
        );
        assert!(share.left.is_empty());
        // Both specialty stacks, to Ganyu's Qingxin (she lacks 168).
        assert_eq!(picked["Qingxin"], 30);

        // What the plan lacks went down by what the pass gives.
        let lacks = |a: &Allocation, id| a.missing.materials.get(&id).copied().unwrap_or(0);
        let qingxin = planner.material_named("Qingxin").unwrap();
        assert_eq!(lacks(&with, qingxin), lacks(&without, qingxin) - 30);
        assert_eq!(lacks(&with, MORA), lacks(&without, MORA) - 2_160_000);
        assert_eq!(with.needed, without.needed);
    }

    #[test]
    fn boxes_the_plan_needs_nothing_from_are_left_to_pick() {
        let planner = PlannerData::embedded();
        let rewards = data().remaining(0, true, RewardPlan::NoArtifactExp);
        let none = BoxPicks::default();
        let (allocation, share) = planner.allocate_with(&[], &Inventory::default(), &rewards, none);
        assert!(allocation.missing.materials.is_empty());
        assert!(share.picked.is_empty());
        assert_eq!(share.left[&BoxKind::Guides], 7);
        assert_eq!(share.left[&BoxKind::Philosophies], 2);
        assert_eq!(share.left[&BoxKind::Specialties], 2);
        // An inventory already holding all of it: nothing picked either.
        let mut bag = Inventory::default();
        bag.set(&CustomItem::Material(MORA), 1);
        let (_, share) = planner.allocate_with(&[], &bag, &rewards, none);
        assert!(share.picked.is_empty());
    }

    #[test]
    fn a_series_picked_takes_every_box_of_its_kind() {
        let planner = PlannerData::embedded();
        let id = |name: &str| planner.material_named(name).unwrap();
        let plan = [PlanEntry {
            id: 1,
            target: PlanTarget::Character {
                key: "ganyu".to_owned(),
                goal: CharacterGoal::FULL,
            },
        }];
        let rewards = data().remaining(0, true, RewardPlan::NoArtifactExp);
        // Philosophies of Freedom picked, even though Ganyu needs none; the
        // Guides and specialties still follow her lacks.
        let picks =
            BoxPicks::default().with(BoxKind::Philosophies, Some(id("Philosophies of Freedom")));
        let (with, share) = planner.allocate_with(&plan, &Inventory::default(), &rewards, picks);
        let of = |kind| share.picked.get(&kind).cloned().unwrap_or_default();
        assert_eq!(
            of(BoxKind::Philosophies),
            BTreeMap::from([(id("Philosophies of Freedom"), 10)])
        );
        assert_eq!(
            of(BoxKind::Guides),
            BTreeMap::from([(id("Guide to Diligence"), 35)])
        );
        assert_eq!(
            of(BoxKind::Specialties),
            BTreeMap::from([(id("Qingxin"), 30)])
        );
        // Ganyu's Philosophies get nothing from the pass now.
        let (auto, _) =
            planner.allocate_with(&plan, &Inventory::default(), &rewards, BoxPicks::default());
        let lacks = |a: &Allocation, id| a.missing.materials.get(&id).copied().unwrap_or(0);
        let diligence = id("Philosophies of Diligence");
        assert_eq!(lacks(&with, diligence), lacks(&auto, diligence) + 10);

        // Picked specialties and Guides, the plan empty: every box given.
        let picks = picks
            .with(BoxKind::Guides, Some(id("Guide to Ballad")))
            .with(BoxKind::Specialties, Some(id("Cecilia")));
        let (_, share) = planner.allocate_with(&[], &Inventory::default(), &rewards, picks);
        assert!(share.left.is_empty(), "{:?}", share.left);
        assert_eq!(share.picked_materials()[&id("Guide to Ballad")], 35);
        assert_eq!(share.picked_materials()[&id("Cecilia")], 30);

        // A pick no box can give (Cecilia as Guides) counts as none.
        let wrong = BoxPicks::default().with(BoxKind::Guides, Some(id("Cecilia")));
        let (_, share) = planner.allocate_with(&[], &Inventory::default(), &rewards, wrong);
        assert_eq!(share.left[&BoxKind::Guides], 7);
    }

    #[test]
    fn boxes_offer_every_series() {
        let planner = PlannerData::embedded();
        let name = |id: MaterialId| planner.material(id).unwrap().name.english().to_owned();
        let guides = planner.box_choices(BoxKind::Guides);
        let philosophies = planner.box_choices(BoxKind::Philosophies);
        assert_eq!(guides.len(), philosophies.len());
        assert!(guides.iter().all(|&id| name(id).starts_with("Guide to")));
        assert!(
            philosophies
                .iter()
                .all(|&id| name(id).starts_with("Philosophies of"))
        );
        assert!(guides.len() >= 18, "{}", guides.len());
        assert!(planner.box_choices(BoxKind::Specialties).len() >= 60);
    }

    #[test]
    fn the_embedded_data_names_materials_the_planner_knows() {
        let planner = PlannerData::embedded();
        for plan in RewardPlan::ALL {
            let all = data().remaining(0, true, *plan);
            for id in all.materials.keys() {
                assert!(planner.material(*id).is_some(), "{plan}: material {id}");
            }
        }
        assert_eq!(data().period_days, 42);
        assert!(data().boxes.guides > 0 && data().boxes.specialties > 0);
    }
}
