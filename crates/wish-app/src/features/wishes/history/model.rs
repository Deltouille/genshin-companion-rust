//! What the history page shows of one banner type's wishes, as pure
//! functions tested without GPUI: which rows, in which order (newest first,
//! or by a column, a 10-pull kept in sequence), a page at a time, each cell's
//! text, and the page's subtitle. The table's delegate (`table.rs`) draws them.

use std::cmp::Ordering;
use std::ops::Range;

use wish_core::data::GameData;
use wish_core::engine::{PoolAnalysis, Pull, RateUpOutcome};
use wish_core::model::{ItemType, Pool, Rarity, Wish};
use wish_ui::format;

/// The page sizes offered under the table.
pub(super) const PAGE_SIZES: [usize; 3] = [25, 50, 100];

/// A column of the wish table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Col {
    Item,
    /// Rarity and character or weapon: "5★ Character". One column for both,
    /// so the table fits a 1280 px window.
    Kind,
    Pity,
    /// Only on pools with rate-up items.
    RateUp,
    Banner,
    /// The wish's number on its banner.
    Roll,
    Time,
}

impl Col {
    /// Its header, in the current language.
    pub fn name(self) -> String {
        match self {
            Col::Item => t!("history-col-item"),
            Col::Kind => t!("history-col-type"),
            Col::Pity => t!("history-col-pity"),
            Col::RateUp => t!("history-col-rate-up"),
            Col::Banner => t!("history-col-banner"),
            Col::Roll => "#".to_owned(),
            Col::Time => t!("history-col-time"),
        }
    }

    /// Its id, the same in every language.
    pub fn key(self) -> &'static str {
        match self {
            Col::Item => "item",
            Col::Kind => "type",
            Col::Pity => "pity",
            Col::RateUp => "rate-up",
            Col::Banner => "banner",
            Col::Roll => "roll",
            Col::Time => "time",
        }
    }

    /// The columns of `pool`'s table: the rate-up only where items have one.
    pub fn of(pool: Pool) -> Vec<Col> {
        [
            Col::Item,
            Col::Kind,
            Col::Pity,
            Col::RateUp,
            Col::Banner,
            Col::Roll,
            Col::Time,
        ]
        .into_iter()
        .filter(|col| *col != Col::RateUp || pool.has_rate_up())
        .collect()
    }
}

/// Which way a column sorts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Order {
    Ascending,
    Descending,
}

/// The index of a rarity in the page's filter: 5★, 4★, 3★.
pub(super) fn rarity_ix(rarity: Rarity) -> usize {
    match rarity {
        Rarity::Five => 0,
        Rarity::Four => 1,
        Rarity::Three => 2,
    }
}

/// The pulls `filter` keeps (by rarity: 5★, 4★, 3★), as indexes into
/// `pulls`: newest first, or by `sort`'s column. Equal keys keep the wishes'
/// order, so a 10-pull stays in sequence. `rolls`: each pull's number on its
/// banner.
pub(super) fn arrange(
    pulls: &[Pull],
    rolls: &[Option<u32>],
    filter: [bool; 3],
    sort: Option<(Col, Order)>,
) -> Vec<usize> {
    let mut rows: Vec<usize> = (0..pulls.len())
        .filter(|&ix| filter[rarity_ix(pulls[ix].wish.rarity)])
        .collect();
    match sort {
        Some((col, order)) => {
            rows.sort_by(|&a, &b| compare(pulls, rolls, col, a, b).then(a.cmp(&b)));
            if order == Order::Descending {
                rows.reverse();
            }
        }
        None => rows.reverse(),
    }
    rows
}

fn compare(pulls: &[Pull], rolls: &[Option<u32>], col: Col, a: usize, b: usize) -> Ordering {
    let roll = |ix: usize| rolls.get(ix).copied().flatten();
    let (roll_a, roll_b) = (roll(a), roll(b));
    let (a, b) = (&pulls[a], &pulls[b]);
    let outcome = |pull: &Pull| {
        pull.rate_up.map(|r| match r.outcome() {
            RateUpOutcome::Won => 0,
            RateUpOutcome::Guaranteed => 1,
            RateUpOutcome::Lost => 2,
            RateUpOutcome::LostWhileGuaranteed => 3,
        })
    };
    match col {
        Col::Item => item_name(&a.wish).cmp(&item_name(&b.wish)),
        Col::Kind => {
            (a.wish.rarity, kind(a.wish.item_type)).cmp(&(b.wish.rarity, kind(b.wish.item_type)))
        }
        Col::Pity => a.pity.cmp(&b.pity),
        Col::RateUp => outcome(a).cmp(&outcome(b)),
        Col::Banner => a.banner.cmp(&b.banner),
        Col::Roll => roll_a.cmp(&roll_b),
        // Wish order is time order, with ties broken by sequence.
        Col::Time => Ordering::Equal,
    }
}

/// The page shown (from 0) and how many rows a page holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Paging {
    pub page: usize,
    pub size: usize,
}

impl Paging {
    pub fn first() -> Self {
        Self {
            page: 0,
            size: PAGE_SIZES[0],
        }
    }

    /// How many pages `rows` fill (at least one, so an empty table has a
    /// page).
    pub fn pages(self, rows: usize) -> usize {
        rows.div_ceil(self.size).max(1)
    }

    /// Page `page`, or the last of `rows`'.
    pub fn at(self, page: usize, rows: usize) -> Self {
        Self {
            page: page.min(self.pages(rows) - 1),
            ..self
        }
    }

    /// Pages of `size` rows, on the one that holds the first row shown.
    pub fn resized(self, size: usize) -> Self {
        Self {
            page: self.page * self.size / size,
            size,
        }
    }

    /// The rows of the page, as indexes into the rows.
    pub fn range(self, rows: usize) -> Range<usize> {
        let start = (self.page * self.size).min(rows);
        start..(start + self.size).min(rows)
    }

    /// "26–50 of 120", or "No wishes".
    pub fn shown(self, rows: usize) -> String {
        if rows == 0 {
            return t!("history-none");
        }
        let range = self.range(rows);
        t!(
            "history-shown",
            first = range.start + 1,
            last = range.end,
            total = rows
        )
    }
}

/// Each banner of `pool`, as the table names it: "Raiden Shogun · 4.3".
pub(super) fn banner_labels(data: &GameData, pool: Pool) -> Vec<String> {
    (data.banners.for_pool(pool).iter())
        .map(|banner| match &banner.version {
            Some(version) if !banner.short_name.is_empty() => {
                format!("{} · {version}", banner.short_name)
            }
            _ => banner.name.clone(),
        })
        .collect()
}

/// A cell's text: `pull`'s, in `col`. `roll`: its number on its banner.
pub(super) fn cell(pull: &Pull, col: Col, banner_labels: &[String], roll: Option<u32>) -> String {
    match col {
        Col::Time => pull.wish.time.naive().format("%Y-%m-%d %H:%M").to_string(),
        Col::Item => item_name(&pull.wish),
        Col::Kind => match kind(pull.wish.item_type) {
            None => format!("{}★", pull.wish.rarity.stars()),
            Some(kind) => format!("{}★ {kind}", pull.wish.rarity.stars()),
        },
        Col::Pity => pull.pity.map(|p| p.to_string()).unwrap_or_default(),
        Col::RateUp => pull
            .rate_up
            .map(|r| format::outcome(r.outcome()))
            .unwrap_or_default(),
        Col::Banner => (pull.banner)
            .and_then(|b| banner_labels.get(b))
            .cloned()
            .unwrap_or_default(),
        Col::Roll => roll.map(|roll| roll.to_string()).unwrap_or_default(),
    }
}

/// "Character" or "Weapon"; nothing for a 3★ counted by hand, whose item is
/// not known.
pub(super) fn kind(item_type: ItemType) -> Option<String> {
    match item_type {
        ItemType::Character => Some(t!("history-character")),
        ItemType::Weapon => Some(t!("history-weapon")),
        ItemType::Unknown => None,
    }
}

/// A wish's item as the table names it, in the current language: a 3★
/// counted by hand, whose item isn't known, is "Unknown".
pub(super) fn item_name(wish: &Wish) -> String {
    if wish.item_type == ItemType::Unknown && wish.name == wish_core::import::manual::UNKNOWN_ITEM {
        t!("history-unknown-item")
    } else {
        wish_core::i18n::item_name(&wish.name).to_owned()
    }
}

/// "52 wishes · 5★ pity 12 / 90 · 4★ pity 3 / 10 · next 5★: 50/50".
pub(super) fn subtitle(pool: &PoolAnalysis) -> String {
    let counters = pool.counters;
    let mut parts = vec![
        t!("history-wish-count", count = counters.total),
        t!(
            "history-five-pity",
            pity = counters.pity5,
            hard = pool.pool.hard_pity_five()
        ),
        t!(
            "history-four-pity",
            pity = counters.pity4,
            hard = pool.pool.hard_pity_four()
        ),
    ];
    if let Some(guarantee) = pool.guarantee {
        parts.push(t!(
            "history-next-five",
            odds = format::next_rate_up(pool.pool, guarantee.five)
        ));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use wish_core::engine::{self, Analysis};
    use wish_core::import::paimon;

    use super::*;

    const SAMPLE: &str = include_str!("../../../../../wish-core/tests/fixtures/paimon-sample.json");

    /// The sample's first account, analysed.
    fn analysis() -> Analysis {
        let data = GameData::embedded();
        let batch = paimon::parse(SAMPLE.as_bytes(), data).unwrap();
        let account = &batch.accounts[0];
        engine::analyze(
            account.wishes.clone(),
            account.server,
            &account.manual_counters,
            data,
        )
    }

    fn event(analysis: &Analysis) -> &PoolAnalysis {
        analysis.pool(Pool::CharacterEvent).unwrap()
    }

    #[test]
    fn newest_first_by_default_and_the_filter_keeps_rarities() {
        let analysis = analysis();
        let pool = event(&analysis);
        let rolls = pool.banner_rolls();
        let all = arrange(&pool.pulls, &rolls, [true; 3], None);
        assert_eq!(all.len(), pool.pulls.len());
        assert_eq!(all.first(), Some(&(pool.pulls.len() - 1)));
        let rare = arrange(&pool.pulls, &rolls, [true, true, false], None);
        assert!(!rare.is_empty() && rare.len() < all.len());
        assert!(
            rare.iter()
                .all(|&ix| pool.pulls[ix].wish.rarity != Rarity::Three)
        );
    }

    #[test]
    fn a_sort_keeps_equal_keys_in_wish_order() {
        let analysis = analysis();
        let pool = event(&analysis);
        let rolls = pool.banner_rolls();
        let by_kind = arrange(
            &pool.pulls,
            &rolls,
            [true; 3],
            Some((Col::Kind, Order::Ascending)),
        );
        let rarities: Vec<Rarity> = by_kind
            .iter()
            .map(|&ix| pool.pulls[ix].wish.rarity)
            .collect();
        assert!(rarities.windows(2).all(|w| w[0] <= w[1]));
        // Among equal kinds, the wishes' order.
        for pair in by_kind.windows(2) {
            let (a, b) = (&pool.pulls[pair[0]].wish, &pool.pulls[pair[1]].wish);
            if (a.rarity, kind(a.item_type)) == (b.rarity, kind(b.item_type)) {
                assert!(pair[0] < pair[1]);
            }
        }
        let down = arrange(
            &pool.pulls,
            &rolls,
            [true; 3],
            Some((Col::Kind, Order::Descending)),
        );
        let mut up = by_kind;
        up.reverse();
        assert_eq!(down, up);
    }

    #[test]
    fn pages_keep_in_range_and_say_which_rows() {
        let paging = Paging::first();
        assert_eq!(paging.pages(0), 1);
        assert_eq!(paging.pages(60), 3);
        assert_eq!(paging.at(9, 60).page, 2);
        assert_eq!(paging.at(1, 60).range(60), 25..50);
        assert_eq!(paging.at(2, 60).range(60), 50..60);
        assert_eq!(paging.at(1, 60).shown(60), "26–50 of 60");
        assert_eq!(paging.shown(0), "No wishes");
        // The first row shown stays on screen: row 50 is on page 1 of 50.
        assert_eq!(paging.at(2, 60).resized(50), Paging { page: 1, size: 50 });
    }

    #[test]
    fn cells_and_subtitle_read_as_the_game_writes() {
        let analysis = analysis();
        let pool = event(&analysis);
        let labels = banner_labels(GameData::embedded(), Pool::CharacterEvent);
        let five = (pool.pulls.iter())
            .find(|pull| pull.wish.rarity == Rarity::Five)
            .unwrap();
        assert_eq!(cell(five, Col::Kind, &labels, None), "5★ Character");
        assert_eq!(
            cell(five, Col::Pity, &labels, None),
            five.pity.unwrap().to_string()
        );
        assert!(!cell(five, Col::Banner, &labels, None).is_empty());
        assert_eq!(cell(five, Col::Roll, &labels, Some(12)), "12");
        assert_eq!(
            cell(five, Col::Time, &labels, None).len(),
            "2024-01-01 10:00".len()
        );
        assert_eq!(Col::of(Pool::Standard).len(), 6);
        assert!(Col::of(Pool::CharacterEvent).contains(&Col::RateUp));
        let subtitle = subtitle(pool);
        assert!(
            subtitle.contains("5★ pity") && subtitle.contains(" / 90"),
            "{subtitle}"
        );
    }
}
