//! What to farm today: the domains open on the game's day that give what the
//! plan still lacks once the inventory is shared out.
//!
//! A Domain of Mastery (talent books) or of Forgery (weapon ascension
//! materials) gives one series of its three on each day, two days a week, and
//! all three on Sunday; the data says which days (`Material::days`). The
//! game's day starts at 04:00 in the server's time, as the domains and the
//! daily commissions change then.

use chrono::{DateTime, Datelike as _, FixedOffset, NaiveTime, TimeDelta, TimeZone as _, Weekday};

use super::{Allocation, MaterialId, PlannerData, ResinSource};
use crate::i18n::Text;

/// The hour the game's day starts, in the server's time.
pub const DAY_STARTS_AT: u32 = 4;

/// The game's day at `now`, given in the server's time (or the player's,
/// when the server isn't known).
pub fn game_day(now: DateTime<FixedOffset>) -> Weekday {
    (now - TimeDelta::hours(i64::from(DAY_STARTS_AT))).weekday()
}

/// When the game's day after the one at `now` starts, in `now`'s offset.
pub fn next_game_day(now: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    let today = (now - TimeDelta::hours(i64::from(DAY_STARTS_AT))).date_naive();
    let start = NaiveTime::from_hms_opt(DAY_STARTS_AT, 0, 0).unwrap_or_default();
    let next = today.succ_opt().unwrap_or(today).and_time(start);
    // A fixed offset maps every local time to exactly one instant.
    (now.offset().from_local_datetime(&next).single()).unwrap_or(now + TimeDelta::days(1))
}

/// A series a domain gives that the plan still lacks some tiers of.
#[derive(Clone, Debug, PartialEq)]
pub struct Farm {
    /// Talent books or weapon materials.
    pub source: ResinSource,
    /// The domain that gives it.
    pub domain: Option<Text>,
    /// The days the domain gives it, Sunday included.
    pub days: Vec<Weekday>,
    /// Its tiers the plan still lacks, lowest first, with how many.
    pub lacking: Vec<(MaterialId, u64)>,
    /// The plan's entries that still lack some of it, in the plan's order.
    pub entries: Vec<i64>,
}

impl Farm {
    /// Whether its domain gives it on `day`.
    pub fn open_on(&self, day: Weekday) -> bool {
        self.days.contains(&day)
    }
}

impl PlannerData {
    /// Every series of talent books and weapon materials the plan still
    /// lacks once the inventory is shared out (`allocation`), talent books
    /// first, in the game's inventory order. A placeholder ("?") has no days,
    /// so it isn't one.
    pub fn farms(&self, allocation: &Allocation) -> Vec<Farm> {
        let mut farms: Vec<(u32, Farm)> = (self.craft_families().iter())
            .filter_map(|tiers| {
                let first = self.material(*tiers.first()?)?;
                let source = ResinSource::of(first)?;
                if first.days.is_empty() {
                    return None;
                }
                let lacking: Vec<(MaterialId, u64)> = (tiers.iter())
                    .map(|&id| (id, allocation.missing.materials.get(&id).copied()))
                    .filter_map(|(id, lack)| lack.filter(|&lack| lack > 0).map(|lack| (id, lack)))
                    .collect();
                if lacking.is_empty() {
                    return None;
                }
                let entries = (allocation.entries.iter())
                    .filter(|share| tiers.iter().any(|&id| share.missing_material(id) > 0))
                    .map(|share| share.id)
                    .collect();
                let farm = Farm {
                    source,
                    domain: first.domain.clone(),
                    days: first.days.clone(),
                    lacking,
                    entries,
                };
                Some((first.sort, farm))
            })
            .collect();
        farms.sort_by_key(|(sort, farm)| (farm.source, *sort));
        farms.into_iter().map(|(_, farm)| farm).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{CharacterGoal, Inventory, PlanEntry, PlanTarget};

    fn at(text: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(text).unwrap()
    }

    #[test]
    fn the_game_day_starts_at_four() {
        // Monday 3:59 in the server's time is still Sunday's game day.
        assert_eq!(game_day(at("2026-11-16T03:59:00+01:00")), Weekday::Sun);
        assert_eq!(game_day(at("2026-11-16T04:00:00+01:00")), Weekday::Mon);
        assert_eq!(game_day(at("2026-11-16T23:59:00+01:00")), Weekday::Mon);
        // The same instant is another day on another server.
        assert_eq!(game_day(at("2026-11-16T02:30:00-05:00")), Weekday::Sun);

        assert_eq!(
            next_game_day(at("2026-11-16T03:59:00+01:00")),
            at("2026-11-16T04:00:00+01:00")
        );
        assert_eq!(
            next_game_day(at("2026-11-16T04:00:00+01:00")),
            at("2026-11-17T04:00:00+01:00")
        );
        assert_eq!(
            next_game_day(at("2026-11-16T22:00:00+08:00")),
            at("2026-11-17T04:00:00+08:00")
        );
    }

    fn character(id: i64, key: &str) -> PlanEntry {
        PlanEntry {
            id,
            target: PlanTarget::Character {
                key: key.to_owned(),
                goal: CharacterGoal::FULL,
            },
        }
    }

    #[test]
    fn a_plan_farms_its_books_on_their_days() {
        let planner = PlannerData::embedded();
        // Ganyu and Xiangling both read Diligence (Liyue, Tuesday and Friday).
        let plan = [character(1, "ganyu"), character(2, "xiangling")];
        let allocation = planner.allocate(&plan, &Inventory::default());
        let farms = planner.farms(&allocation);

        let books: Vec<&Farm> = (farms.iter())
            .filter(|farm| farm.source == ResinSource::TalentBooks)
            .collect();
        assert_eq!(books.len(), 1);
        let diligence = books[0];
        assert_eq!(diligence.days, [Weekday::Tue, Weekday::Fri, Weekday::Sun]);
        assert!(diligence.open_on(Weekday::Sun));
        assert!(!diligence.open_on(Weekday::Mon));
        assert_eq!(diligence.entries, [1, 2]);
        // Three tiers, lowest first, each with what the plan lacks.
        assert_eq!(diligence.lacking.len(), 3);
        for (id, lack) in &diligence.lacking {
            assert_eq!(allocation.missing.materials[id], *lack);
        }
        assert!(diligence.domain.is_some());
        // Talent books come before weapon materials, which characters don't
        // need.
        assert!(
            farms
                .iter()
                .all(|farm| farm.source == ResinSource::TalentBooks)
        );
    }

    #[test]
    fn what_the_bag_covers_is_not_farmed() {
        let planner = PlannerData::embedded();
        let plan = [character(1, "ganyu")];
        let lacking = planner.allocate(&plan, &Inventory::default());
        let books = &planner.farms(&lacking)[0];
        // The player has every book the plan needs.
        let mut inventory = Inventory::default();
        for (id, _) in &books.lacking {
            inventory.materials.insert(*id, 1_000);
        }
        let covered = planner.allocate(&plan, &inventory);
        assert!(
            (planner.farms(&covered).iter()).all(|farm| farm.source != ResinSource::TalentBooks)
        );
    }
}
