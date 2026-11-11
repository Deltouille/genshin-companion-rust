//! The ascension planner: what it costs to take a character or a weapon from
//! where it is to where the player wants it, in materials, Mora and EXP.
//!
//! The game data (costs per ascension phase and talent level, EXP tables)
//! comes from [`PlannerData`]; the rules of the game the data does not carry
//! (level caps per phase, what EXP books and ores give, what levelling costs in
//! Mora) are constants here.

mod allocate;
mod custom;
mod data;
mod inventory;
mod plan;

use std::collections::BTreeMap;

pub use allocate::{Allocation, EntryShare};
pub use custom::{CustomMaterials, Role};
pub use data::{
    Character, Cost, MAX_LEVEL, Material, MaterialId, PLANNER_FILE, PlannerData, Weapon,
};
pub use inventory::Inventory;
pub use plan::{CustomCost, CustomItem, PlanEntry, PlanTarget, PlanTotals};

/// Mora's material id.
pub const MORA: MaterialId = 202;

/// The highest level of each ascension phase: phase 0 ends at 20, phase 6 at 90.
const PHASE_CAPS: [u8; 7] = [20, 40, 50, 60, 70, 80, 90];

/// Character EXP materials, largest first: Hero's Wit, Adventurer's
/// Experience, Wanderer's Advice, and the EXP each gives.
pub(crate) const EXP_BOOKS: [(MaterialId, u64); 3] =
    [(104003, 20_000), (104002, 5_000), (104001, 1_000)];
/// Weapon EXP materials, largest first: Mystic, Fine and plain Enhancement Ore.
pub(crate) const EXP_ORES: [(MaterialId, u64); 3] =
    [(104013, 10_000), (104012, 2_000), (104011, 400)];
/// Levelling costs 1 Mora per 5 EXP for a character, per 10 for a weapon
/// (a Hero's Wit costs 4,000 Mora to use, a Mystic Enhancement Ore 1,000).
const CHARACTER_EXP_PER_MORA: u64 = 5;
const WEAPON_EXP_PER_MORA: u64 = 10;

/// How many times something of `rarity` stars ascends: 1★ and 2★ weapons stop
/// at level 70 after 4 ascensions; everything else ascends 6 times.
pub(crate) fn phases(rarity: u8) -> u8 {
    if rarity <= 2 { 4 } else { 6 }
}

/// A level and its ascension phase. Level 80 is phase 5 before ascending and
/// phase 6 after (shown "80/90" in game: the level, then the cap).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Level {
    pub level: u8,
    pub phase: u8,
}

impl Level {
    /// Level 1, never ascended.
    pub const START: Level = Level { level: 1, phase: 0 };
    /// Level 90, fully ascended.
    pub const MAX: Level = Level {
        level: MAX_LEVEL,
        phase: 6,
    };

    /// `level` at `phase`, if the game allows it: a phase holds the levels
    /// from the previous phase's cap (or 1) to its own.
    pub fn new(level: u8, phase: u8) -> Option<Level> {
        let cap = *PHASE_CAPS.get(usize::from(phase))?;
        let floor = match phase {
            0 => 1,
            _ => PHASE_CAPS[usize::from(phase) - 1],
        };
        (floor..=cap)
            .contains(&level)
            .then_some(Level { level, phase })
    }

    /// Every level and phase in order, as a slider steps through them: 1/20 …
    /// 20/20, 20/40 … 90/90 (96 steps).
    /// The levels of a weapon of `rarity` stars, as [`Level::steps`]: up to
    /// 70/70 for 1★ and 2★ weapons, which ascend 4 times.
    pub fn weapon_steps(rarity: u8) -> Vec<Level> {
        let top = phases(rarity);
        Level::steps()
            .into_iter()
            .filter(|level| level.phase <= top && level.level <= Level::cap_of(top))
            .collect()
    }

    pub fn steps() -> Vec<Level> {
        (0..=6u8)
            .flat_map(|phase| {
                let floor = if phase == 0 {
                    1
                } else {
                    PHASE_CAPS[usize::from(phase) - 1]
                };
                (floor..=PHASE_CAPS[usize::from(phase)]).map(move |level| Level { level, phase })
            })
            .collect()
    }

    /// `level` moved into `phase` (clamped to 0–6): the nearest level the phase
    /// holds.
    pub fn clamped(level: u8, phase: u8) -> Level {
        let phase = phase.min(6);
        let floor = if phase == 0 {
            1
        } else {
            PHASE_CAPS[usize::from(phase) - 1]
        };
        Level {
            level: level.clamp(floor, PHASE_CAPS[usize::from(phase)]),
            phase,
        }
    }

    /// The highest level of this phase (shown after the slash in game).
    pub fn cap(self) -> u8 {
        PHASE_CAPS[usize::from(self.phase)]
    }

    /// The highest level of `phase` (0 to 6).
    pub fn cap_of(phase: u8) -> u8 {
        PHASE_CAPS[usize::from(phase.min(6))]
    }
}

/// Where a character should go: its level and the level of each of its three
/// talents (normal attack, skill, burst), from 1 to 10.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterGoal {
    pub from: Level,
    pub to: Level,
    pub talents: [(u8, u8); 3],
}

impl CharacterGoal {
    /// From level 1 to 90/90, talents 1 to 10: what a character added to the plan
    /// starts with.
    pub const FULL: CharacterGoal = CharacterGoal {
        from: Level::START,
        to: Level::MAX,
        talents: [(1, 10); 3],
    };
}

/// Where a weapon should go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponGoal {
    pub from: Level,
    pub to: Level,
}

impl WeaponGoal {
    /// From level 1 to the top of a weapon of `rarity` stars: what a weapon
    /// added to the plan starts with.
    pub fn full(rarity: u8) -> WeaponGoal {
        WeaponGoal {
            from: Level::START,
            to: *Level::weapon_steps(rarity)
                .last()
                .expect("a weapon has levels"),
        }
    }
}

/// What a plan needs: materials (Mora included) by id, and the EXP to give,
/// which [`Requirement::materials_with_exp`] turns into books and ores.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Requirement {
    pub materials: BTreeMap<MaterialId, u64>,
    pub character_exp: u64,
    pub weapon_exp: u64,
}

impl Requirement {
    fn add(&mut self, costs: &[Cost]) {
        for cost in costs {
            *self.materials.entry(cost.material).or_default() += u64::from(cost.count);
        }
    }

    /// Adds another requirement to this one (several characters and weapons).
    pub fn merge(&mut self, other: &Requirement) {
        for (&material, &count) in &other.materials {
            *self.materials.entry(material).or_default() += count;
        }
        self.character_exp += other.character_exp;
        self.weapon_exp += other.weapon_exp;
    }

    /// The materials with the EXP turned into books and ores, largest first
    /// (the smallest rounded up, so the EXP is always reached), and the Mora
    /// levelling costs added: 1 per 5 character EXP, 1 per 10 weapon EXP.
    pub fn materials_with_exp(&self) -> BTreeMap<MaterialId, u64> {
        let mut materials = self.materials.clone();
        add_exp_items(&mut materials, self.character_exp, self.weapon_exp);
        let mora = self.exp_mora();
        if mora > 0 {
            *materials.entry(MORA).or_default() += mora;
        }
        materials
    }

    /// What levelling with this EXP costs in Mora.
    pub(crate) fn exp_mora(&self) -> u64 {
        self.character_exp / CHARACTER_EXP_PER_MORA + self.weapon_exp / WEAPON_EXP_PER_MORA
    }
}

/// Adds the books giving `character_exp` and the ores giving `weapon_exp` to
/// `materials`: largest first, the smallest rounded up.
pub(crate) fn add_exp_items(
    materials: &mut BTreeMap<MaterialId, u64>,
    character_exp: u64,
    weapon_exp: u64,
) {
    for (exp, items) in [(character_exp, EXP_BOOKS), (weapon_exp, EXP_ORES)] {
        let mut left = exp;
        for (i, (material, gives)) in items.into_iter().enumerate() {
            let count = if i + 1 == items.len() {
                left.div_ceil(gives)
            } else {
                left / gives
            };
            if count > 0 {
                *materials.entry(material).or_default() += count;
            }
            left = left.saturating_sub(count * gives);
        }
    }
}

impl PlannerData {
    /// What taking `character` from `goal.from` to `goal.to` costs. Nothing
    /// is counted for a step backwards.
    pub fn character_cost(&self, character: &Character, goal: &CharacterGoal) -> Requirement {
        let mut need = Requirement::default();
        for level in goal.from.level..goal.to.level {
            need.character_exp += u64::from(self.character_exp(level));
        }
        for phase in goal.from.phase..goal.to.phase {
            need.add(&character.ascension[usize::from(phase)]);
        }
        for (from, to) in goal.talents {
            for level in from.max(1)..to.min(10) {
                // `talents[0]` raises a talent to level 2.
                need.add(&character.talents[usize::from(level) - 1]);
            }
        }
        need
    }

    /// What taking `weapon` from `goal.from` to `goal.to` costs.
    pub fn weapon_cost(&self, weapon: &Weapon, goal: &WeaponGoal) -> Requirement {
        let mut need = Requirement::default();
        let top = goal.to.phase.min(phases(weapon.rarity));
        for level in goal.from.level..goal.to.level.min(PHASE_CAPS[usize::from(top)]) {
            need.weapon_exp += u64::from(self.weapon_exp(weapon.rarity, level));
        }
        for phase in goal.from.phase..top {
            need.add(&weapon.ascension[usize::from(phase)]);
        }
        need
    }
}
