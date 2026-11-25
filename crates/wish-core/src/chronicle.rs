//! The chronicle: what HoYoLAB keeps of an account's play for a while only,
//! kept here for good. HoYoLAB shows the Spiral Abyss and the Imaginarium
//! Theater of the period under way and the one before, and the Traveler's
//! Diary of this month and the two before; then they are gone.
//!
//! Each period is a [`Record`]: HoYoLAB's answer as it came, whole, so
//! nothing it sent is lost even when the app reads only part of it, and a
//! later version of the app can show more of what is already stored. What the
//! pages show is read from it ([`Chronicle::read`]), never stored.
//!
//! **A period fetched once it was over never changes** ([`merge`]): a later
//! fetch (from HoYoLAB or a snapshot) leaves it as it is. One fetched while
//! under way is replaced by a later fetch, which shows more of it.

use std::cmp::Reverse;

use chrono::{DateTime, Datelike as _, Duration, FixedOffset, NaiveDate, TimeZone as _, Utc};
use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::model::{Server, text_enum};

pub use hoyolab::chronicle::{
    AbyssBattle, AbyssChamber, AbyssFloor, DiaryEntry, DiaryLogPage, DiaryMonth, DiarySource,
    RankedCharacter, SpiralAbyss, TeamMember, TheaterAct, TheaterCast, TheaterFight, TheaterPeriod,
};

text_enum! {
    /// What a record holds.
    pub enum RecordKind {
        /// A period of the Spiral Abyss.
        Abyss => "abyss",
        /// A period of the Imaginarium Theater.
        Theater => "theater",
        /// A month of the Traveler's Diary.
        Diary => "diary",
    }
}

/// How long after a month ends its Diary is taken as complete: HoYoLAB adds
/// a reward to the Diary a while after it is earned.
const DIARY_GRACE: Duration = Duration::days(1);

/// One period HoYoLAB showed, kept as it came.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub kind: RecordKind,
    /// The period: the game's number for it (Abyss, Theater: the same for
    /// every player), or the Diary's month as `YYYYMM`.
    pub period: u32,
    pub fetched_at: DateTime<Utc>,
    /// Whether the period was over when it was fetched: then it never
    /// changes.
    pub ended: bool,
    /// HoYoLAB's answer: the Abyss's or a Theater period's `data`; for the
    /// Diary, `{"month": …, "primogems": [pages…]}`, its totals and every
    /// page of its primogem log.
    pub data: Value,
}

impl Record {
    /// The Abyss period HoYoLAB sent (`data`), fetched at `fetched_at`;
    /// `None` when the player didn't fight in it: there is nothing to keep.
    pub fn abyss(data: Value, fetched_at: DateTime<Utc>) -> Result<Option<Record>> {
        let abyss: SpiralAbyss = hoyolab::chronicle::read(&data)?;
        if abyss.total_battle_times == 0 || abyss.schedule_id == 0 {
            return Ok(None);
        }
        Ok(Some(Record {
            kind: RecordKind::Abyss,
            period: abyss.schedule_id,
            fetched_at,
            ended: ended_at(abyss.end_time, fetched_at),
            data,
        }))
    }

    /// Each Theater period of HoYoLAB's answer (`data`) the player took part
    /// in, each as its own record.
    pub fn theater(data: Value, fetched_at: DateTime<Utc>) -> Result<Vec<Record>> {
        let Value::Object(mut answer) = data else {
            return Err(
                hoyolab::Error::Format("the Theater's answer isn't an object".into()).into(),
            );
        };
        let periods = match answer.remove("data") {
            Some(Value::Array(periods)) => periods,
            None | Some(Value::Null) => Vec::new(),
            Some(_) => {
                return Err(
                    hoyolab::Error::Format("the Theater's periods aren't a list".into()).into(),
                );
            }
        };
        let mut records = Vec::new();
        for data in periods {
            let period: TheaterPeriod = hoyolab::chronicle::read(&data)?;
            if !period.has_data || period.schedule.schedule_id == 0 {
                continue;
            }
            records.push(Record {
                kind: RecordKind::Theater,
                period: period.schedule.schedule_id,
                fetched_at,
                ended: ended_at(period.schedule.end_time, fetched_at),
                data,
            });
        }
        Ok(records)
    }

    /// A Diary month: its totals (`month`, HoYoLAB's `month_info`) and every
    /// page of its primogem log, fetched at `fetched_at` from an account on
    /// `server`. HoYoLAB names the month without its year: it is the latest
    /// one of that number up to the server's month under way, `current`.
    pub fn diary(
        month: Value,
        pages: Vec<Value>,
        current: u32,
        server: Server,
        fetched_at: DateTime<Utc>,
    ) -> Result<Record> {
        let read: DiaryMonth = hoyolab::chronicle::read(&month)?;
        let (year, number) = diary_month(read.data_month, current, server, fetched_at)?;
        Ok(Record {
            kind: RecordKind::Diary,
            period: diary_period(year, number),
            fetched_at,
            ended: diary_ended(year, number, server, fetched_at),
            data: json!({ "month": month, "primogems": pages }),
        })
    }
}

/// Whether a period ending at `end` (seconds since the epoch) was over at
/// `fetched_at`.
fn ended_at(end: u64, fetched_at: DateTime<Utc>) -> bool {
    end > 0 && u64::try_from(fetched_at.timestamp()).is_ok_and(|now| now > end)
}

/// The server's wall clock at `at`.
fn server_date(server: Server, at: DateTime<Utc>) -> NaiveDate {
    let offset = FixedOffset::east_opt(server.utc_offset_hours() * 3600)
        .unwrap_or_else(|| FixedOffset::east_opt(0).expect("UTC is an offset"));
    at.with_timezone(&offset).date_naive()
}

/// The year and month of the Diary's month `month` (1 to 12), seen at
/// `fetched_at` when the server's month under way was `current`: the latest
/// month of that number up to `current`.
fn diary_month(
    month: u32,
    current: u32,
    server: Server,
    fetched_at: DateTime<Utc>,
) -> Result<(i32, u32)> {
    if !(1..=12).contains(&month) || !(1..=12).contains(&current) {
        return Err(hoyolab::Error::Format(format!("{month} isn't a month")).into());
    }
    let today = server_date(server, fetched_at);
    // The year of `current`: this one, unless the computer's clock and
    // HoYoLAB's disagree across a new year.
    let current_year = match (current, today.month()) {
        (12, 1) => today.year() - 1,
        (1, 12) => today.year() + 1,
        _ => today.year(),
    };
    let year = if month > current {
        current_year - 1
    } else {
        current_year
    };
    Ok((year, month))
}

/// A Diary month's period: `YYYYMM`.
fn diary_period(year: i32, month: u32) -> u32 {
    u32::try_from(year).unwrap_or(0) * 100 + month
}

/// The year and month of a Diary period.
pub fn diary_year_month(period: u32) -> (i32, u32) {
    (i32::try_from(period / 100).unwrap_or(0), period % 100)
}

/// Whether the Diary of `month` in `year` was complete at `fetched_at`: the
/// month was over on the server for [`DIARY_GRACE`].
fn diary_ended(year: i32, month: u32, server: Server, fetched_at: DateTime<Utc>) -> bool {
    let next = match month {
        12 => NaiveDate::from_ymd_opt(year + 1, 1, 1),
        _ => NaiveDate::from_ymd_opt(year, month + 1, 1),
    };
    let Some(next) = next.and_then(|d| d.and_hms_opt(0, 0, 0)) else {
        return false;
    };
    let offset = FixedOffset::east_opt(server.utc_offset_hours() * 3600);
    let Some(end) = offset.and_then(|o| o.from_local_datetime(&next).single()) else {
        return false;
    };
    fetched_at >= end.with_timezone(&Utc) + DIARY_GRACE
}

/// What becomes of a record fetched again, against the one stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Merge {
    /// Nothing stored for this period yet.
    Add,
    /// The stored one was fetched while the period was under way, and this
    /// one is newer and says something else.
    Replace,
    /// The stored one stays: the period was over when it was fetched, or
    /// this one is older, or the same.
    Keep,
}

/// What a record fetched again does to `stored`, the one kept for its
/// period: a period fetched once over never changes; one fetched while
/// under way gives way to a newer fetch.
pub fn merge(stored: Option<&Record>, incoming: &Record) -> Merge {
    let Some(stored) = stored else {
        return Merge::Add;
    };
    let newer = incoming.fetched_at > stored.fetched_at;
    let says_more = incoming.data != stored.data || incoming.ended != stored.ended;
    if stored.ended || !newer || !says_more {
        Merge::Keep
    } else {
        Merge::Replace
    }
}

/// How many periods of one kind a sync adds, updates and leaves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecordCounts {
    pub added: usize,
    pub updated: usize,
    pub kept: usize,
}

impl RecordCounts {
    pub fn changes(&self) -> usize {
        self.added + self.updated
    }

    fn count(&mut self, merge: Merge) {
        match merge {
            Merge::Add => self.added += 1,
            Merge::Replace => self.updated += 1,
            Merge::Keep => self.kept += 1,
        }
    }
}

/// What a sync does to the chronicle, by kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecordsPlan {
    pub abyss: RecordCounts,
    pub theater: RecordCounts,
    pub diary: RecordCounts,
}

impl RecordsPlan {
    /// What `incoming` does to `stored` (an account's records).
    pub fn new(incoming: &[Record], stored: &[Record]) -> Self {
        let mut plan = Self::default();
        for record in incoming {
            let kept = stored
                .iter()
                .find(|s| s.kind == record.kind && s.period == record.period);
            plan.count(record.kind, merge(kept, record));
        }
        plan
    }

    pub fn count(&mut self, kind: RecordKind, merge: Merge) {
        match kind {
            RecordKind::Abyss => self.abyss.count(merge),
            RecordKind::Theater => self.theater.count(merge),
            RecordKind::Diary => self.diary.count(merge),
        }
    }

    pub fn changes(&self) -> usize {
        self.abyss.changes() + self.theater.changes() + self.diary.changes()
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A record's when and whether: what every period's page says of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kept {
    pub fetched_at: DateTime<Utc>,
    pub ended: bool,
}

/// A period of the Spiral Abyss, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbyssRecord {
    pub kept: Kept,
    pub abyss: SpiralAbyss,
}

/// A period of the Imaginarium Theater, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TheaterRecord {
    pub kept: Kept,
    pub theater: TheaterPeriod,
}

/// A month of the Traveler's Diary, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiaryRecord {
    pub kept: Kept,
    pub year: i32,
    /// 1 to 12.
    pub month: u32,
    pub totals: DiaryMonth,
    /// Every primogem the log lists, newest first.
    pub primogems: Vec<DiaryEntry>,
}

/// An account's chronicle, read: each kind newest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chronicle {
    pub abyss: Vec<AbyssRecord>,
    pub theater: Vec<TheaterRecord>,
    pub diary: Vec<DiaryRecord>,
    /// Records this version can't read (stored by a newer one, say). They
    /// stay stored.
    pub unreadable: usize,
}

impl Chronicle {
    /// Reads `records`, an account's. One that doesn't read is counted in
    /// `unreadable`, never dropped from the store.
    pub fn read(records: &[Record]) -> Self {
        let mut chronicle = Self::default();
        for record in records {
            if chronicle.add(record).is_err() {
                chronicle.unreadable += 1;
            }
        }
        (chronicle.abyss).sort_by_key(|a| Reverse(a.abyss.schedule_id));
        (chronicle.theater).sort_by_key(|t| Reverse(t.theater.schedule.schedule_id));
        (chronicle.diary).sort_by_key(|d| Reverse((d.year, d.month)));
        chronicle
    }

    pub fn is_empty(&self) -> bool {
        self.abyss.is_empty() && self.theater.is_empty() && self.diary.is_empty()
    }

    fn add(&mut self, record: &Record) -> Result<()> {
        let kept = Kept {
            fetched_at: record.fetched_at,
            ended: record.ended,
        };
        match record.kind {
            RecordKind::Abyss => self.abyss.push(AbyssRecord {
                kept,
                abyss: hoyolab::chronicle::read(&record.data)?,
            }),
            RecordKind::Theater => self.theater.push(TheaterRecord {
                kept,
                theater: hoyolab::chronicle::read(&record.data)?,
            }),
            RecordKind::Diary => {
                let (year, month) = diary_year_month(record.period);
                let totals = hoyolab::chronicle::read(&record.data["month"])?;
                let pages: Vec<DiaryLogPage> = match &record.data["primogems"] {
                    Value::Array(pages) => (pages.iter())
                        .map(hoyolab::chronicle::read)
                        .collect::<hoyolab::Result<_>>()?,
                    _ => return Err(Error::from(hoyolab::Error::Format("no log".into()))),
                };
                self.diary.push(DiaryRecord {
                    kept,
                    year,
                    month,
                    totals,
                    primogems: pages.into_iter().flat_map(|page| page.list).collect(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn abyss(id: u32, battles: u32, end: u64) -> Value {
        json!({ "schedule_id": id, "start_time": "1", "end_time": end.to_string(),
                "total_battle_times": battles, "total_star": 30, "max_floor": "12-1" })
    }

    #[test]
    fn an_abyss_period_is_kept_once_fought_and_ends_with_its_period() {
        let end = 1_765_850_399; // 2025-12-16 01:59:59 UTC
        let before = at("2025-12-10T00:00:00Z");
        let after = at("2025-12-17T00:00:00Z");
        let record = Record::abyss(abyss(102, 14, end), before).unwrap().unwrap();
        assert_eq!(
            (record.kind, record.period, record.ended),
            (RecordKind::Abyss, 102, false)
        );
        assert!(
            Record::abyss(abyss(102, 14, end), after)
                .unwrap()
                .unwrap()
                .ended
        );
        assert_eq!(
            Record::abyss(abyss(103, 0, end), after).unwrap(),
            None,
            "not fought"
        );
    }

    #[test]
    fn each_theater_period_taken_part_in_is_a_record() {
        let data = json!({ "is_unlock": true, "data": [
            { "schedule": { "schedule_id": 61, "end_time": "1764547199" }, "has_data": true },
            { "schedule": { "schedule_id": 60, "end_time": "1761955199" }, "has_data": false },
        ]});
        let records = Record::theater(data, at("2025-11-20T00:00:00Z")).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!((records[0].period, records[0].ended), (61, false));
        assert_eq!(records[0].data["schedule"]["schedule_id"], 61);
    }

    #[test]
    fn a_diary_month_takes_its_year_from_the_month_under_way() {
        let month = |n: u32| json!({ "data_month": n, "month_data": { "current_primogems": 100 } });
        let europe = Server::Europe;
        // In January, November and December are last year's.
        let january = at("2026-01-10T12:00:00Z");
        let november = Record::diary(month(11), Vec::new(), 1, europe, january).unwrap();
        assert_eq!((november.period, november.ended), (202511, true));
        let this = Record::diary(month(1), Vec::new(), 1, europe, january).unwrap();
        assert_eq!((this.period, this.ended), (202601, false));
        // Just past midnight on the 1st, the month before isn't complete yet.
        let new_year = at("2026-01-01T00:30:00Z");
        let december = Record::diary(month(12), Vec::new(), 1, europe, new_year).unwrap();
        assert_eq!((december.period, december.ended), (202512, false));
        assert!(Record::diary(month(13), Vec::new(), 1, europe, january).is_err());
    }

    #[test]
    fn a_period_fetched_once_over_never_changes() {
        let under_way = at("2025-12-10T00:00:00Z");
        let later = at("2025-12-12T00:00:00Z");
        let over = at("2025-12-17T00:00:00Z");
        let end = 1_765_850_399;
        let first = Record::abyss(abyss(102, 10, end), under_way)
            .unwrap()
            .unwrap();
        let more = Record::abyss(abyss(102, 14, end), later).unwrap().unwrap();
        let last = Record::abyss(abyss(102, 15, end), over).unwrap().unwrap();
        assert_eq!(merge(None, &first), Merge::Add);
        assert_eq!(merge(Some(&first), &more), Merge::Replace);
        assert_eq!(merge(Some(&more), &first), Merge::Keep, "an older fetch");
        assert_eq!(merge(Some(&more), &more.clone()), Merge::Keep, "the same");
        assert_eq!(merge(Some(&more), &last), Merge::Replace);
        let mut again = last.clone();
        again.fetched_at = at("2026-01-01T00:00:00Z");
        again.data["total_star"] = json!(0);
        assert_eq!(merge(Some(&last), &again), Merge::Keep, "over: final");

        let plan = RecordsPlan::new(&[last.clone(), first.clone()], std::slice::from_ref(&more));
        assert_eq!(
            plan.abyss,
            RecordCounts {
                added: 0,
                updated: 1,
                kept: 1
            }
        );
        assert_eq!(plan.changes(), 1);
    }

    #[test]
    fn the_chronicle_reads_each_kind_newest_first() {
        let fetched = at("2025-12-20T00:00:00Z");
        let page = json!({ "data_month": 11, "list": [
            { "action": "Daily Commission Rewards", "time": "2025-11-24 05:01:02", "num": 60 }] });
        let records = vec![
            Record::abyss(abyss(101, 3, 1), fetched).unwrap().unwrap(),
            Record::abyss(abyss(102, 3, 1), fetched).unwrap().unwrap(),
            Record::diary(
                json!({ "data_month": 11 }),
                vec![page],
                12,
                Server::Asia,
                fetched,
            )
            .unwrap(),
            Record {
                kind: RecordKind::Theater,
                period: 60,
                fetched_at: fetched,
                ended: true,
                data: json!({ "stat": { "max_round_id": "ten" } }),
            },
        ];
        let chronicle = Chronicle::read(&records);
        let ids: Vec<u32> = chronicle
            .abyss
            .iter()
            .map(|a| a.abyss.schedule_id)
            .collect();
        assert_eq!(ids, [102, 101]);
        let diary = &chronicle.diary[0];
        assert_eq!(
            (diary.year, diary.month, diary.primogems[0].num),
            (2025, 11, 60)
        );
        assert_eq!((chronicle.theater.len(), chronicle.unreadable), (0, 1));
    }
}
