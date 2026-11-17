//! What the weapon pages show, from the game data and what the account owns:
//! the list (searched, filtered, sorted, what the account owns first, its
//! favourites leading) and a weapon's page (its stats at a level, what
//! reaching it costs, its passive at a refinement).
//!
//! Plain data from pure functions, tested without GPUI. The view builds them
//! as it draws: a few hundred weapons to sort, nothing costly.

use std::collections::{BTreeMap, BTreeSet};

use wish_core::archive::ArchiveData;
use wish_core::collection::Collection;
use wish_core::planner::{Level, MaterialId, PlannerData, Weapon, WeaponGoal};
use wish_ui::format;

use super::super::Sort;
use super::super::shared::release;

/// What narrows and orders the list.
pub(super) struct Filters<'a> {
    /// Lowercase; empty for none.
    pub query: &'a str,
    /// Weapon types ("Sword"…) and rarities shown; none means all.
    pub types: &'a BTreeSet<&'static str>,
    pub stars: &'a BTreeSet<u8>,
    /// Only the weapons with a copy marked as a favourite.
    pub favourites: bool,
    pub sort: Sort,
    /// The order turned round.
    pub reversed: bool,
}

impl Filters<'_> {
    /// Whether one narrows the list.
    pub fn any(&self) -> bool {
        !self.query.is_empty()
            || !self.types.is_empty()
            || !self.stars.is_empty()
            || self.favourites
    }
}

pub(super) struct WeaponList {
    /// What the account owns, its favourites first, each part in the chosen
    /// order.
    pub mine: Vec<WeaponCard>,
    pub others: Vec<WeaponCard>,
    /// How many of the game's weapons the account owns, and how many there
    /// are (whatever the filters).
    pub owned: usize,
    pub total: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct WeaponCard {
    pub key: String,
    pub name: String,
    pub rarity: u8,
    /// "Sword", "Bow"…
    pub kind: String,
    pub owned: bool,
    /// One of its copies is a favourite.
    pub favourite: bool,
    /// The best refinement, and the copies when more than one: "R3 · ×2".
    pub summary: Option<String>,
    /// Out in the latest version the data knows.
    pub new: bool,
}

impl WeaponCard {
    /// What a screen reader says of the card.
    pub fn label(&self) -> String {
        let (name, stars, kind) = (
            self.name.as_str(),
            self.rarity,
            format::weapon_type(&self.kind),
        );
        if self.owned {
            t!(
                "archive-weapon-label-owned",
                name = name,
                stars = stars,
                kind = kind
            )
        } else {
            t!(
                "archive-weapon-label",
                name = name,
                stars = stars,
                kind = kind
            )
        }
    }
}

/// The list as `filters` show it.
pub(super) fn list(
    planner: &PlannerData,
    archive: &ArchiveData,
    owned: &Collection,
    filters: &Filters,
) -> WeaponList {
    let version = |key: &str| {
        archive
            .weapon(key)
            .and_then(|page| page.version.as_deref())
            .map(release)
            .unwrap_or_default()
    };
    let latest = planner.weapons().iter().map(|w| version(&w.key)).max();
    let favourite = |key: &str| owned.copies(key).any(|c| c.favourite);
    let mut shown: Vec<&Weapon> = (planner.weapons().iter())
        .filter(|w| filters.query.is_empty() || w.name.to_lowercase().contains(filters.query))
        .filter(|w| filters.types.is_empty() || filters.types.contains(w.weapon.as_str()))
        .filter(|w| filters.stars.is_empty() || filters.stars.contains(&w.rarity))
        .filter(|w| !filters.favourites || favourite(&w.key))
        .collect();
    match filters.sort {
        Sort::Newest => shown.sort_by(|a, b| {
            version(&b.key)
                .cmp(&version(&a.key))
                .then_with(|| b.rarity.cmp(&a.rarity))
                .then_with(|| a.name.cmp(&b.name))
        }),
        Sort::Name => shown.sort_by(|a, b| a.name.cmp(&b.name)),
        Sort::Rarity => {
            shown.sort_by(|a, b| b.rarity.cmp(&a.rarity).then_with(|| a.name.cmp(&b.name)))
        }
    }
    if filters.reversed {
        shown.reverse();
    }
    // What the account owns first, its favourites leading; the others after,
    // each part in the chosen order.
    let (mut mine, others): (Vec<&Weapon>, Vec<&Weapon>) =
        (shown.into_iter()).partition(|w| owned.copies(&w.key).next().is_some());
    mine.sort_by_key(|w| !favourite(&w.key));
    let card = |weapon: &Weapon| {
        let copies: Vec<_> = owned.copies(&weapon.key).collect();
        let summary = (!copies.is_empty()).then(|| {
            let best = copies.iter().map(|c| c.refinement).max().unwrap_or(1);
            match copies.len() {
                1 => format!("R{best}"),
                n => format!("R{best} · ×{n}"),
            }
        });
        WeaponCard {
            key: weapon.key.clone(),
            name: weapon.name.to_string(),
            rarity: weapon.rarity,
            kind: weapon.weapon.clone(),
            owned: !copies.is_empty(),
            favourite: copies.iter().any(|c| c.favourite),
            summary,
            new: latest.is_some() && Some(version(&weapon.key)) == latest,
        }
    };
    WeaponList {
        mine: mine.into_iter().map(card).collect(),
        others: others.into_iter().map(card).collect(),
        owned: (planner.weapons().iter())
            .filter(|w| owned.copies(&w.key).next().is_some())
            .count(),
        total: planner.weapons().len(),
    }
}

/// A weapon's page at one level and one refinement.
pub(super) struct WeaponPage {
    pub name: String,
    pub rarity: u8,
    pub kind: String,
    pub description: Option<String>,
    /// Type, secondary stat, passive, version: those the data has.
    pub facts: [(String, Option<String>); 4],
    /// Its wish art's file name.
    pub art: Option<String>,
    /// Whether its refinements differ (1★ and 2★ weapons have none).
    pub refinable: bool,
    /// "Lv. 80/90".
    pub level_label: String,
    /// Base ATK and the secondary stat, formatted.
    pub stats: Vec<(String, String)>,
    /// What reaching that level takes from level 1, EXP as ores.
    pub cost: BTreeMap<MaterialId, u64>,
    pub passive: Option<Passive>,
}

pub(super) struct Passive {
    pub name: String,
    /// The refinement shown, 1 to `refinements`.
    pub refinement: usize,
    pub refinements: usize,
    pub text: String,
}

/// Weapon `key`'s page, at the level its slider is on (`level`, an index into
/// its rarity's levels) and `refinement`; `None` for a weapon the data lacks.
pub(super) fn page(
    planner: &PlannerData,
    archive: &ArchiveData,
    key: &str,
    level: usize,
    refinement: usize,
) -> Option<WeaponPage> {
    let (weapon, page) = (planner.weapon(key)?, archive.weapon(key)?);
    let levels = Level::weapon_steps(weapon.rarity);
    let level = levels[level.min(levels.len() - 1)];
    let stats = archive.weapon_stats(page, level);
    let mut rows = vec![(
        format::stat("Base ATK"),
        format::count(stats.attack.round() as u64),
    )];
    if let (Some(name), Some(value)) = (&page.stats.special, stats.special) {
        let value = if page.stats.special_percent {
            format::percent(value)
        } else {
            format::count(value.round() as u64)
        };
        rows.push((format::stat(name), value));
    }
    let cost = planner
        .weapon_cost(
            weapon,
            &WeaponGoal {
                from: Level::START,
                to: level,
            },
        )
        .materials_with_exp();
    let passive = page.passive.as_ref().map(|passive| {
        let refinement = refinement.clamp(1, passive.refinements.len());
        Passive {
            name: passive.name.to_string(),
            refinement,
            refinements: passive.refinements.len(),
            text: passive.refinements[refinement - 1].to_string(),
        }
    });
    Some(WeaponPage {
        name: weapon.name.to_string(),
        rarity: weapon.rarity,
        kind: weapon.weapon.clone(),
        description: page.description.as_deref().map(str::to_owned),
        facts: [
            (
                t!("archive-fact-type"),
                Some(format::weapon_type(&weapon.weapon)),
            ),
            (
                t!("archive-fact-secondary"),
                page.stats.special.as_deref().map(format::stat),
            ),
            (
                t!("archive-fact-passive"),
                page.passive.as_ref().map(|p| p.name.to_string()),
            ),
            (t!("archive-released-in"), page.version.clone()),
        ],
        art: page.art.clone(),
        refinable: page
            .passive
            .as_ref()
            .is_some_and(|p| p.refinements.len() > 1),
        level_label: t!("archive-level-cap", level = level.level, cap = level.cap()),
        stats: rows,
        cost,
        passive,
    })
}

#[cfg(test)]
mod tests {
    use wish_core::collection::OwnedWeapon;

    use super::*;

    fn data() -> (&'static PlannerData, &'static ArchiveData) {
        (PlannerData::embedded(), ArchiveData::embedded())
    }

    fn no_types() -> BTreeSet<&'static str> {
        BTreeSet::new()
    }

    fn filters<'a>(
        query: &'a str,
        types: &'a BTreeSet<&'static str>,
        stars: &'a BTreeSet<u8>,
        sort: Sort,
    ) -> Filters<'a> {
        Filters {
            query,
            types,
            stars,
            favourites: false,
            sort,
            reversed: false,
        }
    }

    fn copy(key: &str, refinement: u8, favourite: bool) -> OwnedWeapon {
        OwnedWeapon {
            refinement,
            favourite,
            ..OwnedWeapon::new(key)
        }
    }

    #[test]
    fn what_the_account_owns_comes_first_its_favourites_leading() {
        let (planner, archive) = data();
        let owned = Collection {
            weapons: vec![
                copy("amosbow", 1, false),
                copy("favoniussword", 5, false),
                copy("favoniussword", 3, false),
                copy("sacrificialbow", 2, true),
            ],
            ..Collection::default()
        };
        let (types, stars) = (no_types(), BTreeSet::new());
        let list = list(
            planner,
            archive,
            &owned,
            &filters("", &types, &stars, Sort::Name),
        );
        let mine: Vec<&str> = list.mine.iter().map(|w| w.key.as_str()).collect();
        assert_eq!(mine, ["sacrificialbow", "amosbow", "favoniussword"]);
        let favonius = &list.mine[2];
        assert_eq!(favonius.summary.as_deref(), Some("R5 · ×2"));
        assert_eq!(list.mine[1].summary.as_deref(), Some("R1"));
        assert!(list.mine[0].favourite);
        assert_eq!(list.owned, 3);
        assert_eq!(list.mine.len() + list.others.len(), list.total);
        assert!(list.others.iter().all(|w| !w.owned && w.summary.is_none()));
        assert_eq!(favonius.label(), "Favonius Sword, 4★ Sword, owned");
    }

    #[test]
    fn the_filters_narrow_and_the_sort_orders() {
        let (planner, archive) = data();
        let owned = Collection::default();
        let bows = BTreeSet::from(["Bow"]);
        let five = BTreeSet::from([5]);
        let list = list(
            planner,
            archive,
            &owned,
            &filters("", &bows, &five, Sort::Name),
        );
        assert!(!list.others.is_empty());
        assert!(list.others.iter().all(|w| w.kind == "Bow" && w.rarity == 5));
        let names: Vec<&str> = list.others.iter().map(|w| w.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);

        let (types, stars) = (no_types(), BTreeSet::new());
        let found = super::list(
            planner,
            archive,
            &owned,
            &filters("amos", &types, &stars, Sort::Newest),
        );
        assert_eq!(found.others.len(), 1);
        assert!(filters("amos", &types, &stars, Sort::Newest).any());
        assert!(!filters("", &types, &stars, Sort::Newest).any());

        // The newest first, marked new.
        let all = super::list(
            planner,
            archive,
            &owned,
            &filters("", &types, &stars, Sort::Newest),
        );
        assert!(all.others[0].new);
        assert!(!all.others.last().unwrap().new);
    }

    #[test]
    fn a_weapon_s_page_at_a_level_and_a_refinement() {
        let (planner, archive) = data();
        let top = Level::weapon_steps(5).len() - 1;
        let amos = page(planner, archive, "amosbow", top, 9).unwrap();
        assert_eq!(amos.level_label, "Lv. 90/90");
        assert_eq!(amos.stats[0].0, "Base ATK");
        assert_eq!(amos.stats[0].1, "608");
        assert!(amos.stats[1].1.ends_with('%'), "{:?}", amos.stats);
        assert!(amos.refinable);
        let passive = amos.passive.unwrap();
        assert_eq!((passive.refinement, passive.refinements), (5, 5));
        assert!(!amos.cost.is_empty());
        assert_eq!(amos.facts[0], ("Type".to_owned(), Some("Bow".to_owned())));

        let start = page(planner, archive, "amosbow", 0, 1).unwrap();
        assert_eq!(start.level_label, "Lv. 1/20");
        assert!(start.cost.is_empty());
        assert!(page(planner, archive, "not-a-weapon", 0, 1).is_none());
    }
}
