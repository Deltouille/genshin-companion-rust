//! What the chronicle's pages show, in words and numbers: the periods to
//! pick from, the chosen one's figures, the charts' points. Pure functions
//! over the account's [`Chronicle`], tested without GPUI.

use chrono::{DateTime, Datelike as _, Local, NaiveDate, NaiveDateTime, TimeZone as _};
use wish_core::chronicle::{AbyssRecord, DiaryRecord, Kept, RankedCharacter, TheaterRecord};
use wish_core::collection::hoyolab_character;
use wish_core::model::Rarity;
use wish_core::planner::PlannerData;
use wish_ui::format;

/// The most stars a period of the Abyss gives (floors 9 to 12).
const ABYSS_MAX_STARS: u32 = 36;

/// A character of a team or a ranking, as a tile draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Member {
    /// The planner's key, for its picture; `None` for one the game data
    /// doesn't have yet.
    pub key: Option<String>,
    pub name: String,
    pub rarity: Rarity,
    /// The level it fought at, where HoYoLAB says.
    pub level: Option<u8>,
    /// "trial", "support": not the player's own.
    pub tag: Option<String>,
}

/// A character HoYoLAB names by its game id.
fn member(planner: &PlannerData, id: u32, element: &str, stars: u8, level: Option<u8>) -> Member {
    let character = hoyolab_character(planner, id, element);
    // HoYoLAB gives Aloy 105: a 5★ of her own kind.
    let rarity = Rarity::from_stars(stars % 100)
        .or_else(|| character.and_then(|c| Rarity::from_stars(c.rarity)))
        .unwrap_or(Rarity::Four);
    Member {
        key: character.map(|c| c.key.clone()),
        name: character.map_or_else(|| format!("#{id}"), |c| c.name.as_str().to_owned()),
        rarity,
        level,
        tag: None,
    }
}

/// One point of a chart over the periods kept: its tick, its tooltip's
/// title, its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Point {
    pub tick: String,
    pub title: String,
    pub value: u64,
}

/// A record and its figure: "Strongest single strike", Raiden, 123,456.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Highlight {
    pub title: String,
    pub member: Member,
    pub value: String,
}

/// The local day of `seconds` since the epoch.
fn day(seconds: u64) -> Option<DateTime<Local>> {
    let seconds = i64::try_from(seconds).ok()?;
    Local.timestamp_opt(seconds, 0).single()
}

/// "16 Nov 2025 – 16 Dec 2025".
fn range(start: u64, end: u64) -> String {
    match (day(start), day(end)) {
        (Some(from), Some(to)) => t!(
            "chronicle-range",
            from = format::date(&from),
            to = format::date(&to)
        ),
        _ => String::new(),
    }
}

/// A chart's tick for a period starting at `start`: "2025-11".
fn tick(start: u64) -> String {
    day(start).map_or_else(String::new, |d| format!("{:04}-{:02}", d.year(), d.month()))
}

/// When a record was kept, and whether a later sync brings it up to date.
pub(super) fn kept(kept: &Kept) -> String {
    let date = format::date(&kept.fetched_at.with_timezone(&Local));
    if kept.ended {
        t!("chronicle-kept", date = date)
    } else {
        t!("chronicle-kept-under-way", date = date)
    }
}

// The Spiral Abyss.

/// One floor of an Abyss period.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Floor {
    pub title: String,
    pub stars: u8,
    pub max_stars: u8,
    pub chambers: Vec<Chamber>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Chamber {
    pub title: String,
    pub stars: u8,
    pub max_stars: u8,
    /// When it was cleared, or that it wasn't.
    pub when: String,
    /// Each half's team, the first half first.
    pub halves: Vec<(String, Vec<Member>)>,
}

/// An Abyss period, as its page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AbyssPeriod {
    pub range: String,
    pub stars: u32,
    pub max_stars: u32,
    /// "12-3", and in words.
    pub deepest: String,
    pub deepest_detail: String,
    pub battles: u32,
    pub won: u32,
    pub highlights: Vec<Highlight>,
    /// The deepest floor first.
    pub floors: Vec<Floor>,
    pub kept: String,
}

/// The periods' labels for the picker, newest first.
pub(super) fn abyss_periods(abyss: &[AbyssRecord]) -> Vec<String> {
    (abyss.iter())
        .map(|record| range(record.abyss.start_time, record.abyss.end_time))
        .collect()
}

/// Stars per period, oldest first.
pub(super) fn abyss_trend(abyss: &[AbyssRecord]) -> Vec<Point> {
    (abyss.iter().rev())
        .map(|record| Point {
            tick: tick(record.abyss.start_time),
            title: range(record.abyss.start_time, record.abyss.end_time),
            value: u64::from(record.abyss.total_star),
        })
        .collect()
}

/// A ranking's first place, when it has one.
fn highlight(
    title: String,
    ranking: &[RankedCharacter],
    value: impl Fn(u64) -> String,
    planner: &PlannerData,
) -> Option<Highlight> {
    let first = ranking.first()?;
    Some(Highlight {
        title,
        member: member(planner, first.avatar_id, "", first.rarity, None),
        value: value(first.value),
    })
}

pub(super) fn abyss_period(record: &AbyssRecord, planner: &PlannerData) -> AbyssPeriod {
    let abyss = &record.abyss;
    let count = |n: u64| format::count(n);
    let times = |n: u64| t!("chronicle-times", count = n);
    let highlights = [
        highlight(
            t!("chronicle-most-played"),
            &abyss.reveal_rank,
            times,
            planner,
        ),
        highlight(
            t!("chronicle-strongest-strike"),
            &abyss.damage_rank,
            count,
            planner,
        ),
        highlight(
            t!("chronicle-most-defeats"),
            &abyss.defeat_rank,
            count,
            planner,
        ),
        highlight(
            t!("chronicle-most-damage-taken"),
            &abyss.take_damage_rank,
            count,
            planner,
        ),
        highlight(
            t!("chronicle-most-skills"),
            &abyss.normal_skill_rank,
            count,
            planner,
        ),
        highlight(
            t!("chronicle-most-bursts"),
            &abyss.energy_skill_rank,
            count,
            planner,
        ),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mut deepest_first: Vec<&wish_core::chronicle::AbyssFloor> = abyss.floors.iter().collect();
    deepest_first.sort_by_key(|floor| std::cmp::Reverse(floor.index));
    let floors: Vec<Floor> = (deepest_first.into_iter())
        .map(|floor| Floor {
            title: t!("chronicle-floor", floor = floor.index),
            stars: floor.star,
            max_stars: floor.max_star,
            chambers: (floor.levels.iter())
                .map(|chamber| {
                    let won = chamber.battles.iter().map(|b| b.timestamp).max();
                    Chamber {
                        title: t!("chronicle-chamber", chamber = chamber.index),
                        stars: chamber.star,
                        max_stars: chamber.max_star,
                        when: match won.and_then(day) {
                            Some(when) if chamber.star > 0 => {
                                t!("chronicle-won-on", date = format::date_at_time(&when))
                            }
                            _ => t!("chronicle-not-won"),
                        },
                        halves: (chamber.battles.iter())
                            .map(|battle| {
                                let half = match battle.index {
                                    1 => t!("chronicle-first-half"),
                                    _ => t!("chronicle-second-half"),
                                };
                                let team = (battle.avatars.iter())
                                    .map(|a| {
                                        member(planner, a.id, &a.element, a.rarity, Some(a.level))
                                    })
                                    .collect();
                                (half, team)
                            })
                            .collect(),
                    }
                })
                .collect(),
        })
        .collect();
    let (floor, chamber) = abyss.max_floor.split_once('-').unwrap_or(("", ""));
    AbyssPeriod {
        range: range(abyss.start_time, abyss.end_time),
        stars: abyss.total_star,
        max_stars: ABYSS_MAX_STARS,
        deepest: abyss.max_floor.clone(),
        deepest_detail: t!("chronicle-deepest-detail", floor = floor, chamber = chamber),
        battles: abyss.total_battle_times,
        won: abyss.total_win_times,
        highlights,
        floors,
        kept: kept(&record.kept),
    }
}

// The Imaginarium Theater.

/// One act of a Theater period.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Act {
    pub title: String,
    pub medal: bool,
    pub cast: Vec<Member>,
}

/// A Theater period, as its page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TheaterPeriod {
    pub range: String,
    pub difficulty: String,
    pub furthest: u8,
    pub medals: u32,
    pub flowers: u32,
    pub support: u32,
    /// Minutes on stage, when HoYoLAB says.
    pub minutes: Option<u64>,
    pub highlights: Vec<Highlight>,
    pub acts: Vec<Act>,
    pub kept: String,
}

pub(super) fn theater_periods(theater: &[TheaterRecord]) -> Vec<String> {
    (theater.iter())
        .map(|record| {
            let schedule = &record.theater.schedule;
            range(schedule.start_time, schedule.end_time)
        })
        .collect()
}

/// The furthest act per period, oldest first.
pub(super) fn theater_trend(theater: &[TheaterRecord]) -> Vec<Point> {
    (theater.iter().rev())
        .map(|record| {
            let schedule = &record.theater.schedule;
            Point {
                tick: tick(schedule.start_time),
                title: range(schedule.start_time, schedule.end_time),
                value: u64::from(record.theater.stat.max_round_id),
            }
        })
        .collect()
}

fn difficulty(id: u8) -> String {
    match id {
        1 => t!("chronicle-difficulty-1"),
        2 => t!("chronicle-difficulty-2"),
        3 => t!("chronicle-difficulty-3"),
        4 => t!("chronicle-difficulty-4"),
        5 => t!("chronicle-difficulty-5"),
        id => t!("chronicle-difficulty-unknown", id = id),
    }
}

pub(super) fn theater_period(record: &TheaterRecord, planner: &PlannerData) -> TheaterPeriod {
    let period = &record.theater;
    let detail = period.detail.clone().unwrap_or_default();
    let fight = detail.fight_statisic.clone().unwrap_or_default();
    let count = |n: u64| format::count(n);
    let one = |ranked: &Option<RankedCharacter>| {
        ranked
            .iter()
            .filter(|c| c.avatar_id != 0)
            .cloned()
            .collect::<Vec<_>>()
    };
    let highlights = [
        highlight(
            t!("chronicle-most-damage"),
            &one(&fight.max_damage_avatar),
            count,
            planner,
        ),
        highlight(
            t!("chronicle-most-defeats"),
            &one(&fight.max_defeat_avatar),
            count,
            planner,
        ),
        highlight(
            t!("chronicle-most-damage-taken"),
            &one(&fight.max_take_damage_avatar),
            count,
            planner,
        ),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mut in_order: Vec<&wish_core::chronicle::TheaterAct> = detail.rounds_data.iter().collect();
    in_order.sort_by_key(|act| act.round_id);
    let acts: Vec<Act> = (in_order.into_iter())
        .map(|act| Act {
            title: t!("chronicle-act", act = act.round_id),
            medal: act.is_get_medal,
            cast: (act.avatars.iter())
                .map(|cast| Member {
                    tag: match cast.avatar_type {
                        2 => Some(t!("chronicle-trial")),
                        3 => Some(t!("chronicle-supporting")),
                        _ => None,
                    },
                    ..member(
                        planner,
                        cast.avatar_id,
                        &cast.element,
                        cast.rarity,
                        Some(cast.level),
                    )
                })
                .collect(),
        })
        .collect();
    TheaterPeriod {
        range: range(period.schedule.start_time, period.schedule.end_time),
        difficulty: difficulty(period.stat.difficulty_id),
        furthest: period.stat.max_round_id,
        medals: period.stat.medal_num,
        flowers: period.stat.coin_num,
        support: period.stat.avatar_bonus_num,
        minutes: (fight.total_use_time > 0).then(|| fight.total_use_time.div_ceil(60)),
        highlights,
        acts,
        kept: kept(&record.kept),
    }
}

// The Traveler's Diary.

/// One source of a month's primogems, named in the player's language when
/// the app knows HoYoLAB's English name for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Source {
    pub name: String,
    pub primogems: u64,
}

/// A month of the Diary, as its page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DiaryMonth {
    pub title: String,
    pub primogems: u64,
    pub mora: u64,
    /// The most first.
    pub sources: Vec<Source>,
    /// Primogems per day of the month, the 1st first, every day until the
    /// month's last one or the last one logged.
    pub days: Vec<Point>,
    /// Every reward of the log, newest first: when, what, how many.
    pub log: Vec<(String, String, u64)>,
    pub kept: String,
}

fn month_title(year: i32, month: u32) -> String {
    NaiveDate::from_ymd_opt(year, month, 1).map_or_else(String::new, |d| format::month_year(&d))
}

pub(super) fn diary_months(diary: &[DiaryRecord]) -> Vec<String> {
    (diary.iter())
        .map(|month| month_title(month.year, month.month))
        .collect()
}

/// Primogems per month, oldest first.
pub(super) fn diary_trend(diary: &[DiaryRecord]) -> Vec<Point> {
    (diary.iter().rev())
        .map(|month| Point {
            tick: format!("{:04}-{:02}", month.year, month.month),
            title: month_title(month.year, month.month),
            value: month.totals.month_data.current_primogems,
        })
        .collect()
}

/// A source's name in the player's language: HoYoLAB names them in English.
fn source_name(english: &str) -> String {
    match english {
        "Events" => t!("chronicle-source-events"),
        "Daily Activity" => t!("chronicle-source-daily"),
        "Adventure" => t!("chronicle-source-adventure"),
        "Spiral Abyss" => t!("chronicle-source-abyss"),
        "Quests" => t!("chronicle-source-quests"),
        "Mail" => t!("chronicle-source-mail"),
        "Other" => t!("chronicle-source-other"),
        other => other.to_owned(),
    }
}

pub(super) fn diary_month(record: &DiaryRecord) -> DiaryMonth {
    let totals = &record.totals.month_data;
    let mut sources: Vec<Source> = (totals.group_by.iter())
        .filter(|source| source.num > 0)
        .map(|source| Source {
            name: source_name(&source.action),
            primogems: source.num,
        })
        .collect();
    sources.sort_by_key(|source| std::cmp::Reverse(source.primogems));

    // The log's times are the server's wall clock.
    let entries: Vec<(NaiveDateTime, &str, u64)> = (record.primogems.iter())
        .filter_map(|entry| {
            let time = NaiveDateTime::parse_from_str(&entry.time, "%Y-%m-%d %H:%M:%S").ok()?;
            Some((time, entry.action.as_str(), entry.num))
        })
        .collect();
    let in_month = |d: &NaiveDate| d.year() == record.year && d.month() == record.month;
    let last_day = (entries.iter())
        .map(|(time, ..)| time.date())
        .filter(in_month)
        .max()
        .map_or(0, |d| d.day());
    let days = (1..=last_day)
        .filter_map(|n| NaiveDate::from_ymd_opt(record.year, record.month, n))
        .map(|date| Point {
            tick: date.day().to_string(),
            title: format::date(&date),
            value: (entries.iter())
                .filter(|(time, ..)| time.date() == date)
                .map(|(.., num)| num)
                .sum(),
        })
        .collect();
    let mut log: Vec<(NaiveDateTime, &str, u64)> = entries;
    log.sort_by_key(|(time, ..)| std::cmp::Reverse(*time));
    DiaryMonth {
        title: month_title(record.year, record.month),
        primogems: totals.current_primogems,
        mora: totals.current_mora,
        sources,
        days,
        log: (log.into_iter())
            .map(|(time, action, num)| (format::date_time(&time), action.to_owned(), num))
            .collect(),
        kept: kept(&record.kept),
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;
    use wish_core::chronicle::{Chronicle, Record};
    use wish_core::i18n::{Language, with_language};
    use wish_core::model::Server;

    use super::*;

    fn at(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn chronicle() -> Chronicle {
        let fetched = at("2025-12-20T12:00:00Z");
        let abyss = json!({
            "schedule_id": 102, "start_time": "1763258400", "end_time": "1765850399",
            "total_battle_times": 14, "total_win_times": 12, "max_floor": "12-3", "total_star": 35,
            "reveal_rank": [{ "avatar_id": 10000089, "value": 10, "rarity": 5 }],
            "damage_rank": [{ "avatar_id": 10000052, "value": 123456, "rarity": 5 }],
            "floors": [
                { "index": 11, "star": 9, "max_star": 9, "levels": [] },
                { "index": 12, "star": 8, "max_star": 9, "levels": [
                    { "index": 1, "star": 3, "max_star": 3, "battles": [
                        { "index": 1, "timestamp": "1763300000", "avatars": [
                            { "id": 10000005, "element": "Geo", "level": 90, "rarity": 5 },
                            { "id": 99999999, "level": 80, "rarity": 105 }] },
                        { "index": 2, "timestamp": "1763300400", "avatars": [] }] }] }
            ]
        });
        let theater = json!({
            "schedule": { "schedule_id": 61, "start_time": "1761955200", "end_time": "1764547199" },
            "stat": { "difficulty_id": 4, "max_round_id": 10, "medal_num": 9, "coin_num": 40 },
            "detail": { "rounds_data": [
                { "round_id": 10, "is_get_medal": false, "avatars": [] },
                { "round_id": 2, "is_get_medal": true, "avatars": [
                    { "avatar_id": 10000089, "avatar_type": 3, "level": 90, "rarity": 5 }] }],
                "fight_statisic": { "total_use_time": 1801,
                    "max_damage_avatar": { "avatar_id": 10000089, "value": "999999", "rarity": 5 } } },
            "has_data": true
        });
        let month = json!({ "data_month": 11, "month_data": { "current_primogems": 7440,
            "current_mora": 3102030, "group_by": [
                { "action": "Daily Activity", "num": 1800 }, { "action": "Events", "num": 3000 },
                { "action": "Something New", "num": 5 }, { "action": "Mail", "num": 0 }] } });
        let page = json!({ "data_month": 11, "list": [
            { "action": "Daily Commission Rewards", "time": "2025-11-02 05:00:00", "num": 60 },
            { "action": "Event Rewards", "time": "2025-11-02 20:00:00", "num": 420 },
            { "action": "Daily Commission Rewards", "time": "2025-11-04 05:00:00", "num": 60 }] });
        let records = [
            Record::abyss(abyss, fetched).unwrap().unwrap(),
            Record::theater(json!({ "data": [theater] }), fetched)
                .unwrap()
                .remove(0),
            Record::diary(month, vec![page], 12, Server::Europe, fetched).unwrap(),
        ];
        Chronicle::read(&records)
    }

    #[test]
    fn an_abyss_period_in_figures_floors_and_teams() {
        with_language(Language::English, || {
            let chronicle = chronicle();
            let planner = PlannerData::embedded();
            let period = abyss_period(&chronicle.abyss[0], planner);
            assert_eq!((period.stars, period.max_stars, period.won), (35, 36, 12));
            assert_eq!(period.deepest_detail, "floor 12, chamber 3");
            let titles: Vec<&str> = period.highlights.iter().map(|h| h.title.as_str()).collect();
            assert_eq!(titles, ["Most used", "Strongest single strike"]);
            assert_eq!(period.highlights[0].value, "10 times");
            assert_eq!(period.highlights[1].value, "123,456");
            // The deepest floor first; the Traveler of the element fought.
            assert_eq!(period.floors[0].title, "Floor 12");
            let chamber = &period.floors[0].chambers[0];
            let (half, team) = &chamber.halves[0];
            assert_eq!(half, "First half");
            assert_eq!(team[0].key.as_deref(), Some("travelergeo"));
            assert_eq!((team[1].key.as_ref(), team[1].rarity), (None, Rarity::Five));
            assert!(period.kept.starts_with("Kept from HoYoLAB on"));
            assert_eq!(abyss_trend(&chronicle.abyss)[0].value, 35);
        });
    }

    #[test]
    fn a_theater_period_in_acts_and_highlights() {
        with_language(Language::English, || {
            let chronicle = chronicle();
            let period = theater_period(&chronicle.theater[0], PlannerData::embedded());
            assert_eq!(
                (period.difficulty.as_str(), period.furthest, period.minutes),
                ("Visionary", 10, Some(31))
            );
            let acts: Vec<&str> = period.acts.iter().map(|a| a.title.as_str()).collect();
            assert_eq!(acts, ["Act 2", "Act 10"]);
            assert_eq!(period.acts[0].cast[0].tag.as_deref(), Some("support"));
            assert_eq!(period.highlights[0].value, "999,999");
        });
    }

    #[test]
    fn a_diary_month_by_source_and_by_day() {
        with_language(Language::French, || {
            let chronicle = chronicle();
            let month = diary_month(&chronicle.diary[0]);
            assert_eq!(month.title, "novembre 2025");
            let sources: Vec<(&str, u64)> = (month.sources.iter())
                .map(|s| (s.name.as_str(), s.primogems))
                .collect();
            assert_eq!(
                sources,
                [
                    ("Événements", 3000),
                    ("Activités quotidiennes", 1800),
                    ("Something New", 5)
                ]
            );
            let days: Vec<u64> = month.days.iter().map(|d| d.value).collect();
            assert_eq!(days, [0, 480, 0, 60]);
            assert_eq!(month.log[0].2, 60, "newest first");
            assert_eq!(month.log.len(), 3);
        });
    }
}
