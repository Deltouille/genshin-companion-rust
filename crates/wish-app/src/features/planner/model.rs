//! What the planner page shows, worked out from the plan (each goal as its
//! sliders show it), the inventory, the game data, the resin options and the
//! Battle Pass: every card's name, picture, ranges and materials, what each
//! lacks once the inventory (and the pass's rewards still to come, when they
//! count) is shared out, the resin, what the pass brings, and the words of the
//! tooltips.
//!
//! Plain data, no GPUI: the view builds it when one of those changes (the
//! library, the account, a slider moved), its render only reads it, and it is
//! tested like a function. Pictures are named (a character's or weapon's
//! key, a material's id); the sections look them up as they draw.

use std::collections::{BTreeMap, BTreeSet};

use wish_core::planner::{
    BattlePassRewards, BoxKind, BoxPicks, CustomItem, Inventory, Level, MaterialId, PlanEntry,
    PlanTarget, PlanTotals, PlannerData, ResinEstimate, ResinOptions,
};
use wish_ui::format;

/// The talent rows' names, in the order of the goal's talents.
pub(crate) fn talent_names() -> [String; 3] {
    [
        t!("planner-normal-attack"),
        t!("planner-elemental-skill"),
        t!("planner-elemental-burst"),
    ]
}

/// Everything the planner page shows, as plain data: built by
/// [`PlannerModel::build`] from the plan, the inventory and the settings, never
/// in `render`.
#[derive(Default)]
pub(super) struct PlannerModel {
    /// The plan's entries, in its order: the first take from the inventory
    /// first.
    pub entries: Vec<EntryModel>,
    /// What the whole plan lacks once the inventory is shared out.
    pub total: MaterialsModel,
    /// The resin farming that takes.
    pub resin: ResinEstimate,
    /// What the Battle Pass's levels still to come bring the plan.
    pub pass: PassModel,
}

/// The Battle Pass's levels still to come, as the drawer shows them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct PassModel {
    /// The first level still to come; past 50 when none is.
    pub from_level: u8,
    /// Mora, EXP books and ores, in the game's order.
    pub rewards: Vec<(MaterialId, u64)>,
    /// The boxes still to come, by kind.
    pub boxes: Vec<(BoxKind, u64)>,
    /// What each kind of box becomes, in the game's order.
    pub picked: Vec<(BoxKind, MaterialId, u64)>,
    /// The boxes the plan needs nothing from, to pick freely.
    pub left: Vec<(BoxKind, u64)>,
}

/// The Battle Pass as the page counts it: its rewards still to come, the
/// series picked for its boxes, and whether "Still needed" counts them.
pub(super) struct PassInput<'a> {
    pub rewards: &'a BattlePassRewards,
    pub picks: BoxPicks,
    pub counted: bool,
}

/// One card of the plan: its entry, its name and picture, and what it still
/// lacks once the entries before it took their share.
pub(super) struct EntryModel {
    pub entry: PlanEntry,
    pub name: String,
    /// The line under the name: "Cryo · Bow", "Custom weapon · 4★".
    pub detail: Option<String>,
    pub portrait: Portrait,
    /// Why the entry costs nothing: what it names left the game data.
    pub warning: Option<String>,
    /// What its sliders are set to; none for a custom entry of earlier
    /// versions.
    pub ranges: Option<Ranges>,
    /// What it needs and still lacks after the entries before it.
    pub materials: MaterialsModel,
    pub resin: ResinEstimate,
}

impl EntryModel {
    pub fn id(&self) -> i64 {
        self.entry.id
    }

    /// A custom character or weapon, edited in its dialog.
    pub fn editable(&self) -> bool {
        matches!(
            self.entry.target,
            PlanTarget::CustomCharacter { .. } | PlanTarget::CustomWeapon { .. }
        )
    }
}

/// What a card shows in the corner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Portrait {
    /// A character's icon on its rarity tile, by its key.
    Character(String, u8),
    /// A weapon's icon on its rarity tile, by its key.
    Weapon(String, u8),
    Custom(u8),
    CustomWeapon(u8),
    /// A custom entry of earlier versions.
    Entry,
}

/// What an entry's sliders are set to: its level range over its steps (a
/// character's, or a weapon's of its rarity), and a character's talents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Ranges {
    pub steps: Vec<Level>,
    pub from: Level,
    pub to: Level,
    /// Empty for a weapon.
    pub talents: Vec<(u8, u8)>,
}

impl Ranges {
    /// "70/80 → 90/90".
    pub fn level_text(&self) -> String {
        format!(
            "{}/{} → {}/{}",
            self.from.level,
            self.from.cap(),
            self.to.level,
            self.to.cap()
        )
    }

    /// Where a level falls on the slider, 0 to 1.
    pub fn position(&self, level: Level) -> f32 {
        let last = (self.steps.len().max(2) - 1) as f32;
        self.steps.iter().position(|s| *s == level).unwrap_or(0) as f32 / last
    }
}

/// A list of materials: those the game data knows, in its inventory order,
/// then those it doesn't, by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct MaterialsModel {
    pub known: Vec<KnownMaterial>,
    pub named: Vec<NamedMaterial>,
}

impl MaterialsModel {
    pub fn is_empty(&self) -> bool {
        self.known.is_empty() && self.named.is_empty()
    }
}

/// A material of the game data on a tile: what is needed, what is still
/// lacking, how many are crafted from below, and its tooltip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct KnownMaterial {
    pub id: MaterialId,
    pub need: u64,
    pub lack: u64,
    /// Crafted from the tiers below, shown on the tile.
    pub crafted: u64,
    pub tip: String,
}

/// A material only known by the name the player typed (a custom entry's), on a
/// tile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NamedMaterial {
    pub name: String,
    pub need: u64,
    pub lack: u64,
    pub tip: String,
}

impl NamedMaterial {
    /// "12 × Moonfall Lily": what is still lacking.
    pub fn label(&self) -> String {
        format!("{} × {}", format::count(self.lack), self.name)
    }
}

/// What the counts under a list of materials say.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Count {
    /// An entry's: what it still lacks once the entries before it took their
    /// part of the inventory.
    Entry,
    /// The plan's: what it still lacks.
    Plan,
}

impl PlannerModel {
    /// The page for `plan`, its goals as the sliders show them, with the
    /// Battle Pass's rewards still to come counted when `pass` says so.
    pub fn build(
        plan: &[PlanEntry],
        inventory: &Inventory,
        planner: &PlannerData,
        resin: &ResinOptions,
        pass: Option<PassInput>,
    ) -> Self {
        // The pass's share is worked out either way: the drawer shows where
        // its boxes would go.
        let shared = pass
            .as_ref()
            .map(|pass| planner.allocate_with(plan, inventory, pass.rewards, pass.picks));
        let counted = pass.as_ref().is_some_and(|pass| pass.counted);
        let (allocation, share) = match shared {
            Some((with, share)) if counted => (with, Some(share)),
            Some((_, share)) => (planner.allocate(plan, inventory), Some(share)),
            None => (planner.allocate(plan, inventory), None),
        };
        // What the pass adds to each material, when it counts.
        let mut from_pass: BTreeMap<MaterialId, u64> = BTreeMap::new();
        if let Some(share) = share.as_ref().filter(|_| counted) {
            let picked = share.picked_materials();
            for (&id, &count) in share.rewards.materials.iter().chain(&picked) {
                *from_pass.entry(id).or_default() += count;
            }
        }
        let entries: Vec<EntryModel> = (plan.iter().zip(&allocation.entries))
            .map(|(entry, share)| {
                let materials = materials(
                    &share.needed,
                    &share.missing,
                    &share.crafted,
                    Count::Entry,
                    inventory,
                    &from_pass,
                    planner,
                );
                let resin = planner.resin_estimate(&share.missing.materials, resin);
                describe(entry, materials, resin, planner)
            })
            .collect();
        let total = materials(
            &allocation.needed,
            &allocation.missing,
            &allocation.crafted,
            Count::Plan,
            inventory,
            &from_pass,
            planner,
        );
        let in_order = |counts: &BTreeMap<MaterialId, u64>| {
            let mut list: Vec<(MaterialId, u64)> = counts.iter().map(|(&id, &n)| (id, n)).collect();
            list.sort_by_key(|(id, _)| (planner.material(*id).map_or(u32::MAX, |m| m.sort), *id));
            list
        };
        let pass = share
            .map(|share| PassModel {
                from_level: share.rewards.from_level,
                rewards: in_order(&share.rewards.materials),
                boxes: share.rewards.boxes.iter().map(|(&k, &n)| (k, n)).collect(),
                picked: (share.picked.iter())
                    .flat_map(|(&kind, picked)| {
                        in_order(picked)
                            .into_iter()
                            .map(move |(id, n)| (kind, id, n))
                    })
                    .collect(),
                left: share.left.into_iter().collect(),
            })
            .unwrap_or_default();
        Self {
            entries,
            total,
            resin: planner.resin_estimate(&allocation.missing.materials, resin),
            pass,
        }
    }

    pub fn entry(&self, id: i64) -> Option<&EntryModel> {
        self.entries.iter().find(|entry| entry.id() == id)
    }
}

/// An entry's card: its name, picture and ranges, with what it costs.
fn describe(
    entry: &PlanEntry,
    materials: MaterialsModel,
    resin: ResinEstimate,
    planner: &PlannerData,
) -> EntryModel {
    let character_ranges = |goal: &wish_core::planner::CharacterGoal| Ranges {
        steps: Level::steps(),
        from: goal.from,
        to: goal.to,
        talents: goal.talents.to_vec(),
    };
    let weapon_ranges = |rarity: u8, goal: &wish_core::planner::WeaponGoal| Ranges {
        steps: Level::weapon_steps(rarity),
        from: goal.from,
        to: goal.to,
        talents: Vec::new(),
    };
    let (name, detail, portrait, warning, ranges) = match &entry.target {
        PlanTarget::Character { key, goal } => {
            let character = planner.character(key);
            (
                character.map_or(key.clone(), |c| c.name.to_string()),
                character.map(|c| {
                    format!(
                        "{} · {}",
                        format::element(&c.element),
                        format::weapon_type(&c.weapon)
                    )
                }),
                Portrait::Character(key.clone(), character.map_or(4, |c| c.rarity)),
                (character.is_none()).then(|| t!("planner-character-gone")),
                Some(character_ranges(goal)),
            )
        }
        PlanTarget::Weapon { key, goal } => {
            let weapon = planner.weapon(key);
            let rarity = weapon.map_or(5, |w| w.rarity);
            (
                weapon.map_or(key.clone(), |w| w.name.to_string()),
                weapon.map(|w| format!("{} · {}★", format::weapon_type(&w.weapon), w.rarity)),
                Portrait::Weapon(key.clone(), rarity),
                (weapon.is_none()).then(|| t!("planner-weapon-gone")),
                Some(weapon_ranges(rarity, goal)),
            )
        }
        PlanTarget::CustomCharacter {
            name, stars, goal, ..
        } => (
            name.clone(),
            Some(t!("planner-custom-character", stars = *stars)),
            Portrait::Custom(*stars),
            None,
            Some(character_ranges(goal)),
        ),
        PlanTarget::CustomWeapon {
            name, stars, goal, ..
        } => (
            name.clone(),
            Some(t!("planner-custom-weapon", stars = *stars)),
            Portrait::CustomWeapon(*stars),
            None,
            Some(weapon_ranges(*stars, goal)),
        ),
        // The free-form entries of earlier versions: shown, and removable,
        // but no longer made.
        PlanTarget::Custom { name, .. } => (
            name.clone(),
            Some(t!("planner-custom-entry")),
            Portrait::Entry,
            None,
            None,
        ),
    };
    EntryModel {
        entry: entry.clone(),
        name,
        detail,
        portrait,
        warning,
        ranges,
        materials,
        resin,
    }
}

/// A list's materials: the known ones in the game's inventory order (EXP
/// lacking may come out as other books than the EXP needed), then the others
/// by name, each with its tooltip.
fn materials(
    needed: &PlanTotals,
    missing: &PlanTotals,
    crafted: &BTreeMap<MaterialId, u64>,
    count: Count,
    inventory: &Inventory,
    from_pass: &BTreeMap<MaterialId, u64>,
    planner: &PlannerData,
) -> MaterialsModel {
    let pass_of = |item: &CustomItem| match item {
        CustomItem::Material(id) => from_pass.get(id).copied().unwrap_or(0),
        CustomItem::Named(_) => 0,
    };
    let tip = |name: &str, item: &CustomItem, need: u64, lack: u64| match count {
        Count::Entry if lack == 0 => t!("planner-tip-covered", name = name, need = need),
        Count::Entry if lack < need => {
            t!("planner-tip-partly", name = name, need = need, lack = lack)
        }
        Count::Entry => t!("planner-tip-needed", name = name, need = need),
        Count::Plan if pass_of(item) > 0 => t!(
            "planner-tip-plan-pass",
            name = name,
            need = need,
            owned = inventory.owned(item),
            pass = pass_of(item),
            lack = lack
        ),
        Count::Plan => t!(
            "planner-tip-plan",
            name = name,
            need = need,
            owned = inventory.owned(item),
            lack = lack
        ),
    };

    let ids: BTreeSet<MaterialId> = (needed.materials.keys())
        .chain(missing.materials.keys())
        .copied()
        .collect();
    let mut known: Vec<_> = (ids.into_iter())
        .filter_map(|id| Some((planner.material(id)?, id)))
        .collect();
    known.sort_by_key(|(material, id)| (material.sort, *id));
    let known = (known.into_iter())
        .map(|(material, id)| {
            let need = needed.materials.get(&id).copied().unwrap_or(0);
            let lack = missing.materials.get(&id).copied().unwrap_or(0);
            let crafted = crafted.get(&id).copied().unwrap_or(0);
            let mut tip = tip(&material.name, &CustomItem::Material(id), need, lack);
            if crafted > 0 {
                tip = t!("planner-tip-crafted", tip = tip, crafted = crafted);
            }
            KnownMaterial {
                id,
                need,
                lack,
                crafted,
                tip,
            }
        })
        .collect();

    let names: BTreeSet<&String> = needed.named.keys().chain(missing.named.keys()).collect();
    let named = (names.into_iter())
        .map(|name| {
            let need = needed.named.get(name).copied().unwrap_or(0);
            let lack = missing.named.get(name).copied().unwrap_or(0);
            let item = CustomItem::Named(name.clone());
            NamedMaterial {
                name: name.clone(),
                need,
                lack,
                tip: t!("planner-tip-unknown", tip = tip(name, &item, need, lack)),
            }
        })
        .collect();
    MaterialsModel { known, named }
}

#[cfg(test)]
mod tests {
    use wish_core::planner::{CharacterGoal, CustomMaterials, MORA, WeaponGoal};

    use super::*;

    fn data() -> &'static PlannerData {
        PlannerData::embedded()
    }

    fn entry(id: i64, target: PlanTarget) -> PlanEntry {
        PlanEntry { id, target }
    }

    fn ganyu(id: i64) -> PlanEntry {
        entry(
            id,
            PlanTarget::Character {
                key: "ganyu".to_owned(),
                goal: CharacterGoal::FULL,
            },
        )
    }

    fn build(plan: &[PlanEntry], inventory: &Inventory) -> PlannerModel {
        PlannerModel::build(plan, inventory, data(), &ResinOptions::default(), None)
    }

    #[test]
    fn the_battle_pass_counts_only_when_asked() {
        use wish_core::planner::{BattlePassData, RewardPlan};
        let rewards = BattlePassData::embedded().remaining(20, true, RewardPlan::NoArtifactExp);
        let plan = [ganyu(1)];
        let with = |counted| {
            PlannerModel::build(
                &plan,
                &Inventory::default(),
                data(),
                &ResinOptions::default(),
                Some(PassInput {
                    rewards: &rewards,
                    picks: BoxPicks::default(),
                    counted,
                }),
            )
        };
        let (off, on, none) = (with(false), with(true), build(&plan, &Inventory::default()));
        let lack = |model: &PlannerModel, id: MaterialId| {
            (model.total.known.iter())
                .find(|line| line.id == id)
                .map_or(0, |line| line.lack)
        };
        let mora = wish_core::planner::MORA;
        // Not counted: the totals are the plan's alone; the drawer still
        // knows where the boxes would go.
        assert_eq!(lack(&off, mora), lack(&none, mora));
        assert_eq!(off.pass.picked, on.pass.picked);
        assert!(!on.pass.picked.is_empty());
        assert_eq!(none.pass, PassModel::default());
        // Counted: Mora and Ganyu's books lack less, and the tooltip says the
        // pass brings some.
        assert!(lack(&on, mora) < lack(&off, mora));
        let (_, books, _) = on.pass.picked[0];
        assert!(lack(&on, books) < lack(&off, books));
        let tip = &(on.total.known.iter())
            .find(|line| line.id == mora)
            .unwrap()
            .tip;
        assert!(tip.contains("Battle Pass"), "{tip}");
        // Levels 21 to 50: the first level still to come is 21.
        assert_eq!(on.pass.from_level, 21);
    }

    #[test]
    fn each_card_says_what_it_is() {
        let custom = PlanTarget::CustomWeapon {
            name: "New sword".to_owned(),
            stars: 4,
            materials: CustomMaterials::new(),
            goal: WeaponGoal::full(4),
        };
        let gone = PlanTarget::Weapon {
            key: "not-a-weapon".to_owned(),
            goal: WeaponGoal::full(5),
        };
        let model = build(
            &[ganyu(1), entry(2, custom), entry(3, gone)],
            &Inventory::default(),
        );
        let [ganyu, sword, gone] = model.entries.as_slice() else {
            panic!("three cards");
        };
        assert_eq!(ganyu.name, "Ganyu");
        assert_eq!(ganyu.detail.as_deref(), Some("Cryo · Bow"));
        assert_eq!(ganyu.portrait, Portrait::Character("ganyu".to_owned(), 5));
        let ranges = ganyu.ranges.as_ref().unwrap();
        assert_eq!(ranges.level_text(), "1/20 → 90/90");
        assert_eq!(ranges.talents, [(1, 10); 3]);
        assert!(!ganyu.editable() && ganyu.warning.is_none());

        assert_eq!(sword.detail.as_deref(), Some("Custom weapon · 4★"));
        assert!(sword.editable());
        assert!(sword.ranges.as_ref().unwrap().talents.is_empty());

        assert_eq!(gone.name, "not-a-weapon");
        assert!(gone.warning.is_some());
        assert!(gone.materials.is_empty());
    }

    #[test]
    fn the_inventory_goes_to_the_first_card_and_the_tips_say_so() {
        let mut inventory = Inventory::default();
        inventory.set(&CustomItem::Material(MORA), 10_000_000);
        let model = build(&[ganyu(1), ganyu(2)], &inventory);
        let mora = |materials: &MaterialsModel| {
            materials
                .known
                .iter()
                .find(|m| m.id == MORA)
                .cloned()
                .unwrap()
        };
        // Enough for the first, not for both.
        let first = mora(&model.entries[0].materials);
        assert_eq!(first.lack, 0);
        assert!(
            first.tip.ends_with("covered by your inventory"),
            "{}",
            first.tip
        );
        let second = mora(&model.entries[1].materials);
        assert!(second.lack > 0 && second.lack < second.need);
        assert!(second.tip.contains("still to get"), "{}", second.tip);
        let total = mora(&model.total);
        assert_eq!(total.lack, second.lack);
        assert!(total.tip.contains("10,000,000 owned"), "{}", total.tip);
        assert_eq!(
            model.entries[0]
                .resin
                .runs(wish_core::planner::ResinSource::Mora),
            0
        );
        assert!(model.resin.resin() > 0);
    }

    #[test]
    fn materials_go_in_the_game_s_order_then_by_name() {
        let materials: CustomMaterials = wish_core::planner::Role::CHARACTER
            .iter()
            .map(|role| (*role, CustomItem::Named(format!("{role}"))))
            .collect();
        let custom = PlanTarget::CustomCharacter {
            name: "New".to_owned(),
            stars: 5,
            materials,
            goal: CharacterGoal::FULL,
        };
        let model = build(&[entry(1, custom)], &Inventory::default());
        let card = &model.entries[0].materials;
        let sorts: Vec<u32> = (card.known.iter())
            .map(|m| data().material(m.id).unwrap().sort)
            .collect();
        assert!(sorts.windows(2).all(|w| w[0] <= w[1]));
        let names: Vec<&str> = card.named.iter().map(|m| m.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
        assert!(card.named[0].tip.ends_with("(not in the game data)"));
        assert_eq!(
            card.named[0].label(),
            format!("{} × {}", format::count(card.named[0].lack), names[0])
        );
    }

    #[test]
    fn a_crafted_tier_says_how_many() {
        // Ganyu's books: 9 Teachings, 63 Guides, 114 Philosophies of
        // Diligence. 27 Teachings to spare make 9 Guides.
        let teachings = 104313;
        let mut inventory = Inventory::default();
        inventory.set(&CustomItem::Material(teachings), 9 + 27);
        let model = build(&[ganyu(1)], &inventory);
        let guides = (model.entries[0].materials.known.iter())
            .find(|m| m.id == teachings + 1)
            .unwrap();
        assert_eq!(guides.crafted, 9);
        assert_eq!(guides.lack, 63 - 9);
        assert!(
            guides.tip.ends_with("; 9 crafted from the tiers below"),
            "{}",
            guides.tip
        );
    }
}
