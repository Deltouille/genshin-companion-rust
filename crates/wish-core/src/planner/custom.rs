//! Custom characters and weapons: those the game data does not have yet (a
//! leak), made of the materials they use, as genshin-center's planner does.
//!
//! Every character but the Traveler follows one pattern: six materials, each
//! in a role (a common material, a local specialty, a boss material, an
//! elemental gem, a talent book, a weekly boss material), in the same
//! amounts. Every weapon of a rarity follows one too: a domain material, an
//! elite drop and a common drop. So a custom one costs what the first such
//! character (or weapon of its rarity) of the data costs, with its own
//! materials in place of that one's.

use std::collections::BTreeMap;

use super::{
    Character, CharacterGoal, CustomItem, Material, MaterialId, PlannerData, Requirement, Weapon,
    WeaponGoal,
};
use crate::model::text_enum;

/// The Crown of Insight, which every character's last talent level takes.
const CROWN_OF_INSIGHT: MaterialId = 104319;

text_enum! {
    /// What a material is for in a character's or a weapon's costs, besides
    /// Mora, EXP and the Crown of Insight.
    pub enum Role {
        Common => "common",
        LocalSpecialty => "local-specialty",
        BossMaterial => "boss-material",
        ElementalGem => "elemental-gem",
        TalentBook => "talent-book",
        WeeklyMaterial => "weekly-material",
        WeaponMaterial => "weapon-material",
        EliteDrop => "elite-drop",
    }
}

impl Role {
    /// A character's six roles, in genshin-center's order.
    pub const CHARACTER: [Role; 6] = [
        Role::Common,
        Role::LocalSpecialty,
        Role::BossMaterial,
        Role::ElementalGem,
        Role::TalentBook,
        Role::WeeklyMaterial,
    ];
    /// A weapon's three: its domain material, an elite and a common drop.
    pub const WEAPON: [Role; 3] = [Role::WeaponMaterial, Role::EliteDrop, Role::Common];

    pub fn label(self) -> &'static str {
        match self {
            Role::Common => "Common",
            Role::LocalSpecialty => "Local specialty",
            Role::BossMaterial => "Boss material",
            Role::ElementalGem => "Elemental gem",
            Role::TalentBook => "Talent book",
            Role::WeeklyMaterial => "Weekly material",
            Role::WeaponMaterial => "Weapon material",
            Role::EliteDrop => "Elite drop",
        }
    }

    /// How many tiers its materials come in: a common or elite drop or a
    /// talent book in three, a gem or a weapon material in four, the others
    /// alone.
    pub fn tiers(self) -> usize {
        match self {
            Role::Common | Role::EliteDrop | Role::TalentBook => 3,
            Role::ElementalGem | Role::WeaponMaterial => 4,
            Role::LocalSpecialty | Role::BossMaterial | Role::WeeklyMaterial => 1,
        }
    }

    /// The role of `material` in a character's costs, from what the game
    /// says it is for. The game files boss and weekly boss materials alike:
    /// `in_talents` tells them apart, since only the latter raise talents.
    fn of(material: &Material, in_talents: bool) -> Option<Role> {
        let kind = material.kind.as_str();
        Some(match kind {
            _ if material.id == CROWN_OF_INSIGHT => return None,
            "Character and Weapon Enhancement Material" => Role::Common,
            _ if kind.starts_with("Local Specialty") => Role::LocalSpecialty,
            "Character Level-Up Material" if in_talents => Role::WeeklyMaterial,
            "Character Level-Up Material" => Role::BossMaterial,
            "Character Ascension Material" => Role::ElementalGem,
            "Character Talent Material" => Role::TalentBook,
            _ => return None,
        })
    }
}

/// A custom character's or weapon's materials, by role: one the game data
/// knows (the lowest tier of its series), or a name for one it does not. A
/// role left out costs material named after the role.
pub type CustomMaterials = BTreeMap<Role, CustomItem>;

/// Materials by role, each series lowest tier first.
pub(super) type Roles = BTreeMap<Role, Vec<MaterialId>>;

/// Adds `id` to `role`'s series in `roles`: the costs take each series in
/// order, lowest tier first.
fn note(roles: &mut Roles, role: Role, id: MaterialId) {
    let tiers = roles.entry(role).or_default();
    if !tiers.contains(&id) {
        tiers.push(id);
    }
}

/// Whether `roles` are exactly `expected`, each series complete.
fn regular(roles: &Roles, expected: &[Role]) -> bool {
    roles.len() == expected.len()
        && expected.iter().all(|role| {
            roles
                .get(role)
                .is_some_and(|tiers| tiers.len() == role.tiers())
        })
}

impl PlannerData {
    /// `character`'s materials by role, or `None` when its costs don't follow
    /// the usual pattern (the Traveler's talents take three series of books).
    pub fn roles(&self, character: &Character) -> Option<Roles> {
        let mut roles = Roles::new();
        let steps = (character.ascension.iter().map(|step| (step, false)))
            .chain(character.talents.iter().map(|step| (step, true)));
        for (step, in_talents) in steps {
            for cost in step {
                let role = self
                    .material(cost.material)
                    .and_then(|material| Role::of(material, in_talents));
                if let Some(role) = role {
                    note(&mut roles, role, cost.material);
                }
            }
        }
        regular(&roles, &Role::CHARACTER).then_some(roles)
    }

    /// `weapon`'s materials by role, or `None` when its costs don't follow
    /// the pattern of the 3★ to 5★ weapons (1★ and 2★ ones have fewer tiers).
    /// An enemy drop is an elite's when its lowest tier is 2★.
    pub fn weapon_roles(&self, weapon: &Weapon) -> Option<Roles> {
        let mut roles = Roles::new();
        for cost in weapon.ascension.iter().flatten() {
            let Some(material) = self.material(cost.material) else {
                continue;
            };
            let role = match material.kind.as_str() {
                "Weapon Ascension Material" => Role::WeaponMaterial,
                "Character and Weapon Enhancement Material" => {
                    let lowest = (self.craft_family(cost.material))
                        .and_then(|series| self.material(series[0]))
                        .map_or(material.rarity, |m| m.rarity);
                    if lowest == 1 {
                        Role::Common
                    } else {
                        Role::EliteDrop
                    }
                }
                _ => continue,
            };
            note(&mut roles, role, cost.material);
        }
        regular(&roles, &Role::WEAPON).then_some(roles)
    }

    /// Works out, once the data is loaded, the series each role can take and
    /// the characters and weapons custom ones are modelled on.
    pub(super) fn index_roles(&mut self) {
        let mut families: BTreeMap<Role, Vec<Vec<MaterialId>>> = BTreeMap::new();
        let mut add = |roles: &Roles| {
            for (role, tiers) in roles {
                let known = families.entry(*role).or_default();
                if !known.contains(tiers) {
                    known.push(tiers.clone());
                }
            }
        };
        let mut template = None;
        for (index, character) in self.characters.iter().enumerate() {
            if let Some(roles) = self.roles(character) {
                add(&roles);
                template.get_or_insert((index, roles));
            }
        }
        let mut weapon_templates = BTreeMap::new();
        for (index, weapon) in self.weapons.iter().enumerate() {
            if let Some(roles) = self.weapon_roles(weapon) {
                add(&roles);
                weapon_templates
                    .entry(weapon.rarity)
                    .or_insert((index, roles));
            }
        }
        // In the game's inventory order.
        for series in families.values_mut() {
            series.sort_by_key(|tiers| {
                self.material(tiers[0])
                    .map_or((u32::MAX, tiers[0]), |m| (m.sort, m.id))
            });
        }
        self.families = families;
        self.template = template;
        self.weapon_templates = weapon_templates;
    }

    /// The series of materials a custom character or weapon can take for
    /// `role`, each lowest tier first: those of the data use.
    pub fn families(&self, role: Role) -> &[Vec<MaterialId>] {
        self.families.get(&role).map_or(&[], Vec::as_slice)
    }

    /// The series for `role` whose lowest tier is `first`.
    pub fn family(&self, role: Role, first: MaterialId) -> Option<&[MaterialId]> {
        self.families(role)
            .iter()
            .find(|tiers| tiers[0] == first)
            .map(Vec::as_slice)
    }

    /// What taking a custom character from `goal.from` to `goal.to` costs:
    /// the materials the game data knows, and the others by name. A tier of
    /// a material named by the player is named after it and its stars
    /// ("Silver Fern (2★)").
    pub fn custom_character_cost(
        &self,
        materials: &CustomMaterials,
        goal: &CharacterGoal,
    ) -> (Requirement, BTreeMap<String, u64>) {
        let Some((index, model)) = &self.template else {
            return (Requirement::default(), BTreeMap::new());
        };
        let modelled = self.character_cost(&self.characters[*index], goal);
        self.with_materials(modelled, model, materials)
    }

    /// What taking a custom weapon of `stars` stars from `goal.from` to
    /// `goal.to` costs, as [`PlannerData::custom_character_cost`].
    pub fn custom_weapon_cost(
        &self,
        materials: &CustomMaterials,
        stars: u8,
        goal: &WeaponGoal,
    ) -> (Requirement, BTreeMap<String, u64>) {
        let Some((index, model)) = self.weapon_templates.get(&stars) else {
            return (Requirement::default(), BTreeMap::new());
        };
        let modelled = self.weapon_cost(&self.weapons[*index], goal);
        self.with_materials(modelled, model, materials)
    }

    /// `modelled`, the cost of the character or weapon modelled on, with
    /// `materials` in place of its own (`model`).
    fn with_materials(
        &self,
        modelled: Requirement,
        model: &Roles,
        materials: &CustomMaterials,
    ) -> (Requirement, BTreeMap<String, u64>) {
        let mut named = BTreeMap::new();
        let mut need = Requirement {
            character_exp: modelled.character_exp,
            weapon_exp: modelled.weapon_exp,
            ..Requirement::default()
        };
        for (id, count) in modelled.materials {
            let place = model
                .iter()
                .find_map(|(role, tiers)| Some((*role, tiers.iter().position(|t| *t == id)?)));
            let Some((role, tier)) = place else {
                // Mora and the Crown of Insight: the same for everyone.
                *need.materials.entry(id).or_default() += count;
                continue;
            };
            let stars = self.material(id).map_or(0, |m| m.rarity);
            let name_tier = |name: &str| {
                if role.tiers() == 1 {
                    name.to_owned()
                } else {
                    format!("{name} ({stars}★)")
                }
            };
            let own = match materials.get(&role) {
                Some(CustomItem::Material(first)) => match self.family(role, *first) {
                    Some(tiers) => Ok(tiers[tier]),
                    // A series this data no longer has: keep it by name.
                    None => Err(name_tier(
                        &self
                            .material(*first)
                            .map_or_else(|| first.to_string(), |m| m.name.clone()),
                    )),
                },
                Some(CustomItem::Named(name)) => Err(name_tier(name)),
                None => Err(name_tier(role.label())),
            };
            match own {
                Ok(id) => *need.materials.entry(id).or_default() += count,
                Err(name) => *named.entry(name).or_default() += count,
            }
        }
        (need, named)
    }
}
