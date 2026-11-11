//! Data types shared by storage, importers and the engine.

use std::fmt;
use std::str::FromStr;

use chrono::{NaiveDateTime, Timelike as _};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Price of one wish, in primogems.
pub const PRIMOGEMS_PER_WISH: u32 = 160;

/// A string did not name any variant of a text enum.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown {kind} `{value}`")]
pub struct UnknownVariant {
    kind: &'static str,
    value: String,
}

impl UnknownVariant {
    #[doc(hidden)]
    pub fn new(kind: &'static str, value: &str) -> Self {
        Self {
            kind,
            value: value.to_owned(),
        }
    }
}

/// Declares an enum stored as fixed text, in SQLite and in JSON alike.
macro_rules! text_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident => $text:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $( $(#[$vmeta])* $variant ),+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $( $name::$variant => $text ),+
                }
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = $crate::model::UnknownVariant;

            fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
                match s {
                    $( $text => Ok($name::$variant), )+
                    _ => Err($crate::model::UnknownVariant::new(stringify!($name), s)),
                }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> ::std::result::Result<Self, D::Error> {
                <String as ::serde::Deserialize>::deserialize(deserializer)?
                    .parse()
                    .map_err(<D::Error as ::serde::de::Error>::custom)
            }
        }

        impl ::rusqlite::types::ToSql for $name {
            fn to_sql(&self) -> ::rusqlite::Result<::rusqlite::types::ToSqlOutput<'_>> {
                Ok(::rusqlite::types::ToSqlOutput::from(self.as_str()))
            }
        }

        impl ::rusqlite::types::FromSql for $name {
            fn column_result(value: ::rusqlite::types::ValueRef<'_>) -> ::rusqlite::types::FromSqlResult<Self> {
                value
                    .as_str()?
                    .parse()
                    .map_err(|e| ::rusqlite::types::FromSqlError::Other(Box::new(e)))
            }
        }
    };
}
pub(crate) use text_enum;

text_enum! {
    /// Wishes in the same pool share pity. Character Event Wish and
    /// Character Event Wish-2 (gacha types 301 and 400) are one pool.
    /// Declared in paimon.moe's display order, which `Ord` follows.
    pub enum Pool {
        CharacterEvent => "character-event",
        WeaponEvent => "weapon-event",
        Standard => "standard",
        Chronicled => "chronicled",
        Beginners => "beginners",
    }
}

impl Pool {
    /// The gacha type used for this pool when the source does not say.
    pub fn default_gacha_type(self) -> GachaType {
        match self {
            Pool::Beginners => GachaType::Beginners,
            Pool::Standard => GachaType::Standard,
            Pool::CharacterEvent => GachaType::CharacterEvent,
            Pool::WeaponEvent => GachaType::WeaponEvent,
            Pool::Chronicled => GachaType::Chronicled,
        }
    }

    /// Character and weapon event banners have rate-up items (and a 50/50).
    /// paimon.moe does not track rate-up on Chronicled Wish, nor do we yet.
    pub fn has_rate_up(self) -> bool {
        matches!(self, Pool::CharacterEvent | Pool::WeaponEvent)
    }

    /// Wishes after which a 5★ is guaranteed.
    pub fn hard_pity_five(self) -> u32 {
        match self {
            Pool::WeaponEvent => 80,
            _ => 90,
        }
    }

    /// Wishes after which a 4★ or better is guaranteed.
    pub fn hard_pity_four(self) -> u32 {
        10
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Pool::Beginners => "Beginners' Wish",
            Pool::Standard => "Standard",
            Pool::CharacterEvent => "Character Event",
            Pool::WeaponEvent => "Weapon Event",
            Pool::Chronicled => "Chronicled Wish",
        }
    }
}

text_enum! {
    /// The `gacha_type` code returned by the game's API.
    pub enum GachaType {
        Beginners => "100",
        Standard => "200",
        CharacterEvent => "301",
        CharacterEvent2 => "400",
        WeaponEvent => "302",
        Chronicled => "500",
    }
}

impl GachaType {
    pub fn pool(self) -> Pool {
        match self {
            GachaType::Beginners => Pool::Beginners,
            GachaType::Standard => Pool::Standard,
            GachaType::CharacterEvent | GachaType::CharacterEvent2 => Pool::CharacterEvent,
            GachaType::WeaponEvent => Pool::WeaponEvent,
            GachaType::Chronicled => Pool::Chronicled,
        }
    }
}

text_enum! {
    pub enum ItemType {
        Character => "Character",
        Weapon => "Weapon",
        /// A 3★ placeholder added by paimon.moe's manual counter (`+1`, `+10`).
        Unknown => "Unknown",
    }
}

text_enum! {
    /// Where a wish record came from.
    pub enum Source {
        /// Fetched from the game's wish history API.
        Api => "api",
        /// Migrated from a paimon.moe export.
        Paimon => "paimon",
        /// Entered by hand in this app.
        Manual => "manual",
    }
}

text_enum! {
    /// Game server. It decides how the API's local times line up with banner times.
    pub enum Server {
        Asia => "asia",
        America => "america",
        Europe => "europe",
        TwHkMo => "tw_hk_mo",
        China => "china",
    }
}

impl Server {
    /// The server's fixed UTC offset (no daylight saving time).
    pub fn utc_offset_hours(self) -> i32 {
        match self {
            Server::Asia | Server::TwHkMo | Server::China => 8,
            Server::America => -5,
            Server::Europe => 1,
        }
    }

    /// Maps the `region` field of an API response.
    pub fn from_api_region(region: &str) -> Option<Self> {
        match region {
            "os_asia" => Some(Server::Asia),
            "os_usa" => Some(Server::America),
            "os_euro" => Some(Server::Europe),
            "os_cht" => Some(Server::TwHkMo),
            _ if region.starts_with("cn_") => Some(Server::China),
            _ => None,
        }
    }

    /// Maps paimon.moe's `server` setting. Its "Asia" also covers TW/HK/MO,
    /// which has the same offset.
    pub fn from_paimon(name: &str) -> Option<Self> {
        match name {
            "Asia" => Some(Server::Asia),
            "America" => Some(Server::America),
            "Europe" => Some(Server::Europe),
            "China" => Some(Server::China),
            _ => None,
        }
    }
}

/// Star rating of a wished item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rarity {
    Three = 3,
    Four = 4,
    Five = 5,
}

impl Rarity {
    pub fn from_stars(stars: u8) -> Option<Self> {
        match stars {
            3 => Some(Rarity::Three),
            4 => Some(Rarity::Four),
            5 => Some(Rarity::Five),
            _ => None,
        }
    }

    pub fn stars(self) -> u8 {
        self as u8
    }
}

impl Serialize for Rarity {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.stars())
    }
}

impl<'de> Deserialize<'de> for Rarity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let stars = u8::deserialize(deserializer)?;
        Rarity::from_stars(stars).ok_or_else(|| D::Error::custom(format!("invalid rarity {stars}")))
    }
}

impl ToSql for Rarity {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(i64::from(self.stars())))
    }
}

impl FromSql for Rarity {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let stars = value.as_i64()?;
        u8::try_from(stars)
            .ok()
            .and_then(Rarity::from_stars)
            .ok_or(FromSqlError::OutOfRange(stars))
    }
}

/// A wish time as the game reports it: server-local wall clock, no time zone.
///
/// It is kept exactly as received (`YYYY-MM-DD HH:MM:SS`); converting it to an
/// instant needs the account's [`Server`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WishTime(NaiveDateTime);

impl WishTime {
    pub const FORMAT: &'static str = "%Y-%m-%d %H:%M:%S";

    pub fn new(time: NaiveDateTime) -> Self {
        Self(time)
    }

    pub fn naive(self) -> NaiveDateTime {
        self.0
    }

    /// Digits only (`YYYYMMDDHHMMSS`), for building identifiers.
    pub fn compact(self) -> String {
        self.0.format("%Y%m%d%H%M%S").to_string()
    }

    /// The current time on `server`, to the second, as the game would record it.
    pub fn now_on(server: Server) -> Self {
        let local = chrono::Utc::now().naive_utc()
            + chrono::TimeDelta::hours(i64::from(server.utc_offset_hours()));
        Self(local.with_nanosecond(0).unwrap_or(local))
    }
}

/// The text is not in `YYYY-MM-DD HH:MM:SS` form.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid wish time `{0}` (expected YYYY-MM-DD HH:MM:SS)")]
pub struct InvalidWishTime(String);

impl FromStr for WishTime {
    type Err = InvalidWishTime;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Length check first: chrono accepts unpadded fields such as "2024-1-1 9:00:00".
        if s.len() != 19 {
            return Err(InvalidWishTime(s.to_owned()));
        }
        NaiveDateTime::parse_from_str(s, Self::FORMAT)
            .map(Self)
            .map_err(|_| InvalidWishTime(s.to_owned()))
    }
}

impl fmt::Display for WishTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.format(Self::FORMAT))
    }
}

impl Serialize for WishTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for WishTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl ToSql for WishTime {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

impl FromSql for WishTime {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value
            .as_str()?
            .parse()
            .map_err(|e| FromSqlError::Other(Box::new(e)))
    }
}

/// Identifies a wish within an account.
///
/// Wishes fetched from the API keep the API's numeric id. Other sources get a
/// deterministic id, so importing the same file twice yields the same ids.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WishId(String);

impl WishId {
    /// The id of a wish fetched from the API (1 to 19 digits).
    pub fn from_api(id: &str) -> Option<Self> {
        let valid = (1..=19).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit());
        valid.then(|| Self(id.to_owned()))
    }

    /// A wish read from a UIGF file under an id made up by an exporter
    /// (see [`crate::uigf::is_made_up_id`]). Kept apart from real game ids.
    pub fn made_up(id: &str) -> Self {
        Self(format!("uigf:{id}"))
    }

    /// A wish entered by hand in this app. `stamp` makes it unique (nanoseconds
    /// of the entry), `ix` orders the wishes of one entry (`+10`).
    pub fn manual(time: WishTime, stamp: i64, ix: u32) -> Self {
        Self(format!("manual:{}:{stamp:019}:{ix:02}", time.compact()))
    }

    /// The id of a wish migrated from paimon.moe, which stores no API id.
    /// `seq` is the wish's position among same-time wishes of the pool.
    pub fn paimon(pool: Pool, time: WishTime, seq: u32) -> Self {
        Self(format!("paimon:{pool}:{}:{seq}", time.compact()))
    }

    pub fn is_api(&self) -> bool {
        self.0.bytes().all(|b| b.is_ascii_digit())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WishId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl ToSql for WishId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for WishId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        String::column_result(value).map(Self)
    }
}

/// Local database id of an account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(pub i64);

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl ToSql for AccountId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for AccountId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Self)
    }
}

/// A game account (one UID), or a profile with no UID yet (manual counting).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub id: AccountId,
    pub label: String,
    pub uid: Option<String>,
    pub server: Option<Server>,
}

/// One wish, exactly as recorded by its source. Pity, banner and 50/50 are
/// derived from these records and never stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wish {
    pub id: WishId,
    pub gacha_type: GachaType,
    pub time: WishTime,
    /// Position among the pool's wishes that share `time` (a 10-pull is 0..=9).
    pub seq: u32,
    /// English item name, as the API returns it with `lang=en-us`.
    pub name: String,
    pub item_type: ItemType,
    pub rarity: Rarity,
    /// paimon.moe's item key (`kaedehara_kazuha`), when the item is known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_key: Option<String>,
    /// Pity typed by hand in paimon.moe's manual mode. It is source data, not
    /// a derived value: such entries may have no 3★ wishes before them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_pity: Option<u32>,
    pub source: Source,
}

impl Wish {
    pub fn pool(&self) -> Pool {
        self.gacha_type.pool()
    }
}

/// Counters typed by hand in paimon.moe's manual mode ("edit" on a counter).
/// They can disagree with the wish list, so they are kept as they were.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualCounters {
    pub pool: Pool,
    pub total: u32,
    pub pity5: u32,
    pub pity4: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wish_time_round_trips_and_rejects_other_formats() {
        let time: WishTime = "2024-03-05 07:08:09".parse().unwrap();
        assert_eq!(time.to_string(), "2024-03-05 07:08:09");
        assert_eq!(time.compact(), "20240305070809");

        for bad in ["2024-3-5 7:08:09", "2024-03-05T07:08:09", "2024-03-05", ""] {
            assert!(bad.parse::<WishTime>().is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn text_enums_round_trip() {
        for pool in Pool::ALL {
            assert_eq!(pool.as_str().parse::<Pool>(), Ok(*pool));
        }
        for gacha_type in GachaType::ALL {
            assert_eq!(gacha_type.as_str().parse::<GachaType>(), Ok(*gacha_type));
        }
        assert!("character".parse::<ItemType>().is_err());
    }

    #[test]
    fn character_event_wishes_share_one_pool() {
        assert_eq!(GachaType::CharacterEvent.pool(), Pool::CharacterEvent);
        assert_eq!(GachaType::CharacterEvent2.pool(), Pool::CharacterEvent);
    }

    #[test]
    fn servers_map_from_api_regions_and_paimon_names() {
        assert_eq!(Server::from_api_region("os_usa"), Some(Server::America));
        assert_eq!(Server::from_api_region("cn_qd01"), Some(Server::China));
        assert_eq!(Server::from_api_region("mars"), None);
        assert_eq!(Server::from_paimon("Europe"), Some(Server::Europe));
        assert_eq!(Server::America.utc_offset_hours(), -5);
    }

    #[test]
    fn api_ids_are_digits_only() {
        assert!(WishId::from_api("1672000000000000001").is_some());
        assert!(WishId::from_api("").is_none());
        assert!(WishId::from_api("12a").is_none());
        assert!(WishId::from_api("12345678901234567890").is_none());

        let migrated = WishId::paimon(
            Pool::CharacterEvent,
            "2024-01-01 10:00:00".parse().unwrap(),
            3,
        );
        assert_eq!(migrated.as_str(), "paimon:character-event:20240101100000:3");
        assert!(!migrated.is_api());
    }
}
