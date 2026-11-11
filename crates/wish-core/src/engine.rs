//! Pity, banner and 50/50 computation from raw wish records.
//!
//! A port of paimon.moe's `processWishes` (`src/routes/wish/import.svelte`),
//! the code behind the numbers it stores and shows. Differences are deliberate
//! and documented where they occur. Everything here is derived on demand and
//! never stored.

use std::ops::Range;

use chrono::{NaiveDateTime, TimeDelta};

use crate::data::{Banner, GameData};
use crate::model::{ItemType, ManualCounters, Pool, Rarity, Server, Wish};
use crate::stats::{MonthStats, Stats, Totals, monthly};

/// paimon.moe assumes Asia when the server is not set.
pub const DEFAULT_SERVER: Server = Server::Asia;

/// Banner times in the game data are UTC+8 wall clock.
const BANNER_DATA_UTC_OFFSET: i32 = 8;

/// Everything derived from one account's wishes.
#[derive(Clone, Debug)]
pub struct Analysis {
    /// Pools that have wishes or manual counters, in [`Pool`] order.
    pub pools: Vec<PoolAnalysis>,
    /// Wishes per calendar month (of the server-local time), all pools together.
    pub monthly: Vec<MonthStats>,
}

impl Analysis {
    pub fn pool(&self, pool: Pool) -> Option<&PoolAnalysis> {
        self.pools.iter().find(|p| p.pool == pool)
    }

    /// Lifetime wishes over all pools (paimon.moe's "total wishes").
    pub fn total_wishes(&self) -> u32 {
        self.pools.iter().map(|p| p.counters.total).sum()
    }

    /// Totals over every banner type: wishes, 5★ and 4★ with their pity
    /// (for averages), and the 5★ rate-up results.
    pub fn totals(&self) -> Totals {
        let mut totals = Totals::default();
        for pool in &self.pools {
            totals.wishes += pool.counters.total;
            totals.five.count += pool.stats.five.count;
            totals.five.pity_sum += pool.stats.five.pity_sum;
            totals.four.count += pool.stats.four.count;
            totals.four.pity_sum += pool.stats.four.pity_sum;
            if let Some(rate) = pool.stats.rate_five {
                totals.five_won += rate.won;
                totals.five_lost += rate.lost;
            }
        }
        totals
    }

    /// Every pull of `rarity` over all banner types, oldest first, with its
    /// banner type.
    pub fn pulls_of(&self, rarity: Rarity) -> Vec<(Pool, &Pull)> {
        let mut pulls: Vec<(Pool, &Pull)> = self
            .pools
            .iter()
            .flat_map(|pool| pool.pulls.iter().map(move |pull| (pool.pool, pull)))
            .filter(|(_, pull)| pull.wish.rarity == rarity)
            .collect();
        pulls.sort_by_key(|(_, pull)| (pull.wish.time, pull.wish.seq));
        pulls
    }
}

#[derive(Clone, Debug)]
pub struct PoolAnalysis {
    pub pool: Pool,
    /// The pool's wishes, oldest first.
    pub pulls: Vec<Pull>,
    /// Banners that received wishes, in order, each with its slice of `pulls`.
    pub banners: Vec<BannerPulls>,
    pub counters: Counters,
    /// `counters` were typed by hand in paimon.moe's manual mode rather than
    /// derived from `pulls`.
    pub manual_counters: bool,
    /// Whether the next 5★ / 4★ is guaranteed to be a rate-up item. `None`
    /// for pools without rate-up items.
    pub guarantee: Option<Guarantee>,
    pub stats: Stats,
    /// Wishes made outside every known banner window. They stay with the
    /// previous banner (or none); many of them mean the banner data is stale.
    pub outside_banners: u32,
    /// 4★/5★ wishes on a rate-up banner whose item is unknown, so their 50/50
    /// could not be decided.
    pub unknown_items: u32,
}

impl PoolAnalysis {
    /// For each pull, its number on its banner, from 1 (paimon.moe's "#Roll"),
    /// or `None` for a pull before any known banner.
    pub fn banner_rolls(&self) -> Vec<Option<u32>> {
        let mut rolls = vec![None; self.pulls.len()];
        for range in &self.banners {
            for (n, ix) in range.pulls.clone().enumerate() {
                if let Some(roll) = rolls.get_mut(ix) {
                    *roll = Some(n as u32 + 1);
                }
            }
        }
        rolls
    }
}

/// One wish with what is derived for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pull {
    pub wish: Wish,
    /// For 4★ and 5★: wishes since the previous item of the same rarity,
    /// this one included. A 5★ does not reset the 4★ count, as in the game.
    pub pity: Option<u32>,
    /// Index into `GameData::banners.for_pool(pool)`.
    pub banner: Option<usize>,
    /// For 4★ and 5★ on character and weapon event banners.
    pub rate_up: Option<RateUp>,
}

/// How a 4★/5★ went against the banner's rate-up items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RateUp {
    /// The previous item of this rarity was not a rate-up item.
    pub guaranteed: bool,
    /// This item is one of the banner's rate-up items.
    pub featured: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateUpOutcome {
    Won,
    Lost,
    Guaranteed,
    /// Guaranteed, yet not a rate-up item. The game never does this; it means
    /// the banner data does not match the wish.
    LostWhileGuaranteed,
}

impl RateUp {
    pub fn outcome(self) -> RateUpOutcome {
        match (self.guaranteed, self.featured) {
            (false, true) => RateUpOutcome::Won,
            (false, false) => RateUpOutcome::Lost,
            (true, true) => RateUpOutcome::Guaranteed,
            (true, false) => RateUpOutcome::LostWhileGuaranteed,
        }
    }

    /// paimon.moe's stored `rate`: guaranteed + featured, so 0 is a lost 50/50,
    /// 1 a won one and 2 a guaranteed rate-up.
    pub fn paimon_rate(self) -> u8 {
        u8::from(self.guaranteed) + u8::from(self.featured)
    }
}

/// The numbers on a pool's counter card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counters {
    pub total: u32,
    /// Wishes since the last 5★.
    pub pity5: u32,
    /// Wishes since the last 4★.
    pub pity4: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Guarantee {
    pub five: bool,
    pub four: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BannerPulls {
    /// Index into `GameData::banners.for_pool(pool)`.
    pub banner: usize,
    pub pulls: Range<usize>,
}

/// Derives everything for one account.
///
/// `server` decides how wish times line up with banner times; `None` means
/// [`DEFAULT_SERVER`], like paimon.moe. Wishes may come in any order.
pub fn analyze(
    mut wishes: Vec<Wish>,
    server: Option<Server>,
    manual_counters: &[ManualCounters],
    data: &GameData,
) -> Analysis {
    wishes.sort_by(|a, b| {
        (a.time, a.seq, a.gacha_type, &a.id).cmp(&(b.time, b.seq, b.gacha_type, &b.id))
    });
    let monthly = monthly(&wishes);
    let server = server.unwrap_or(DEFAULT_SERVER);

    let mut by_pool: Vec<(Pool, Vec<Wish>)> = Pool::ALL.iter().map(|&p| (p, Vec::new())).collect();
    for wish in wishes {
        let pool = wish.pool();
        if let Some((_, list)) = by_pool.iter_mut().find(|(p, _)| *p == pool) {
            list.push(wish);
        }
    }

    let pools = by_pool
        .into_iter()
        .filter_map(|(pool, wishes)| {
            let manual = manual_counters.iter().find(|c| c.pool == pool).copied();
            (!wishes.is_empty() || manual.is_some())
                .then(|| analyze_pool(pool, wishes, server, manual, data))
        })
        .collect();

    Analysis { pools, monthly }
}

fn analyze_pool(
    pool: Pool,
    wishes: Vec<Wish>,
    server: Server,
    manual: Option<ManualCounters>,
    data: &GameData,
) -> PoolAnalysis {
    let banners = data.banners.for_pool(pool);
    let windows: Vec<Window> = banners.iter().map(|b| Window::of(b, server)).collect();
    let has_rate_up = pool.has_rate_up();

    let mut current: Option<usize> = None;
    let mut since5 = 0;
    let mut since4 = 0;
    let mut guaranteed5 = false;
    let mut guaranteed4 = false;
    let mut outside_banners = 0;
    let mut unknown_items = 0;
    let mut pulls = Vec::with_capacity(wishes.len());

    for wish in wishes {
        since5 += 1;
        since4 += 1;
        let time = wish.time.naive();

        // Move to the next banner once the current one has ended. When no
        // banner covers the wish, paimon.moe keeps the current one.
        if current.is_none_or(|i| windows[i].end < time) {
            let from = current.map_or(0, |i| i + 1);
            match (from..windows.len()).find(|&i| windows[i].contains(time)) {
                Some(found) => current = Some(found),
                None => outside_banners += 1,
            }
        }

        let (pity, guaranteed) = match wish.rarity {
            Rarity::Five => (
                Some(std::mem::replace(&mut since5, 0)),
                Some(&mut guaranteed5),
            ),
            Rarity::Four => (
                Some(std::mem::replace(&mut since4, 0)),
                Some(&mut guaranteed4),
            ),
            Rarity::Three => (None, None),
        };
        // Pity typed by hand in manual mode is kept, as paimon.moe does.
        let pity = pity.map(|counted| wish.manual_pity.unwrap_or(counted));

        // paimon.moe would crash on a rate-up banner wish before the first
        // banner; here it simply gets no 50/50.
        let rate_up = match (guaranteed, current) {
            (Some(guaranteed), Some(banner)) if has_rate_up => {
                match featured(&banners[banner], &wish, data) {
                    Some(featured) => {
                        let rate_up = RateUp {
                            guaranteed: *guaranteed,
                            featured,
                        };
                        *guaranteed = !featured;
                        Some(rate_up)
                    }
                    None => {
                        unknown_items += 1;
                        None
                    }
                }
            }
            _ => None,
        };

        pulls.push(Pull {
            wish,
            pity,
            banner: current,
            rate_up,
        });
    }

    let derived = Counters {
        total: u32::try_from(pulls.len()).unwrap_or(u32::MAX),
        pity5: since5,
        pity4: since4,
    };
    let counters = manual.map_or(derived, |m| Counters {
        total: m.total,
        pity5: m.pity5,
        pity4: m.pity4,
    });

    PoolAnalysis {
        pool,
        banners: banner_ranges(&pulls),
        stats: Stats::of(&pulls, counters.total, has_rate_up),
        counters,
        manual_counters: manual.is_some(),
        guarantee: has_rate_up.then_some(Guarantee {
            five: guaranteed5,
            four: guaranteed4,
        }),
        outside_banners,
        unknown_items,
        pulls,
    }
}

/// Whether the wished item is one of the banner's rate-up items, by item key
/// (resolved from the name for wishes that have none). `None` if unknown.
fn featured(banner: &Banner, wish: &Wish, data: &GameData) -> Option<bool> {
    let key = match &wish.item_key {
        Some(key) => key.as_str(),
        None if wish.item_type == ItemType::Unknown => return None,
        None => data.items.by_name(&wish.name)?.key.as_str(),
    };
    let rate_up = match wish.rarity {
        Rarity::Five => &banner.featured,
        Rarity::Four => &banner.featured_rare,
        Rarity::Three => return Some(false),
    };
    Some(rate_up.iter().any(|k| k == key))
}

/// A banner's start and end in the account's server-local wall clock.
#[derive(Clone, Copy, Debug)]
struct Window {
    start: NaiveDateTime,
    end: NaiveDateTime,
}

impl Window {
    /// Times flagged `timezone_dependent` are one global instant (written in
    /// UTC+8); the others are the same wall-clock time on every server.
    fn of(banner: &Banner, server: Server) -> Self {
        let shift = TimeDelta::hours(i64::from(
            BANNER_DATA_UTC_OFFSET - server.utc_offset_hours(),
        ));
        let local =
            |time: NaiveDateTime, dependent: bool| if dependent { time - shift } else { time };
        Self {
            start: local(banner.start.naive(), banner.timezone_dependent),
            end: local(banner.end.naive(), banner.timezone_dependent_end),
        }
    }

    /// paimon.moe's test when looking for a banner. (It keeps the current
    /// banner while `time <= end`, so a wish exactly at `end` stays on it.)
    fn contains(self, time: NaiveDateTime) -> bool {
        self.start <= time && time < self.end
    }
}

fn banner_ranges(pulls: &[Pull]) -> Vec<BannerPulls> {
    let mut ranges: Vec<BannerPulls> = Vec::new();
    for (index, pull) in pulls.iter().enumerate() {
        let Some(banner) = pull.banner else { continue };
        match ranges.last_mut() {
            Some(last) if last.banner == banner && last.pulls.end == index => last.pulls.end += 1,
            _ => ranges.push(BannerPulls {
                banner,
                pulls: index..index + 1,
            }),
        }
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GachaType, Source, WishId, WishTime};

    fn wish(gacha_type: GachaType, time: &str, seq: u32, key: &str) -> Wish {
        let data = GameData::embedded();
        let (name, item_type, rarity, item_key) = match data.items.by_key(key) {
            Some(item) => (
                item.name.clone(),
                item.item_type,
                item.rarity,
                Some(item.key.clone()),
            ),
            None => (key.to_owned(), ItemType::Unknown, Rarity::Three, None),
        };
        let time: WishTime = time.parse().unwrap();
        Wish {
            id: WishId::paimon(gacha_type.pool(), time, seq),
            gacha_type,
            time,
            seq,
            name,
            item_type,
            rarity,
            item_key,
            manual_pity: None,
            source: Source::Paimon,
        }
    }

    fn char_event(time: &str, key: &str) -> Wish {
        wish(GachaType::CharacterEvent, time, 0, key)
    }

    fn run(wishes: Vec<Wish>, server: Server) -> PoolAnalysis {
        let pool = wishes[0].pool();
        analyze(wishes, Some(server), &[], GameData::embedded())
            .pool(pool)
            .cloned()
            .unwrap()
    }

    fn banner_id(pool: &PoolAnalysis, pull: usize) -> Option<String> {
        let banners = GameData::embedded().banners.for_pool(pool.pool);
        pool.pulls[pull].banner.map(|b| banners[b].id())
    }

    #[test]
    fn version_update_banners_start_earlier_on_america() {
        // "In the Name of the Rosula 1" starts 2023-12-20 06:00 UTC+8, which
        // is 2023-12-19 17:00 on America (UTC-5).
        let wishes = || vec![char_event("2023-12-19 18:00:00", "navia")];

        let america = run(wishes(), Server::America);
        assert_eq!(
            banner_id(&america, 0).as_deref(),
            Some("In the Name of the Rosula 1")
        );
        assert_eq!(
            america.pulls[0].rate_up.unwrap().outcome(),
            RateUpOutcome::Won
        );

        // On Asia the wish falls between two banners, before any banner was
        // seen: no banner, no 50/50 (paimon.moe would crash here).
        let asia = run(wishes(), Server::Asia);
        assert_eq!(asia.pulls[0].banner, None);
        assert_eq!(asia.pulls[0].rate_up, None);
        assert_eq!(asia.outside_banners, 1);
    }

    #[test]
    fn a_wish_between_banners_stays_on_the_previous_one() {
        // "Reign of Serenity 4" ends 2024-01-30 14:59; the next banner starts
        // 2024-01-31 06:00 on Asia.
        let pool = run(
            vec![
                char_event("2024-01-20 12:00:00", "slingshot"),
                char_event("2024-01-30 20:00:00", "raiden_shogun"),
                char_event("2024-01-31 07:00:00", "slingshot"),
            ],
            Server::Asia,
        );
        assert_eq!(banner_id(&pool, 1).as_deref(), Some("Reign of Serenity 4"));
        assert_eq!(pool.pulls[1].rate_up.unwrap().outcome(), RateUpOutcome::Won);
        assert_eq!(
            banner_id(&pool, 2).as_deref(),
            Some("The Crane Soars Skyward 1")
        );
        assert_eq!(pool.outside_banners, 1);
        assert_eq!(
            pool.banners
                .iter()
                .map(|b| b.pulls.clone())
                .collect::<Vec<_>>(),
            [0..2, 2..3]
        );
    }

    #[test]
    fn losing_a_50_50_guarantees_the_next_one() {
        let pool = run(
            vec![
                char_event("2024-01-10 12:00:00", "diluc"),
                char_event("2024-01-11 12:00:00", "raiden_shogun"),
                char_event("2024-01-12 12:00:00", "yoimiya"),
            ],
            Server::Asia,
        );
        let outcomes: Vec<_> = pool
            .pulls
            .iter()
            .map(|p| p.rate_up.unwrap().outcome())
            .collect();
        assert_eq!(
            outcomes,
            [
                RateUpOutcome::Lost,
                RateUpOutcome::Guaranteed,
                RateUpOutcome::Won
            ]
        );
        assert_eq!(
            pool.guarantee,
            Some(Guarantee {
                five: false,
                four: false
            })
        );

        let rate = pool.stats.rate_five.unwrap();
        assert_eq!((rate.won, rate.lost), (1, 1));
    }

    #[test]
    fn pity_counts_since_the_last_item_of_the_same_rarity() {
        let wishes = vec![
            char_event("2024-01-10 12:00:00", "slingshot"),
            char_event("2024-01-10 12:01:00", "bennett"),
            char_event("2024-01-10 12:02:00", "slingshot"),
            char_event("2024-01-10 12:03:00", "raiden_shogun"),
            char_event("2024-01-10 12:04:00", "slingshot"),
            char_event("2024-01-10 12:05:00", "kujou_sara"),
            char_event("2024-01-10 12:06:00", "slingshot"),
        ];
        let pool = run(wishes, Server::Asia);
        let pities: Vec<_> = pool.pulls.iter().map(|p| p.pity).collect();
        // The 5★ does not reset the 4★ count: Sara is 4 wishes after Bennett.
        assert_eq!(pities, [None, Some(2), None, Some(4), None, Some(4), None]);
        assert_eq!(
            pool.counters,
            Counters {
                total: 7,
                pity5: 3,
                pity4: 1
            }
        );
    }

    #[test]
    fn character_event_wish_2_shares_pity() {
        let pool = run(
            vec![
                char_event("2024-01-10 12:00:00", "slingshot"),
                wish(
                    GachaType::CharacterEvent2,
                    "2024-01-10 12:01:00",
                    0,
                    "slingshot",
                ),
                wish(
                    GachaType::CharacterEvent2,
                    "2024-01-10 12:02:00",
                    0,
                    "yoimiya",
                ),
            ],
            Server::Asia,
        );
        assert_eq!(pool.pulls[2].pity, Some(3));
    }

    #[test]
    fn standard_wishes_have_no_rate_up() {
        let pool = run(
            vec![wish(GachaType::Standard, "2024-01-10 12:00:00", 0, "diluc")],
            Server::Asia,
        );
        assert_eq!(pool.pulls[0].rate_up, None);
        assert_eq!(pool.guarantee, None);
        assert_eq!(pool.stats.rate_five, None);
        assert_eq!(
            banner_id(&pool, 0).as_deref(),
            Some("Wanderlust Invocation 1")
        );
    }

    #[test]
    fn manual_pity_and_counters_are_kept() {
        let mut diluc = wish(GachaType::Standard, "2024-03-02 20:00:00", 0, "diluc");
        diluc.manual_pity = Some(76);
        let manual = ManualCounters {
            pool: Pool::Standard,
            total: 80,
            pity5: 3,
            pity4: 5,
        };
        let analysis = analyze(vec![diluc], None, &[manual], GameData::embedded());
        let pool = analysis.pool(Pool::Standard).unwrap();
        assert_eq!(pool.pulls[0].pity, Some(76));
        assert!(pool.manual_counters);
        assert_eq!(
            pool.counters,
            Counters {
                total: 80,
                pity5: 3,
                pity4: 5
            }
        );
        assert_eq!(analysis.total_wishes(), 80);
        // Percentages use the counter total, like paimon.moe.
        assert!((pool.stats.five.share(pool.stats.total) - 1.0 / 80.0).abs() < 1e-9);
    }

    #[test]
    fn items_without_a_key_are_matched_by_name() {
        let mut raiden = char_event("2024-01-11 12:00:00", "raiden_shogun");
        raiden.item_key = None;
        let mut unknown = char_event("2024-01-12 12:00:00", "raiden_shogun");
        unknown.item_key = None;
        unknown.name = "Someone New".to_owned();

        let pool = run(vec![raiden, unknown], Server::Asia);
        assert!(pool.pulls[0].rate_up.unwrap().featured);
        assert_eq!(pool.pulls[1].rate_up, None);
        assert_eq!(pool.unknown_items, 1);
    }

    #[test]
    fn input_order_does_not_matter() {
        let wishes = vec![
            char_event("2024-01-10 12:00:00", "diluc"),
            char_event("2024-01-11 12:00:00", "raiden_shogun"),
        ];
        let mut reversed = wishes.clone();
        reversed.reverse();
        assert_eq!(
            run(wishes, Server::Asia).pulls,
            run(reversed, Server::Asia).pulls
        );
    }
}
