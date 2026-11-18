//! The Resources page as plain data: every material of the game data
//! (placeholders apart) in its section, sorted as the game's bag sorts them,
//! with how many the account has; a search and "Owned only" filter them.
//! Pure functions, tested without GPUI.
//!
//! The game files boss and weekly boss materials as one kind, and common and
//! elite enemy drops as another: the costs that use them tell them apart, as
//! the planner's roles do ([`PlannerData::families`]).

use std::collections::{BTreeMap, HashSet};

use wish_core::planner::{CustomItem, Inventory, Material, MaterialId, PlannerData, Role};

/// The page's sections, in the order it shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Kind {
    /// Mora, the EXP books and the enhancement ores.
    MoraAndExp,
    Gems,
    BossMaterials,
    WeeklyMaterials,
    /// The talent books, and the Crown of Insight.
    TalentBooks,
    WeaponMaterials,
    CommonDrops,
    EliteDrops,
    /// Each region's apart.
    LocalSpecialties,
    /// A kind the app doesn't know yet.
    Other,
}

impl Kind {
    fn title(self) -> String {
        match self {
            Kind::MoraAndExp => t!("resources-mora-exp"),
            Kind::Gems => t!("resources-gems"),
            Kind::BossMaterials => t!("resources-boss"),
            Kind::WeeklyMaterials => t!("resources-weekly"),
            Kind::TalentBooks => t!("resources-talent"),
            Kind::WeaponMaterials => t!("resources-weapon"),
            Kind::CommonDrops => t!("resources-common-drops"),
            Kind::EliteDrops => t!("resources-elite-drops"),
            Kind::LocalSpecialties => t!("resources-local-specialties"),
            Kind::Other => t!("resources-other"),
        }
    }
}

/// The materials the costs use as elite drops, weekly boss and boss
/// materials.
struct Uses {
    elite: HashSet<MaterialId>,
    weekly: HashSet<MaterialId>,
    boss: HashSet<MaterialId>,
}

impl Uses {
    fn new(planner: &PlannerData) -> Self {
        let of = |role| planner.families(role).iter().flatten().copied().collect();
        Self {
            elite: of(Role::EliteDrop),
            weekly: of(Role::WeeklyMaterial),
            boss: of(Role::BossMaterial),
        }
    }
}

/// The section of `material`.
fn kind_of(material: &Material, uses: &Uses) -> Kind {
    let id = material.id;
    if uses.elite.contains(&id) {
        return Kind::EliteDrops;
    }
    if uses.weekly.contains(&id) {
        return Kind::WeeklyMaterials;
    }
    if uses.boss.contains(&id) {
        return Kind::BossMaterials;
    }
    match material.kind.as_str() {
        "Common Currency" | "Character EXP Material" | "Weapon Enhancement Material" => {
            Kind::MoraAndExp
        }
        "Character Ascension Material" => Kind::Gems,
        // One no cost uses yet: a boss's, most likely.
        "Character Level-Up Material" => Kind::BossMaterials,
        "Character Talent Material" => Kind::TalentBooks,
        "Weapon Ascension Material" => Kind::WeaponMaterials,
        "Character and Weapon Enhancement Material" => Kind::CommonDrops,
        kind if kind.starts_with("Local Specialty") => Kind::LocalSpecialties,
        _ => Kind::Other,
    }
}

/// A local specialty's region: "Local Specialty (Liyue)" is Liyue's.
fn region(kind: &str) -> Option<&str> {
    kind.strip_prefix("Local Specialty (")?.strip_suffix(')')
}

/// One material on the page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Tile {
    pub id: MaterialId,
    /// In the current language.
    pub name: String,
    pub stars: u8,
    /// How many the account has.
    pub owned: u64,
    /// "Hero's Wit · 121".
    pub tip: String,
}

/// Tiles under a title of their own (a region's local specialties), or
/// none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Group {
    pub title: Option<String>,
    pub tiles: Vec<Tile>,
}

/// One section of the page: a kind of material, as the game's bag groups them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Section {
    pub kind: Kind,
    pub title: String,
    /// How many tiles it shows.
    pub count: usize,
    pub groups: Vec<Group>,
}

/// The Resources page as plain data: its sections, searched and filtered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Resources {
    /// The sections with something to show, in order.
    pub sections: Vec<Section>,
    /// How many materials the account has some of, and how many there are.
    pub owned: usize,
    pub total: usize,
}

/// The page for `inventory`: the materials whose name (in the current
/// language or in English) holds `query`, those owned only with
/// `owned_only`.
pub(super) fn resources(
    planner: &PlannerData,
    inventory: &Inventory,
    query: &str,
    owned_only: bool,
) -> Resources {
    let uses = Uses::new(planner);
    let query = query.trim().to_lowercase();
    let materials: Vec<&Material> = (planner.materials().iter())
        .filter(|m| !m.is_placeholder())
        .collect();
    let owned_of = |m: &Material| inventory.owned(&CustomItem::Material(m.id));
    let owned = materials.iter().filter(|m| owned_of(m) > 0).count();
    let mut sections: BTreeMap<Kind, Vec<Group>> = BTreeMap::new();
    for material in &materials {
        let count = owned_of(material);
        let matches = query.is_empty()
            || material.name.to_lowercase().contains(&query)
            || material.name.english().to_lowercase().contains(&query);
        if !matches || (owned_only && count == 0) {
            continue;
        }
        let kind = kind_of(material, &uses);
        let title = match kind {
            Kind::LocalSpecialties => region(&material.kind).map(str::to_owned),
            _ => None,
        };
        let groups = sections.entry(kind).or_default();
        let at = match groups.iter().position(|g| g.title == title) {
            Some(at) => at,
            None => {
                groups.push(Group {
                    title,
                    tiles: Vec::new(),
                });
                groups.len() - 1
            }
        };
        groups[at].tiles.push(Tile {
            id: material.id,
            name: material.name.to_string(),
            stars: material.rarity,
            owned: count,
            tip: t!(
                "resources-tip",
                name = material.name.to_string(),
                count = count
            ),
        });
    }
    Resources {
        sections: (sections.into_iter())
            .map(|(kind, groups)| Section {
                kind,
                title: kind.title(),
                count: groups.iter().map(|g| g.tiles.len()).sum(),
                groups,
            })
            .collect(),
        owned,
        total: materials.len(),
    }
}

#[cfg(test)]
mod tests {
    use wish_core::planner::MORA;

    use super::*;

    fn planner() -> &'static PlannerData {
        PlannerData::embedded()
    }

    fn id(name: &str) -> MaterialId {
        (planner().materials().iter())
            .find(|m| m.name.english() == name)
            .unwrap_or_else(|| panic!("no {name}"))
            .id
    }

    /// The section and group title where `name` shows.
    fn place(page: &Resources, name: &str) -> (Kind, Option<String>) {
        let id = id(name);
        for section in &page.sections {
            for group in &section.groups {
                if group.tiles.iter().any(|t| t.id == id) {
                    return (section.kind, group.title.clone());
                }
            }
        }
        panic!("{name} isn't shown");
    }

    #[test]
    fn every_material_has_its_section() {
        let page = resources(planner(), &Inventory::default(), "", false);
        let shown: usize = page.sections.iter().map(|s| s.count).sum();
        assert_eq!(shown, page.total);
        assert!(
            (planner().materials().iter())
                .filter(|m| m.is_placeholder())
                .all(|m| !page.sections.iter().any(|s| s
                    .groups
                    .iter()
                    .any(|g| g.tiles.iter().any(|t| t.id == m.id)))),
            "no placeholder"
        );
        assert!(
            !page.sections.iter().any(|s| s.kind == Kind::Other),
            "a kind the page doesn't sort: {:?}",
            page.sections.last().map(|s| &s.groups)
        );
        for (name, kind) in [
            ("Mora", Kind::MoraAndExp),
            ("Hero's Wit", Kind::MoraAndExp),
            ("Mystic Enhancement Ore", Kind::MoraAndExp),
            ("Brilliant Diamond Sliver", Kind::Gems),
            ("Hurricane Seed", Kind::BossMaterials),
            ("Dvalin's Plume", Kind::WeeklyMaterials),
            ("Teachings of Freedom", Kind::TalentBooks),
            ("Crown of Insight", Kind::TalentBooks),
            ("Tile of Decarabian's Tower", Kind::WeaponMaterials),
            ("Slime Condensate", Kind::CommonDrops),
            ("Chaos Device", Kind::EliteDrops),
        ] {
            assert_eq!(place(&page, name), (kind, None), "{name}");
        }
        assert_eq!(
            place(&page, "Cecilia"),
            (Kind::LocalSpecialties, Some("Mondstadt".to_owned()))
        );
        // A series stays together, lowest tier first.
        let talent = &page
            .sections
            .iter()
            .find(|s| s.kind == Kind::TalentBooks)
            .unwrap();
        let first: Vec<MaterialId> = talent.groups[0]
            .tiles
            .iter()
            .take(3)
            .map(|t| t.id)
            .collect();
        assert_eq!(
            first,
            [
                id("Teachings of Freedom"),
                id("Guide to Freedom"),
                id("Philosophies of Freedom")
            ]
        );
    }

    #[test]
    fn a_search_and_owned_only_narrow_it() {
        let inventory = Inventory {
            materials: BTreeMap::from([(MORA, 420_000), (id("Guide to Freedom"), 12)]),
            named: BTreeMap::new(),
        };
        let page = resources(planner(), &inventory, "", true);
        assert_eq!(page.owned, 2);
        let ids: Vec<MaterialId> = (page.sections.iter())
            .flat_map(|s| s.groups.iter().flat_map(|g| g.tiles.iter().map(|t| t.id)))
            .collect();
        assert_eq!(ids, [MORA, id("Guide to Freedom")]);
        let guide = &page.sections[1].groups[0].tiles[0];
        assert_eq!(guide.tip, "Guide to Freedom · 12");

        let found = resources(planner(), &inventory, " freedom ", false);
        assert_eq!(found.sections.len(), 1);
        assert_eq!(found.sections[0].count, 3);
        assert_eq!(
            found.total, page.total,
            "the totals don't follow the search"
        );
        assert!(
            resources(planner(), &inventory, "no such thing", false)
                .sections
                .is_empty()
        );
    }
}
