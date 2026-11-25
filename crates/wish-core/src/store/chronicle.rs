//! The chronicle in the database: one row per period HoYoLAB showed, its
//! answer kept whole. Written by a HoYoLAB sync and by a snapshot's import,
//! inside their transactions, by [`write_records`]: a period fetched once
//! over is never rewritten ([`crate::chronicle::merge`]).

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{OptionalExtension as _, Transaction, params};

use super::Store;
use crate::chronicle::{Merge, Record, RecordKind, RecordsPlan, merge};
use crate::error::Result;
use crate::model::AccountId;

const COLUMNS: &str = "kind, period, fetched_at, ended, data";

impl Store {
    /// Every record `account` keeps, by kind and period.
    pub fn chronicle(&self, account: AccountId) -> Result<Vec<Record>> {
        let mut statement = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM chronicle WHERE account_id = ?1 ORDER BY kind, period"
        ))?;
        let rows = statement.query_map([account.0], raw_record)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(record(row?)?);
        }
        Ok(records)
    }

    /// Writes the chronicle HoYoLAB showed into `account`, alone, without a
    /// preview: what a sync with the login the keyring keeps does. It only
    /// adds periods or brings one under way up to date (a period kept once
    /// over never changes), so nothing the player entered is touched. When
    /// something changes, a backup first (at most one an hour), then one
    /// transaction. Returns what changed.
    pub fn keep_chronicle(
        &mut self,
        account: AccountId,
        records: &[Record],
    ) -> Result<RecordsPlan> {
        let plan = RecordsPlan::new(records, &self.chronicle(account)?);
        if plan.changes() == 0 {
            return Ok(plan);
        }
        self.backup_for_chronicle()?;
        let tx = self.conn.transaction()?;
        let written = write_records(&tx, account, records)?;
        tx.commit()?;
        Ok(written)
    }
}

/// A row as SQLite has it.
type Raw = (RecordKind, u32, String, bool, String);

fn raw_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<Raw> {
    // A period out of range is an error, never another period.
    Ok((
        row.get(0)?,
        row.get::<_, u32>(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
    ))
}

fn record((kind, period, fetched_at, ended, data): Raw) -> Result<Record> {
    let fetched_at = DateTime::parse_from_rfc3339(&fetched_at)
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?
        .with_timezone(&Utc);
    Ok(Record {
        kind,
        period,
        fetched_at,
        ended,
        data: serde_json::from_str(&data)?,
    })
}

/// Writes `records` into `account`'s chronicle, inside `tx`: each period not
/// kept yet is added, one kept while under way is replaced by a newer fetch,
/// one kept once over stays as it is. Returns what changed.
pub(super) fn write_records(
    tx: &Transaction,
    account: AccountId,
    records: &[Record],
) -> Result<RecordsPlan> {
    let mut plan = RecordsPlan::default();
    for incoming in records {
        let stored = tx
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM chronicle \
                     WHERE account_id = ?1 AND kind = ?2 AND period = ?3"
                ),
                params![account.0, incoming.kind, incoming.period],
                raw_record,
            )
            .optional()?
            .map(record)
            .transpose()?;
        let merge = merge(stored.as_ref(), incoming);
        plan.count(incoming.kind, merge);
        let fetched_at = incoming
            .fetched_at
            .to_rfc3339_opts(SecondsFormat::Secs, true);
        let data = incoming.data.to_string();
        match merge {
            Merge::Add => {
                tx.execute(
                    "INSERT INTO chronicle (account_id, kind, period, fetched_at, ended, data) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        account.0,
                        incoming.kind,
                        incoming.period,
                        fetched_at,
                        incoming.ended,
                        data
                    ],
                )?;
            }
            Merge::Replace => {
                tx.execute(
                    "UPDATE chronicle SET fetched_at = ?4, ended = ?5, data = ?6 \
                     WHERE account_id = ?1 AND kind = ?2 AND period = ?3 AND ended = 0",
                    params![
                        account.0,
                        incoming.kind,
                        incoming.period,
                        fetched_at,
                        incoming.ended,
                        data
                    ],
                )?;
            }
            Merge::Keep => {}
        }
    }
    Ok(plan)
}
