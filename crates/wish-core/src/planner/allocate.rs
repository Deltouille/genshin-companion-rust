//! Sharing the inventory out over the plan, in its order, as genshin-center
//! does: the first entry takes what it can of what the player has, the next
//! what is left, and so on. What each entry still lacks is what the player
//! has to farm for it.
//!
//! Two rules of the game stretch the inventory:
//!
//! - a material short of what an entry needs is crafted from the tiers below
//!   it, three for one as at the alchemy table (the Mora crafting costs is
//!   not counted);
//! - EXP books, and weapon ores, are worth their EXP: four Adventurer's
//!   Experience do a Hero's Wit's work. What is still needed shows as Hero's
//!   Wit and Mystic Enhancement Ore alone.

use std::collections::BTreeMap;

use super::{
    EXP_BOOKS, EXP_ORES, Inventory, MORA, MaterialId, PlanEntry, PlanTotals, PlannerData,
    Requirement, add_exp_items,
};

/// The inventory shared out over a plan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Allocation {
    /// Each entry's share, in plan order.
    pub entries: Vec<EntryShare>,
    /// What the whole plan needs, as [`PlannerData::plan_totals`].
    pub needed: PlanTotals,
    /// What the whole plan still lacks once the inventory is used.
    pub missing: PlanTotals,
    /// How many of each material the plan crafts from the tiers below it,
    /// out of what they have to spare.
    pub crafted: BTreeMap<MaterialId, u64>,
}

impl Allocation {
    /// How many of `id` the plan crafts from the tiers below.
    pub fn crafted(&self, id: MaterialId) -> u64 {
        self.crafted.get(&id).copied().unwrap_or(0)
    }
}

/// What one entry needs, and what it still lacks after the entries before it
/// took their part of the inventory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EntryShare {
    pub id: i64,
    pub needed: PlanTotals,
    pub missing: PlanTotals,
    /// How many of each material are crafted for it from the tiers below.
    pub crafted: BTreeMap<MaterialId, u64>,
}

impl EntryShare {
    /// How many of a known material the entry still lacks.
    pub fn missing_material(&self, id: MaterialId) -> u64 {
        self.missing.materials.get(&id).copied().unwrap_or(0)
    }

    /// How many of `id` are crafted for the entry from the tiers below.
    pub fn crafted(&self, id: MaterialId) -> u64 {
        self.crafted.get(&id).copied().unwrap_or(0)
    }
}

impl PlannerData {
    /// Shares `inventory` out over `plan`, in its order.
    pub fn allocate(&self, plan: &[PlanEntry], inventory: &Inventory) -> Allocation {
        let mut pool = Pool::new(inventory);
        let mut allocation = Allocation {
            needed: self.plan_totals(plan),
            ..Allocation::default()
        };
        let (mut character_exp, mut weapon_exp) = (0, 0);
        for entry in plan {
            let mut share = EntryShare {
                id: entry.id,
                ..EntryShare::default()
            };
            let Some((requirement, named)) = self.entry_requirement(entry) else {
                share.needed.unknown.push(entry.id);
                share.missing.unknown.push(entry.id);
                allocation.missing.unknown.push(entry.id);
                allocation.entries.push(share);
                continue;
            };
            share.needed.materials = requirement.materials_with_exp();
            share.needed.named = named.clone();

            let short = pool.take(self, &requirement, &named);
            share.missing.materials = short.materials.clone();
            add_exp_items(
                &mut share.missing.materials,
                short.character_exp,
                short.weapon_exp,
            );
            share.missing.named = short.named.clone();

            for (id, count) in short.materials {
                *allocation.missing.materials.entry(id).or_default() += count;
            }
            for (name, count) in short.named {
                *allocation.missing.named.entry(name).or_default() += count;
            }
            for (&id, &count) in &short.crafted {
                *allocation.crafted.entry(id).or_default() += count;
            }
            share.crafted = short.crafted;
            character_exp += short.character_exp;
            weapon_exp += short.weapon_exp;
            allocation.entries.push(share);
        }
        // The EXP still lacking, turned into books once for the whole plan.
        add_exp_items(&mut allocation.missing.materials, character_exp, weapon_exp);
        allocation
    }
}

/// What is left of the inventory as the entries take their part.
struct Pool {
    materials: BTreeMap<MaterialId, u64>,
    named: BTreeMap<String, u64>,
    /// The EXP of the books the player has.
    character_exp: u64,
    /// The EXP of the ores.
    weapon_exp: u64,
}

/// What an entry lacks (EXP still as EXP), and what was crafted for it.
struct Short {
    materials: BTreeMap<MaterialId, u64>,
    named: BTreeMap<String, u64>,
    crafted: BTreeMap<MaterialId, u64>,
    character_exp: u64,
    weapon_exp: u64,
}

impl Pool {
    fn new(inventory: &Inventory) -> Self {
        let mut materials = inventory.materials.clone();
        let mut exp_of = |items: [(MaterialId, u64); 3]| -> u64 {
            (items.iter())
                .map(|(id, gives)| materials.remove(id).unwrap_or(0) * gives)
                .sum()
        };
        let (character_exp, weapon_exp) = (exp_of(EXP_BOOKS), exp_of(EXP_ORES));
        Self {
            materials,
            named: inventory.named.clone(),
            character_exp,
            weapon_exp,
        }
    }

    /// Takes what `requirement` and `named` need from the pool; returns what
    /// it lacks.
    fn take(
        &mut self,
        data: &PlannerData,
        requirement: &Requirement,
        named: &BTreeMap<String, u64>,
    ) -> Short {
        let mut want = requirement.materials.clone();
        let mora = requirement.exp_mora();
        if mora > 0 {
            *want.entry(MORA).or_default() += mora;
        }
        let mut short = BTreeMap::new();
        let mut crafted_for = BTreeMap::new();

        // Crafted series, lowest tier first: what the entry needs of a lower
        // tier is kept for it before the rest is crafted up.
        for family in data.craft_families() {
            if !family.iter().any(|id| want.contains_key(id)) {
                continue;
            }
            let mut counts: Vec<u64> = (family.iter())
                .map(|id| self.materials.get(id).copied().unwrap_or(0))
                .collect();
            for (tier, id) in family.iter().enumerate() {
                let Some(need) = want.remove(id) else {
                    continue;
                };
                let direct = need.min(counts[tier]);
                counts[tier] -= direct;
                let mut left = need - direct;
                if left > 0 {
                    let crafted = craftable(&counts[..tier]).min(left);
                    use_for_crafting(&mut counts, tier, crafted);
                    left -= crafted;
                    if crafted > 0 {
                        crafted_for.insert(*id, crafted);
                    }
                }
                if left > 0 {
                    short.insert(*id, left);
                }
            }
            for (id, count) in family.iter().zip(counts) {
                self.materials.insert(*id, count);
            }
        }

        // The rest, as it is.
        for (id, need) in want {
            let have = self.materials.entry(id).or_default();
            let taken = need.min(*have);
            *have -= taken;
            if need > taken {
                short.insert(id, need - taken);
            }
        }
        let mut named_short = BTreeMap::new();
        for (name, &need) in named {
            let have = self.named.entry(name.clone()).or_default();
            let taken = need.min(*have);
            *have -= taken;
            if need > taken {
                named_short.insert(name.clone(), need - taken);
            }
        }
        let exp = |need: u64, have: &mut u64| {
            let taken = need.min(*have);
            *have -= taken;
            need - taken
        };
        Short {
            materials: short,
            named: named_short,
            crafted: crafted_for,
            character_exp: exp(requirement.character_exp, &mut self.character_exp),
            weapon_exp: exp(requirement.weapon_exp, &mut self.weapon_exp),
        }
    }
}

/// How many of the next tier `lower` (counts of the tiers below it, lowest
/// first) can be crafted into, crafting each tier up in turn.
fn craftable(lower: &[u64]) -> u64 {
    lower
        .iter()
        .fold(0, |carried, &count| (count + carried) / 3)
}

/// Uses up the lower tiers to craft `count` of `tier`: the tier just below
/// first, then crafting that one from below. `count` must be craftable.
fn use_for_crafting(counts: &mut [u64], tier: usize, count: u64) {
    if count == 0 || tier == 0 {
        return;
    }
    let need = 3 * count;
    let direct = need.min(counts[tier - 1]);
    counts[tier - 1] -= direct;
    use_for_crafting(counts, tier - 1, need - direct);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crafting_carries_up_three_for_one() {
        assert_eq!(craftable(&[]), 0);
        assert_eq!(craftable(&[8]), 2);
        // 27 of the first tier make 9 of the second, then 3 of the third.
        assert_eq!(craftable(&[27, 0]), 3);
        assert_eq!(craftable(&[4, 2]), 1);

        let mut counts = [27, 2, 0];
        use_for_crafting(&mut counts, 2, 1);
        // The two of the second tier first, then one more crafted from three.
        assert_eq!(counts, [24, 0, 0]);
    }
}
