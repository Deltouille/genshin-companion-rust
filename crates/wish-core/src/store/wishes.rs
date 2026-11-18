//! Reading an account's wishes, manual counters and the game ids learned for
//! them, and deleting wishes counted by hand.

use std::collections::{HashMap, HashSet};

use rusqlite::{OptionalExtension, params};

use super::{BackupReason, Store, WISH_COLUMNS, wish_from_row};
use crate::error::{Error, Result};
use crate::import::WishLink;
use crate::model::{AccountId, ManualCounters, Source, Wish, WishId};

impl Store {
    /// The account's wishes in chronological order.
    pub fn wishes(&self, account: AccountId) -> Result<Vec<Wish>> {
        let sql = format!(
            "SELECT {WISH_COLUMNS} FROM wishes WHERE account_id = ?1 \
             ORDER BY time, seq, gacha_type, id"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let wishes = stmt
            .query_map([account], wish_from_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(wishes)
    }

    /// The counters typed by hand in paimon.moe's manual mode, per pool.
    pub fn manual_counters(&self, account: AccountId) -> Result<Vec<ManualCounters>> {
        let mut stmt = self.conn.prepare(
            "SELECT pool, total, pity5, pity4 FROM manual_counters WHERE account_id = ?1 ORDER BY pool",
        )?;
        let counters = stmt
            .query_map([account], |row| {
                Ok(ManualCounters {
                    pool: row.get(0)?,
                    total: row.get(1)?,
                    pity5: row.get(2)?,
                    pity4: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(counters)
    }

    /// How many wishes are stored, all accounts together.
    pub fn wish_count(&self) -> Result<u64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM wishes", [], |row| row.get(0))?;
        Ok(count.unsigned_abs())
    }

    /// Game ids learned for the account's migrated wishes.
    pub fn links(&self, account: AccountId) -> Result<Vec<WishLink>> {
        let mut stmt = self.conn.prepare(
            "SELECT wish_id, api_id FROM wish_links WHERE account_id = ?1 ORDER BY wish_id",
        )?;
        let links = stmt
            .query_map([account], |row| {
                Ok(WishLink {
                    wish_id: row.get(0)?,
                    api_id: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(links)
    }

    /// Every game id already known, per UID: wishes fetched from the game,
    /// and the ids learned for migrated wishes.
    pub fn known_api_ids(&self) -> Result<HashMap<String, HashSet<String>>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.uid, w.id FROM wishes w JOIN accounts a ON a.id = w.account_id \
             WHERE a.uid IS NOT NULL AND w.source = 'api' \
             UNION ALL \
             SELECT a.uid, l.api_id FROM wish_links l JOIN accounts a ON a.id = l.account_id \
             WHERE a.uid IS NOT NULL",
        )?;
        let mut known: HashMap<String, HashSet<String>> = HashMap::new();
        let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get(1)?)))?;
        for row in rows {
            let (uid, id) = row?;
            known.entry(uid).or_default().insert(id);
        }
        Ok(known)
    }

    /// Deletes wishes entered by hand in this app, after a backup. Wishes
    /// from any other source are refused: undo their import instead.
    pub fn delete_manual_wishes(&mut self, account: AccountId, ids: &[WishId]) -> Result<u32> {
        let mut check = self
            .conn
            .prepare("SELECT source FROM wishes WHERE account_id = ?1 AND id = ?2")?;
        for id in ids {
            let source: Option<Source> = check
                .query_row(params![account, id], |row| row.get(0))
                .optional()?;
            if source != Some(Source::Manual) {
                return Err(Error::NotManual(id.to_string()));
            }
        }
        drop(check);

        self.backup(BackupReason::BeforeDelete)?;
        let tx = self.conn.transaction()?;
        let mut deleted = 0;
        for id in ids {
            deleted += tx.execute(
                "DELETE FROM wishes WHERE account_id = ?1 AND id = ?2 AND source = 'manual'",
                params![account, id],
            )?;
        }
        tx.commit()?;
        self.changed(account);
        Ok(u32::try_from(deleted).unwrap_or(u32::MAX))
    }
}
