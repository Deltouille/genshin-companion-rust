//! The player's plan: what they want to raise, as they typed it in, and what
//! it all costs. Costs are never stored: they are worked out from the game
//! data each time, like pity.

use std::collections::BTreeMap;

use super::{CharacterGoal, CustomMaterials, MaterialId, PlannerData, Requirement, WeaponGoal};

/// One line of the plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanEntry {
    pub id: i64,
    pub target: PlanTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanTarget {
    /// A character of the game data, from where it is to where it should be.
    Character { key: String, goal: CharacterGoal },
    /// A weapon of the game data, from its level to the one it should reach.
    Weapon { key: String, goal: WeaponGoal },
    /// A character the game data does not have yet (a leak), made of the
    /// materials it uses: it costs what any character costs (see
    /// [`PlannerData::custom_character_cost`]).
    CustomCharacter {
        name: String,
        /// 4 or 5.
        stars: u8,
        materials: CustomMaterials,
        goal: CharacterGoal,
    },
    /// A weapon the game data does not have yet, made of the materials it
    /// uses: it costs what a weapon of its stars costs (see
    /// [`PlannerData::custom_weapon_cost`]).
    CustomWeapon {
        name: String,
        stars: u8,
        materials: CustomMaterials,
        goal: WeaponGoal,
    },
    /// An earlier kind of custom entry: a name and what it needs, as the
    /// player typed them. Kept so that plans made with it still read.
    Custom {
        name: String,
        costs: Vec<CustomCost>,
    },
}

impl PlanTarget {
    /// Where a character entry's level and talents go from and to.
    pub fn goal(&self) -> Option<&CharacterGoal> {
        match self {
            PlanTarget::Character { goal, .. } | PlanTarget::CustomCharacter { goal, .. } => {
                Some(goal)
            }
            _ => None,
        }
    }

    pub fn goal_mut(&mut self) -> Option<&mut CharacterGoal> {
        match self {
            PlanTarget::Character { goal, .. } | PlanTarget::CustomCharacter { goal, .. } => {
                Some(goal)
            }
            _ => None,
        }
    }

    /// Where a weapon entry's level goes from and to.
    pub fn weapon_goal(&self) -> Option<&WeaponGoal> {
        match self {
            PlanTarget::Weapon { goal, .. } | PlanTarget::CustomWeapon { goal, .. } => Some(goal),
            _ => None,
        }
    }

    pub fn weapon_goal_mut(&mut self) -> Option<&mut WeaponGoal> {
        match self {
            PlanTarget::Weapon { goal, .. } | PlanTarget::CustomWeapon { goal, .. } => Some(goal),
            _ => None,
        }
    }
}

/// So many of something a custom entry needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomCost {
    pub item: CustomItem,
    pub count: u32,
}

/// A material the game data knows, or one it does not, by name: what a
/// custom entry needs and what the inventory counts.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CustomItem {
    /// A material the game data knows.
    Material(MaterialId),
    /// One it does not (a leaked material), by the name the player gave.
    Named(String),
}

/// What a plan, or one entry of it, needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanTotals {
    /// Known materials by id, Mora included, EXP turned into books and ores.
    pub materials: BTreeMap<MaterialId, u64>,
    /// Materials the game data does not have, by name.
    pub named: BTreeMap<String, u64>,
    /// Entries whose character is no longer in the game data.
    pub unknown: Vec<i64>,
}

impl PlannerData {
    /// What one entry needs.
    pub fn entry_totals(&self, entry: &PlanEntry) -> PlanTotals {
        self.plan_totals(std::slice::from_ref(entry))
    }

    /// What the whole plan needs. The EXP of all characters is added up
    /// before it becomes books, so only the very last book is rounded up.
    pub fn plan_totals(&self, entries: &[PlanEntry]) -> PlanTotals {
        let mut totals = PlanTotals::default();
        let mut need = Requirement::default();
        for entry in entries {
            match self.entry_requirement(entry) {
                Some((cost, named)) => {
                    need.merge(&cost);
                    for (name, count) in named {
                        *totals.named.entry(name).or_default() += count;
                    }
                }
                None => totals.unknown.push(entry.id),
            }
        }
        totals.materials = need.materials_with_exp();
        totals
    }

    /// What one entry costs, EXP still as EXP, and the materials the game
    /// data does not have by name; `None` for a character no longer in the
    /// data.
    pub(super) fn entry_requirement(
        &self,
        entry: &PlanEntry,
    ) -> Option<(Requirement, BTreeMap<String, u64>)> {
        Some(match &entry.target {
            PlanTarget::Character { key, goal } => (
                self.character_cost(self.character(key)?, goal),
                BTreeMap::new(),
            ),
            PlanTarget::Weapon { key, goal } => {
                (self.weapon_cost(self.weapon(key)?, goal), BTreeMap::new())
            }
            PlanTarget::CustomCharacter {
                materials, goal, ..
            } => self.custom_character_cost(materials, goal),
            PlanTarget::CustomWeapon {
                materials,
                stars,
                goal,
                ..
            } => self.custom_weapon_cost(materials, *stars, goal),
            PlanTarget::Custom { costs, .. } => {
                let (mut need, mut named) = (Requirement::default(), BTreeMap::new());
                for cost in costs {
                    let count = u64::from(cost.count);
                    match &cost.item {
                        CustomItem::Material(id) => {
                            *need.materials.entry(*id).or_default() += count;
                        }
                        CustomItem::Named(name) => {
                            *named.entry(name.clone()).or_default() += count;
                        }
                    }
                }
                (need, named)
            }
        })
    }

    /// A material by its English name, ignoring case: what a custom entry's
    /// typed resource is, when the game data knows it.
    pub fn material_named(&self, name: &str) -> Option<MaterialId> {
        let name = name.trim();
        self.materials()
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(name))
            .map(|m| m.id)
    }
}
