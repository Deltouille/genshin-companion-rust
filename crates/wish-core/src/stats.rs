//! Summary numbers over a run of pulls, as paimon.moe's wish summary shows
//! them (`src/routes/wish/_summary.svelte`).

use std::collections::BTreeMap;

use chrono::Datelike;

use crate::engine::{Pull, RateUp};
use crate::model::{ItemType, Rarity, Wish};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// Denominator of the shares: the counter total for a pool (which manual
    /// mode can set by hand), the number of wishes for a banner.
    pub total: u32,
    pub five: RarityStats,
    pub four: RarityStats,
    pub four_character: RarityStats,
    pub four_weapon: RarityStats,
    /// 50/50 record for 5★, on pools with rate-up items.
    pub rate_five: Option<RateStats>,
    /// Rate-up record for 4★, on pools with rate-up items.
    pub rate_four: Option<RateStats>,
}

impl Stats {
    pub fn of(pulls: &[Pull], total: u32, has_rate_up: bool) -> Self {
        let mut stats = Stats {
            total,
            rate_five: has_rate_up.then(RateStats::default),
            rate_four: has_rate_up.then(RateStats::default),
            ..Stats::default()
        };
        for pull in pulls {
            let pity = pull.pity.unwrap_or(0);
            let rate = match pull.wish.rarity {
                Rarity::Five => {
                    stats.five.add(pity);
                    &mut stats.rate_five
                }
                Rarity::Four => {
                    stats.four.add(pity);
                    match pull.wish.item_type {
                        ItemType::Character => stats.four_character.add(pity),
                        ItemType::Weapon => stats.four_weapon.add(pity),
                        ItemType::Unknown => {}
                    }
                    &mut stats.rate_four
                }
                Rarity::Three => continue,
            };
            if let (Some(rate), Some(rate_up)) = (rate, pull.rate_up) {
                rate.record(rate_up);
            }
        }
        for rate in [&mut stats.rate_five, &mut stats.rate_four]
            .into_iter()
            .flatten()
        {
            rate.finish();
        }
        stats
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RarityStats {
    pub count: u32,
    pub pity_sum: u64,
}

impl RarityStats {
    fn add(&mut self, pity: u32) {
        self.count += 1;
        self.pity_sum += u64::from(pity);
    }

    /// Fraction of `total` wishes (0 when there are none).
    pub fn share(&self, total: u32) -> f64 {
        if total == 0 {
            0.0
        } else {
            f64::from(self.count) / f64::from(total)
        }
    }

    pub fn average_pity(&self) -> Option<f64> {
        (self.count > 0).then(|| self.pity_sum as f64 / f64::from(self.count))
    }
}

/// Wins and losses against rate-up items, counted like paimon.moe: any item
/// that is not a rate-up item is a loss; a rate-up item is a win unless it
/// was guaranteed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RateStats {
    pub won: u32,
    pub lost: u32,
    pub longest_win_streak: u32,
    pub current_win_streak: u32,
}

impl RateStats {
    fn record(&mut self, rate_up: RateUp) {
        if !rate_up.featured {
            self.lost += 1;
            self.longest_win_streak = self.longest_win_streak.max(self.current_win_streak);
            self.current_win_streak = 0;
        } else if !rate_up.guaranteed {
            self.won += 1;
            self.current_win_streak += 1;
        }
    }

    fn finish(&mut self) {
        self.longest_win_streak = self.longest_win_streak.max(self.current_win_streak);
    }

    /// Won / (won + lost); `None` before the first 50/50.
    pub fn win_rate(&self) -> Option<f64> {
        let played = self.won + self.lost;
        (played > 0).then(|| f64::from(self.won) / f64::from(played))
    }
}

/// Totals over every banner type of an account.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub wishes: u32,
    pub five: RarityStats,
    pub four: RarityStats,
    /// 5★ rate-up results on the event banners (see [`RateStats`]).
    pub five_won: u32,
    pub five_lost: u32,
}

impl Totals {
    /// Won / (won + lost) of the 5★ rate-up results; `None` before the first.
    pub fn win_rate(&self) -> Option<f64> {
        let played = self.five_won + self.five_lost;
        (played > 0).then(|| f64::from(self.five_won) / f64::from(played))
    }
}

/// Wishes made in one calendar month (server-local time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonthStats {
    pub year: i32,
    pub month: u32,
    pub total: u32,
    pub five: u32,
    pub four: u32,
}

impl MonthStats {
    /// `YYYY-MM`.
    pub fn label(&self) -> String {
        format!("{:04}-{:02}", self.year, self.month)
    }
}

/// Per-month totals over all pools, oldest month first.
pub fn monthly(wishes: &[Wish]) -> Vec<MonthStats> {
    let mut months: BTreeMap<(i32, u32), MonthStats> = BTreeMap::new();
    for wish in wishes {
        let time = wish.time.naive();
        let month = months
            .entry((time.year(), time.month()))
            .or_insert(MonthStats {
                year: time.year(),
                month: time.month(),
                total: 0,
                five: 0,
                four: 0,
            });
        month.total += 1;
        match wish.rarity {
            Rarity::Five => month.five += 1,
            Rarity::Four => month.four += 1,
            Rarity::Three => {}
        }
    }
    months.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaks_follow_paimon_rules() {
        let mut rate = RateStats::default();
        let won = RateUp {
            guaranteed: false,
            featured: true,
        };
        let lost = RateUp {
            guaranteed: false,
            featured: false,
        };
        let guaranteed = RateUp {
            guaranteed: true,
            featured: true,
        };
        for r in [won, won, lost, guaranteed, won] {
            rate.record(r);
        }
        rate.finish();
        assert_eq!(
            rate,
            RateStats {
                won: 3,
                lost: 1,
                longest_win_streak: 2,
                current_win_streak: 1
            }
        );
        assert_eq!(rate.win_rate(), Some(0.75));
        assert_eq!(RateStats::default().win_rate(), None);
    }

    #[test]
    fn shares_and_averages_handle_empty_counts() {
        let none = RarityStats::default();
        assert_eq!(none.share(0), 0.0);
        assert_eq!(none.average_pity(), None);

        let some = RarityStats {
            count: 2,
            pity_sum: 150,
        };
        assert_eq!(some.share(100), 0.02);
        assert_eq!(some.average_pity(), Some(75.0));
    }
}
