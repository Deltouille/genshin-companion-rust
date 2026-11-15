//! What the character pages show, from the game data and what the account
//! owns: the list (searched, filtered, sorted, what the account owns first,
//! its favourites leading) and a character's numbers (its stats at the level
//! slider and what reaching it costs, the stats at each ascension, what a
//! talent level costs).
//!
//! Plain data from pure functions, tested without GPUI. The view builds them
//! as it draws: a hundred characters to sort, a few numbers to look up.

use std::collections::{BTreeMap, BTreeSet};

use wish_core::archive::{ArchiveData, CharacterPage};
use wish_core::collection::Collection;
use wish_core::planner::{Character, CharacterGoal, Level, MaterialId, PlannerData};
use wish_ui::format;

use super::super::Sort;
use super::super::shared::release;

/// What narrows and orders the list.
pub(super) struct Filters<'a> {
    /// Lowercase; empty for none.
    pub query: &'a str,
    /// Elements, weapon types and rarities shown; none means all.
    pub elements: &'a BTreeSet<&'static str>,
    pub weapons: &'a BTreeSet<&'static str>,
    pub stars: &'a BTreeSet<u8>,
    /// Only the account's favourites.
    pub favourites: bool,
    pub sort: Sort,
    /// The order turned round.
    pub reversed: bool,
}

impl Filters<'_> {
    /// Whether one narrows the list.
    pub fn any(&self) -> bool {
        !self.query.is_empty()
            || !self.elements.is_empty()
            || !self.weapons.is_empty()
            || !self.stars.is_empty()
            || self.favourites
    }
}

pub(super) struct CharacterList {
    /// What the account owns, its favourites first, each part in the chosen
    /// order.
    pub mine: Vec<CharacterCard>,
    pub others: Vec<CharacterCard>,
    /// How many characters the account owns, and how many the game has
    /// (whatever the filters).
    pub owned: usize,
    pub total: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CharacterCard {
    pub key: String,
    pub name: String,
    pub rarity: u8,
    pub element: String,
    pub weapon: String,
    /// What the account has of it: "Lv. 90 · C2", and whether it is a
    /// favourite.
    pub owned: Option<(String, bool)>,
    /// Out in the latest version the data knows.
    pub new: bool,
}

impl CharacterCard {
    /// What a screen reader says of the card.
    pub fn label(&self) -> String {
        format!(
            "{}, {}★ {} {}{}",
            self.name,
            self.rarity,
            self.element,
            self.weapon,
            if self.owned.is_some() { ", owned" } else { "" }
        )
    }
}

/// The list as `filters` show it.
pub(super) fn list(
    planner: &PlannerData,
    archive: &ArchiveData,
    owned: &Collection,
    filters: &Filters,
) -> CharacterList {
    let version = |key: &str| {
        archive
            .character(key)
            .and_then(|page| page.version.as_deref())
            .map(release)
            .unwrap_or_default()
    };
    let latest = planner.characters().iter().map(|c| version(&c.key)).max();
    let mut shown: Vec<&Character> = (planner.characters().iter())
        .filter(|c| filters.query.is_empty() || c.name.to_lowercase().contains(filters.query))
        .filter(|c| filters.elements.is_empty() || filters.elements.contains(c.element.as_str()))
        .filter(|c| filters.weapons.is_empty() || filters.weapons.contains(c.weapon.as_str()))
        .filter(|c| filters.stars.is_empty() || filters.stars.contains(&c.rarity))
        .filter(|c| !filters.favourites || owned.character(&c.key).is_some_and(|o| o.favourite))
        .collect();
    match filters.sort {
        Sort::Newest => shown.sort_by(|a, b| {
            version(&b.key)
                .cmp(&version(&a.key))
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
    let (mut mine, others): (Vec<&Character>, Vec<&Character>) =
        (shown.into_iter()).partition(|c| owned.character(&c.key).is_some());
    mine.sort_by_key(|c| !owned.character(&c.key).is_some_and(|o| o.favourite));
    let card = |character: &Character| CharacterCard {
        key: character.key.clone(),
        name: character.name.clone(),
        rarity: character.rarity,
        element: character.element.clone(),
        weapon: character.weapon.clone(),
        owned: owned.character(&character.key).map(|o| {
            (
                format!("Lv. {} · C{}", o.level.level, o.constellation),
                o.favourite,
            )
        }),
        new: latest.is_some() && Some(version(&character.key)) == latest,
    };
    CharacterList {
        mine: mine.into_iter().map(card).collect(),
        others: others.into_iter().map(card).collect(),
        owned: owned.characters.len(),
        total: planner.characters().len(),
    }
}

/// A character's stats at one level, and what reaching it costs.
pub(super) struct Stats {
    /// "Lv. 80/90".
    pub level_label: String,
    /// Base HP, ATK and DEF, then the stat its ascensions raise.
    pub rows: [(String, String); 4],
    /// From level 1, EXP as books.
    pub cost: BTreeMap<MaterialId, u64>,
}

/// `character`'s stats at the level its slider is on (`level`, an index into
/// [`Level::steps`]).
pub(super) fn stats(
    planner: &PlannerData,
    archive: &ArchiveData,
    character: &Character,
    page: &CharacterPage,
    level: usize,
) -> Stats {
    let levels = Level::steps();
    let level = levels[level.min(levels.len() - 1)];
    let stats = archive.stats(page, level);
    let count = |value: f64| format::count(value.round() as u64);
    let cost = planner
        .character_cost(
            character,
            &CharacterGoal {
                from: Level::START,
                to: level,
                talents: [(1, 1); 3],
            },
        )
        .materials_with_exp();
    Stats {
        level_label: format!("Lv. {}/{}", level.level, level.cap()),
        rows: [
            ("Base HP".to_owned(), count(stats.hp)),
            ("Base ATK".to_owned(), count(stats.attack)),
            ("Base DEF".to_owned(), count(stats.defense)),
            (page.stats.special.clone(), special(page, stats.special)),
        ],
        cost,
    }
}

/// The stat ascensions raise, as the game writes it: Elemental Mastery as a
/// number, the others as a percentage.
fn special(page: &CharacterPage, value: f64) -> String {
    if page.stats.special == "Elemental Mastery" {
        format::count(value.round() as u64)
    } else {
        format!("{:.1}%", value * 100.0)
    }
}

/// The stats at each bound of each ascension phase, as the game writes the
/// level: "20/20" before ascending, "20/40" after. Level, HP, ATK, DEF and
/// the ascension stat.
pub(super) fn ascension(archive: &ArchiveData, page: &CharacterPage) -> Vec<[String; 5]> {
    const BOUNDS: [(u8, u8); 14] = [
        (1, 0),
        (20, 0),
        (20, 1),
        (40, 1),
        (40, 2),
        (50, 2),
        (50, 3),
        (60, 3),
        (60, 4),
        (70, 4),
        (70, 5),
        (80, 5),
        (80, 6),
        (90, 6),
    ];
    (BOUNDS.iter())
        .filter_map(|&(level, phase)| {
            let level = Level::new(level, phase)?;
            let stats = archive.stats(page, level);
            Some([
                format!("{}/{}", level.level, level.cap()),
                format::count(stats.hp.round() as u64),
                format::count(stats.attack.round() as u64),
                format::count(stats.defense.round() as u64),
                special(page, stats.special),
            ])
        })
        .collect()
}

/// What raising one talent to `level` says and costs.
pub(super) struct TalentCost {
    pub level: u8,
    pub note: String,
    /// From level 1; none when there is nothing to spend (level 1). Levels
    /// past 10 come from constellations: the materials stop at 10.
    pub cost: Option<BTreeMap<MaterialId, u64>>,
}

pub(super) fn talent_cost(planner: &PlannerData, character: &Character, level: u8) -> TalentCost {
    let to = level.min(10);
    let need = planner.character_cost(
        character,
        &CharacterGoal {
            from: Level::START,
            to: Level::START,
            talents: [(1, to), (1, 1), (1, 1)],
        },
    );
    TalentCost {
        level,
        note: if level > 10 {
            "Levels 11 to 15 come from constellations; the materials stop at 10.".to_owned()
        } else {
            format!("Raising one talent from 1 to {level}:")
        },
        cost: (to > 1).then(|| need.materials_with_exp()),
    }
}

#[cfg(test)]
mod tests {
    use wish_core::collection::OwnedCharacter;
    use wish_core::planner::MORA;

    use super::*;

    fn data() -> (&'static PlannerData, &'static ArchiveData) {
        (PlannerData::embedded(), ArchiveData::embedded())
    }

    #[test]
    fn what_the_account_owns_comes_first_its_favourites_leading() {
        let (planner, archive) = data();
        let mut bennett = OwnedCharacter::new("bennett");
        bennett.constellation = 6;
        let mut xiangling = OwnedCharacter::new("xiangling");
        xiangling.favourite = true;
        let owned = Collection {
            characters: vec![OwnedCharacter::new("ganyu"), bennett, xiangling],
            ..Collection::default()
        };
        let none = BTreeSet::new();
        let stars = BTreeSet::new();
        let filters = Filters {
            query: "",
            elements: &none,
            weapons: &none,
            stars: &stars,
            favourites: false,
            sort: Sort::Name,
            reversed: false,
        };
        assert!(!filters.any());
        let list = list(planner, archive, &owned, &filters);
        let mine: Vec<&str> = list.mine.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(mine, ["xiangling", "bennett", "ganyu"]);
        assert_eq!(list.mine[1].owned, Some(("Lv. 1 · C6".to_owned(), false)));
        assert_eq!(list.mine[1].label(), "Bennett, 4★ Pyro Sword, owned");
        assert_eq!(list.owned, 3);
        assert_eq!(list.mine.len() + list.others.len(), list.total);

        // Pyro bows only.
        let pyro = BTreeSet::from(["Pyro"]);
        let bows = BTreeSet::from(["Bow"]);
        let narrowed = Filters {
            elements: &pyro,
            weapons: &bows,
            ..filters
        };
        assert!(narrowed.any());
        let list = super::list(planner, archive, &owned, &narrowed);
        assert!(list.mine.is_empty());
        assert!(!list.others.is_empty());
        assert!(
            list.others
                .iter()
                .all(|c| c.element == "Pyro" && c.weapon == "Bow")
        );
    }

    #[test]
    fn a_character_s_numbers() {
        let (planner, archive) = data();
        let ganyu = planner.character("ganyu").unwrap();
        let page = archive.character("ganyu").unwrap();
        let top = Level::steps().len() - 1;
        let at_90 = stats(planner, archive, ganyu, page, top);
        assert_eq!(at_90.level_label, "Lv. 90/90");
        assert_eq!(at_90.rows[3].0, "CRIT DMG");
        assert!(at_90.rows[3].1.ends_with('%'), "{:?}", at_90.rows);
        assert!(at_90.cost.contains_key(&MORA));
        assert!(stats(planner, archive, ganyu, page, 0).cost.is_empty());

        let rows = ascension(archive, page);
        assert_eq!(rows.len(), 14);
        assert_eq!(
            (rows[0][0].as_str(), rows[13][0].as_str()),
            ("1/20", "90/90")
        );

        assert!(talent_cost(planner, ganyu, 1).cost.is_none());
        let six = talent_cost(planner, ganyu, 6);
        assert_eq!(six.note, "Raising one talent from 1 to 6:");
        let twelve = talent_cost(planner, ganyu, 12);
        assert!(twelve.note.starts_with("Levels 11 to 15"));
        // Past 10, the materials of 10.
        assert_eq!(twelve.cost, talent_cost(planner, ganyu, 10).cost);
    }
}
