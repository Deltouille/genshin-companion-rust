//! What HoYoLAB keeps of an account's play for a while only: the Spiral
//! Abyss (this period and the one before), the Imaginarium Theater (the same)
//! and the Traveler's Diary (this month and the two before: primogems and
//! Mora, by source, and every primogem earned).
//!
//! Each request returns HoYoLAB's `data` as it came, so a caller can keep it
//! whole; the types below read what an app shows of it. They read leniently:
//! a field HoYoLAB leaves out is its default, and a number may come as a
//! string (HoYoLAB sends times as `"1700000000"`).

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::{Client, LANGUAGE, RECORD_URL, USER_AGENT, role_id};
use crate::transport::{Method, Request};
use crate::{Error, GameAccount, Result};

/// The Traveler's Diary: a month's totals and sources.
const DIARY_URL: &str = "https://sg-hk4e-api.hoyolab.com/event/ysledgeros/month_info";
/// The Traveler's Diary: a month's entries, a page at a time.
const DIARY_LOG_URL: &str = "https://sg-hk4e-api.hoyolab.com/event/ysledgeros/month_detail";

/// How many entries a page of the Diary's log holds at most (genshin.py's).
pub const DIARY_PAGE_SIZE: usize = 100;

/// What the Diary's log lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Currency {
    Primogems,
    Mora,
}

impl Currency {
    fn code(self) -> u8 {
        match self {
            Currency::Primogems => 1,
            Currency::Mora => 2,
        }
    }
}

impl Client<'_> {
    /// The Spiral Abyss of the period under way, or of the one before
    /// (`previous`), as HoYoLAB sends it.
    pub fn spiral_abyss_raw(&self, account: &GameAccount, previous: bool) -> Result<Value> {
        let schedule = if previous { "2" } else { "1" };
        let url = record_url(account, "spiralAbyss", &[("schedule_type", schedule)])?;
        self.signed(Method::Get, &url, None)
    }

    /// The Imaginarium Theater, as HoYoLAB sends it: the periods it keeps
    /// (the one under way and the one before), each with its acts.
    pub fn theater_raw(&self, account: &GameAccount) -> Result<Value> {
        let url = record_url(account, "role_combat", &[("need_detail", "true")])?;
        self.signed(Method::Get, &url, None)
    }

    /// The Traveler's Diary of `month` (1 to 12; `None`, the month under
    /// way): its totals and their sources, and the months HoYoLAB still
    /// keeps (`optional_month`).
    pub fn diary_raw(&self, account: &GameAccount, month: Option<u32>) -> Result<Value> {
        let month = month.map(|m| m.to_string()).unwrap_or_default();
        let mut params = vec![("month", month.as_str())];
        params.retain(|(_, value)| !value.is_empty());
        self.ledger(DIARY_URL, account, &params)
    }

    /// Page `page` (from 1) of the Diary's log of `currency` for `month`:
    /// one entry per reward, newest first. A page shorter than
    /// [`DIARY_PAGE_SIZE`] is the last.
    pub fn diary_log_raw(
        &self,
        account: &GameAccount,
        month: u32,
        currency: Currency,
        page: u32,
    ) -> Result<Value> {
        let (month, kind, page) = (
            month.to_string(),
            currency.code().to_string(),
            page.to_string(),
        );
        let size = DIARY_PAGE_SIZE.to_string();
        self.ledger(
            DIARY_LOG_URL,
            account,
            &[
                ("month", &month),
                ("type", &kind),
                ("current_page", &page),
                ("page_size", &size),
            ],
        )
    }

    /// A request to the Diary: the account in the query, no signature (as
    /// genshin.py asks it).
    fn ledger(&self, base: &str, account: &GameAccount, extra: &[(&str, &str)]) -> Result<Value> {
        let uid = role_id(account)?.to_string();
        let params = [
            ("uid", uid.as_str()),
            ("region", account.region.as_str()),
            ("lang", LANGUAGE),
        ];
        let url = url::Url::parse_with_params(base, params.iter().chain(extra))
            .map_err(|e| Error::Format(e.to_string()))?;
        self.send(Request {
            method: Method::Get,
            url: url.into(),
            headers: vec![("User-Agent", USER_AGENT.to_owned())],
            cookie: self.cookie_header(),
            body: None,
        })
    }
}

/// A Battle Chronicle address with the account in its query.
fn record_url(account: &GameAccount, endpoint: &str, extra: &[(&str, &str)]) -> Result<String> {
    let uid = role_id(account)?.to_string();
    let params = [
        ("role_id", uid.as_str()),
        ("server", account.region.as_str()),
    ];
    let url = url::Url::parse_with_params(
        &format!("{RECORD_URL}/{endpoint}"),
        params.iter().chain(extra),
    )
    .map_err(|e| Error::Format(e.to_string()))?;
    Ok(url.into())
}

/// Reads one of HoYoLAB's answers kept whole as one of the types below.
pub fn read<T: for<'de> Deserialize<'de>>(data: &Value) -> Result<T> {
    T::deserialize(data).map_err(|e| Error::Format(e.to_string()))
}

// The Spiral Abyss.

/// One period of the Spiral Abyss.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SpiralAbyss {
    /// The period's number, the same for every player.
    #[serde(deserialize_with = "number")]
    pub schedule_id: u32,
    /// Seconds since the epoch.
    #[serde(deserialize_with = "number")]
    pub start_time: u64,
    #[serde(deserialize_with = "number")]
    pub end_time: u64,
    #[serde(deserialize_with = "number")]
    pub total_battle_times: u32,
    #[serde(deserialize_with = "number")]
    pub total_win_times: u32,
    /// The deepest chamber reached, `"12-3"`.
    pub max_floor: String,
    #[serde(deserialize_with = "number")]
    pub total_star: u32,
    pub is_unlock: bool,
    /// The characters most played, with how many times.
    #[serde(deserialize_with = "ranked")]
    pub reveal_rank: Vec<RankedCharacter>,
    /// Most opponents defeated.
    #[serde(deserialize_with = "ranked")]
    pub defeat_rank: Vec<RankedCharacter>,
    /// The strongest single strike.
    #[serde(deserialize_with = "ranked")]
    pub damage_rank: Vec<RankedCharacter>,
    /// Most damage taken.
    #[serde(deserialize_with = "ranked")]
    pub take_damage_rank: Vec<RankedCharacter>,
    /// Most Elemental Skills cast.
    #[serde(deserialize_with = "ranked")]
    pub normal_skill_rank: Vec<RankedCharacter>,
    /// Most Elemental Bursts unleashed.
    #[serde(deserialize_with = "ranked")]
    pub energy_skill_rank: Vec<RankedCharacter>,
    pub floors: Vec<AbyssFloor>,
}

/// A character and its figure in one of the Abyss's rankings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RankedCharacter {
    /// The game's id, as the game data has it.
    #[serde(deserialize_with = "number")]
    pub avatar_id: u32,
    #[serde(deserialize_with = "number")]
    pub value: u64,
    #[serde(deserialize_with = "number")]
    pub rarity: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AbyssFloor {
    #[serde(deserialize_with = "number")]
    pub index: u8,
    #[serde(deserialize_with = "number")]
    pub star: u8,
    #[serde(deserialize_with = "number")]
    pub max_star: u8,
    /// Its chambers.
    pub levels: Vec<AbyssChamber>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AbyssChamber {
    #[serde(deserialize_with = "number")]
    pub index: u8,
    #[serde(deserialize_with = "number")]
    pub star: u8,
    #[serde(deserialize_with = "number")]
    pub max_star: u8,
    /// Its two halves, the first one first.
    pub battles: Vec<AbyssBattle>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AbyssBattle {
    /// 1 for the first half, 2 for the second.
    #[serde(deserialize_with = "number")]
    pub index: u8,
    /// When it was won, in seconds since the epoch.
    #[serde(deserialize_with = "number")]
    pub timestamp: u64,
    pub avatars: Vec<TeamMember>,
}

/// A character of a team, at the level it fought at.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TeamMember {
    #[serde(deserialize_with = "number")]
    pub id: u32,
    /// In English; it tells the Traveler's forms apart.
    pub element: String,
    #[serde(deserialize_with = "number")]
    pub level: u8,
    #[serde(deserialize_with = "number")]
    pub rarity: u8,
}

// The Imaginarium Theater.

/// The Imaginarium Theater as HoYoLAB keeps it: the periods it holds.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theater {
    #[serde(rename = "data")]
    pub periods: Vec<TheaterPeriod>,
    pub is_unlock: bool,
}

/// One period of the Imaginarium Theater.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TheaterPeriod {
    pub schedule: TheaterSchedule,
    pub stat: TheaterStat,
    pub detail: Option<TheaterDetail>,
    /// Whether the player took part.
    pub has_data: bool,
    pub has_detail_data: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TheaterSchedule {
    #[serde(deserialize_with = "number")]
    pub schedule_id: u32,
    #[serde(deserialize_with = "number")]
    pub start_time: u64,
    #[serde(deserialize_with = "number")]
    pub end_time: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TheaterStat {
    /// 1 Easy, 2 Normal, 3 Hard, 4 Visionary, 5 the Lunar Arcana's.
    #[serde(deserialize_with = "number")]
    pub difficulty_id: u8,
    /// The furthest act reached.
    #[serde(deserialize_with = "number")]
    pub max_round_id: u8,
    /// Whether each act's medal (Star Challenge Stella) was earned.
    #[serde(deserialize_with = "flags")]
    pub get_medal_round_list: Vec<bool>,
    #[serde(deserialize_with = "number")]
    pub medal_num: u32,
    /// Fantasia Flowers used.
    #[serde(deserialize_with = "number")]
    pub coin_num: u32,
    /// Audience support received.
    #[serde(deserialize_with = "number")]
    pub avatar_bonus_num: u32,
    /// How many times the player's supporting cast helped others.
    #[serde(deserialize_with = "number")]
    pub rent_cnt: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TheaterDetail {
    pub rounds_data: Vec<TheaterAct>,
    pub fight_statisic: Option<TheaterFight>,
}

/// One act of a Theater period.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TheaterAct {
    #[serde(deserialize_with = "number")]
    pub round_id: u8,
    pub is_get_medal: bool,
    /// Seconds since the epoch.
    #[serde(deserialize_with = "number")]
    pub finish_time: u64,
    pub avatars: Vec<TheaterCast>,
}

/// A character of an act's cast.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TheaterCast {
    #[serde(deserialize_with = "number")]
    pub avatar_id: u32,
    /// In English; it tells the Traveler's forms apart.
    pub element: String,
    /// 1 the player's own, 2 a trial character, 3 a friend's supporting cast.
    #[serde(deserialize_with = "number")]
    pub avatar_type: u8,
    #[serde(deserialize_with = "number")]
    pub level: u8,
    #[serde(deserialize_with = "number")]
    pub rarity: u8,
}

/// A period's highlights.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TheaterFight {
    pub max_defeat_avatar: Option<RankedCharacter>,
    pub max_damage_avatar: Option<RankedCharacter>,
    pub max_take_damage_avatar: Option<RankedCharacter>,
    /// Seconds spent on the whole period.
    #[serde(deserialize_with = "number")]
    pub total_use_time: u64,
}

// The Traveler's Diary.

/// A month of the Traveler's Diary.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiaryMonth {
    /// The month, 1 to 12, of the server's calendar.
    #[serde(deserialize_with = "number")]
    pub data_month: u32,
    /// The months HoYoLAB still keeps, the month under way last.
    pub optional_month: Vec<u32>,
    pub month_data: DiaryTotals,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiaryTotals {
    #[serde(deserialize_with = "number")]
    pub current_primogems: u64,
    #[serde(deserialize_with = "number")]
    pub current_mora: u64,
    /// Where the month's primogems came from.
    pub group_by: Vec<DiarySource>,
}

/// One source of a month's primogems.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiarySource {
    #[serde(deserialize_with = "number")]
    pub action_id: u32,
    /// Its name, in English.
    pub action: String,
    #[serde(deserialize_with = "number")]
    pub num: u64,
    #[serde(deserialize_with = "number")]
    pub percent: u32,
}

/// A page of the Diary's log.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiaryLogPage {
    #[serde(deserialize_with = "number")]
    pub data_month: u32,
    pub list: Vec<DiaryEntry>,
}

/// One reward of the Diary's log.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiaryEntry {
    #[serde(deserialize_with = "number")]
    pub action_id: u32,
    /// What gave it, in English.
    pub action: String,
    /// The server's wall clock, `2026-11-01 04:12:33`.
    pub time: String,
    #[serde(deserialize_with = "number")]
    pub num: u64,
}

/// A number as HoYoLAB sends it: a number, a string of digits, or nothing
/// (the default).
fn number<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: TryFrom<u64> + Default,
{
    let value = Value::deserialize(deserializer)?;
    let n = match &value {
        Value::Null => return Ok(T::default()),
        Value::Number(n) => n.as_u64(),
        Value::String(s) if s.trim().is_empty() => return Ok(T::default()),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    };
    n.and_then(|n| T::try_from(n).ok())
        .ok_or_else(|| de::Error::custom(format!("{value} isn't a number in range")))
}

/// A list of yes and no, as booleans or as 0 and 1.
fn flags<'de, D>(deserializer: D) -> std::result::Result<Vec<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let values = Option::<Vec<Value>>::deserialize(deserializer)?.unwrap_or_default();
    values
        .iter()
        .map(|value| match value {
            Value::Bool(b) => Ok(*b),
            Value::Number(n) => Ok(n.as_u64().is_some_and(|n| n != 0)),
            _ => Err(de::Error::custom(format!("{value} isn't a yes or a no"))),
        })
        .collect()
}

/// A ranking, without the empty places HoYoLAB pads it with (id 0).
fn ranked<'de, D>(deserializer: D) -> std::result::Result<Vec<RankedCharacter>, D::Error>
where
    D: Deserializer<'de>,
{
    let list = Option::<Vec<RankedCharacter>>::deserialize(deserializer)?.unwrap_or_default();
    Ok(list.into_iter().filter(|c| c.avatar_id != 0).collect())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn an_abyss_period_reads_with_its_times_as_strings() {
        let data = json!({
            "schedule_id": 102, "start_time": "1763258400", "end_time": "1765850399",
            "total_battle_times": 14, "total_win_times": 12, "max_floor": "12-3",
            "total_star": 36, "is_unlock": true,
            "reveal_rank": [{ "avatar_id": 10000089, "avatar_icon": "…", "value": 10, "rarity": 5 }],
            "damage_rank": [{ "avatar_id": 0, "value": 0, "rarity": 0 }],
            "floors": [{ "index": 12, "icon": "", "is_unlock": true, "settle_time": "0",
                "star": 9, "max_star": 9,
                "levels": [{ "index": 1, "star": 3, "max_star": 3, "battles": [
                    { "index": 1, "timestamp": "1763300000",
                      "avatars": [{ "id": 10000089, "icon": "…", "level": 90, "rarity": 5 }] }
                ]}]
            }],
            "something_new": { "HoYoLAB": "adds fields" }
        });
        let abyss: SpiralAbyss = read(&data).unwrap();
        assert_eq!(
            (abyss.schedule_id, abyss.start_time, abyss.total_star),
            (102, 1_763_258_400, 36)
        );
        assert_eq!(abyss.reveal_rank[0].value, 10);
        assert!(
            abyss.damage_rank.is_empty(),
            "the empty places are left out"
        );
        assert!(abyss.defeat_rank.is_empty(), "a ranking left out is empty");
        let battle = &abyss.floors[0].levels[0].battles[0];
        assert_eq!(
            (battle.timestamp, battle.avatars[0].level),
            (1_763_300_000, 90)
        );
    }

    #[test]
    fn a_theater_reads_with_its_medals_as_numbers() {
        let data = json!({ "is_unlock": true, "data": [{
            "schedule": { "schedule_id": 61, "start_time": "1761955200", "end_time": "1764547199",
                          "start_date_time": { "year": 2025, "month": 11, "day": 1 } },
            "stat": { "difficulty_id": 4, "max_round_id": 10, "get_medal_round_list": [1, 1, 0],
                      "medal_num": 2, "coin_num": 40, "avatar_bonus_num": 9, "rent_cnt": 3 },
            "detail": { "rounds_data": [{ "round_id": 1, "is_get_medal": true, "finish_time": "1761990000",
                "avatars": [{ "avatar_id": 10000089, "avatar_type": 1, "level": 90, "rarity": 5 }] }],
                "fight_statisic": { "max_damage_avatar": { "avatar_id": 10000089, "value": "123456", "rarity": 5 },
                                    "max_defeat_avatar": { "avatar_id": 0, "value": "", "rarity": 0 },
                                    "total_use_time": 1800 } },
            "has_data": true, "has_detail_data": true
        }]});
        let theater: Theater = read(&data).unwrap();
        let period = &theater.periods[0];
        assert_eq!(period.stat.get_medal_round_list, [true, true, false]);
        let fight = period
            .detail
            .as_ref()
            .unwrap()
            .fight_statisic
            .as_ref()
            .unwrap();
        assert_eq!(fight.max_damage_avatar.as_ref().unwrap().value, 123_456);
        assert_eq!(fight.max_defeat_avatar.as_ref().unwrap().avatar_id, 0);
    }

    #[test]
    fn a_diary_month_and_a_page_of_its_log() {
        let month: DiaryMonth = read(&json!({
            "uid": 712345678, "region": "os_euro", "data_month": 11, "optional_month": [9, 10, 11],
            "month_data": { "current_primogems": 7440, "current_mora": 3102030, "last_primogems": 6000,
                "group_by": [{ "action_id": 1, "action": "Daily Activity", "num": 1800, "percent": 24 }] },
            "day_data": { "current_primogems": 60 }
        }))
        .unwrap();
        assert_eq!(
            (month.data_month, month.month_data.current_primogems),
            (11, 7440)
        );
        assert_eq!(month.month_data.group_by[0].action, "Daily Activity");

        let page: DiaryLogPage = read(&json!({ "data_month": 11, "current_page": 1, "list": [
            { "action_id": 1, "action": "Daily Commission Rewards", "time": "2025-11-24 05:01:02", "num": 60 }
        ]}))
        .unwrap();
        assert_eq!(page.list[0].num, 60);

        let error = read::<SpiralAbyss>(&json!({ "total_star": "lots" })).unwrap_err();
        assert!(matches!(error, Error::Format(_)));
    }
}
