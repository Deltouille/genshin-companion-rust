//! How the domain's types are stored in SQLite: rusqlite's `ToSql` and
//! `FromSql` for them, here so the domain (`model`, `planner`…) never
//! depends on the database (`tests/layers.rs`). A text enum is its text
//! ([`TextEnum`]); a rarity its number of stars; a wish time its
//! `YYYY-MM-DD HH:MM:SS` text; ids their value. What doesn't read back as one
//! of these is an error, never a guess.

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

use super::BackupReason;
use crate::chronicle::RecordKind;
use crate::import::ImportSource;
use crate::model::{
    AccountId, GachaType, ItemType, Pool, Rarity, Server, Source, TextEnum, WishId, WishTime,
};
use crate::planner::{RewardPlan, Role};

/// Stores each text enum as its text. (A blanket `impl<T: TextEnum> ToSql for
/// T` isn't allowed: the trait and the type parameter are both foreign to
/// rusqlite's crate rules.)
macro_rules! sql_text {
    ($($name:ty),+ $(,)?) => {
        $(
            impl ToSql for $name {
                fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                    Ok(ToSqlOutput::from(TextEnum::as_str(*self)))
                }
            }

            impl FromSql for $name {
                fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                    value
                        .as_str()?
                        .parse()
                        .map_err(|e| FromSqlError::Other(Box::new(e)))
                }
            }
        )+
    };
}

sql_text!(
    Pool,
    GachaType,
    ItemType,
    Source,
    Server,
    ImportSource,
    Role,
    RewardPlan,
    BackupReason,
    RecordKind,
);

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

impl ToSql for WishId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for WishId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        String::column_result(value).map(WishId::stored)
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
