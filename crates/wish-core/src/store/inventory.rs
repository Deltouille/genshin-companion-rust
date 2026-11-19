//! Each account's inventory in the database: one row per material it has
//! some of. Every write is one transaction, after at most one backup an hour
//! (`before-planning`, like the plan's).

use chrono::{Local, SecondsFormat};
use rusqlite::{Connection, Transaction, params};

use super::Store;
use super::plan::item_columns;
use crate::error::Result;
use crate::model::AccountId;
use crate::planner::{CustomItem, Inventory};

impl Store {
    /// Everything the player has entered for `account`.
    pub fn inventory(&self, account: AccountId) -> Result<Inventory> {
        read_inventory(&self.conn, account)
    }

    /// Records that `account` has `count` of `item`; none forgets it.
    pub fn set_owned(&mut self, account: AccountId, item: &CustomItem, count: u64) -> Result<()> {
        self.set_owned_many(account, &[(item.clone(), count)])
    }

    /// Records several counts at once (the tiers of a series, a GOOD file),
    /// in one transaction.
    pub fn set_owned_many(
        &mut self,
        account: AccountId,
        counts: &[(CustomItem, u64)],
    ) -> Result<()> {
        self.backup_for_planning()?;
        let tx = self.conn.transaction()?;
        let now = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
        for (item, count) in counts {
            set_owned(&tx, account, item, *count, &now)?;
        }
        tx.commit()?;
        Ok(())
    }
}

/// `account`'s inventory, read through `conn` (a transaction too).
pub(super) fn read_inventory(conn: &Connection, account: AccountId) -> Result<Inventory> {
    let mut stmt =
        conn.prepare("SELECT material_id, name, count FROM inventory WHERE account_id = ?1")?;
    let mut rows = stmt.query([account.0])?;
    let mut inventory = Inventory::default();
    while let Some(row) = rows.next()? {
        // Out of range (a file restored from elsewhere, where `count > 0`
        // may not hold): an error, never another material or a count of 0.
        let count: i64 = row.get(2)?;
        let count =
            u64::try_from(count).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(2, count))?;
        let item = match row.get::<_, Option<u32>>(0)? {
            Some(id) => CustomItem::Material(id),
            None => CustomItem::Named(row.get(1)?),
        };
        inventory.set(&item, count);
    }
    Ok(inventory)
}

/// Sets how many of `item` the account has; 0 removes its row.
pub(super) fn set_owned(
    tx: &Transaction,
    account: AccountId,
    item: &CustomItem,
    count: u64,
    now: &str,
) -> Result<()> {
    // No inventory comes near the limit of the signed column.
    let count = i64::try_from(count).unwrap_or(i64::MAX);
    let account = account.0;
    match item_columns(item) {
        (Some(id), _) if count == 0 => {
            tx.execute(
                "DELETE FROM inventory WHERE account_id = ?1 AND material_id = ?2",
                params![account, id],
            )?;
        }
        (None, name) if count == 0 => {
            tx.execute(
                "DELETE FROM inventory WHERE account_id = ?1 AND name = ?2",
                params![account, name],
            )?;
        }
        (Some(id), _) => {
            tx.execute(
                "INSERT INTO inventory (account_id, material_id, count, updated_at) \
                 VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT (account_id, material_id) DO UPDATE \
                 SET count = excluded.count, updated_at = excluded.updated_at",
                params![account, id, count, now],
            )?;
        }
        (None, name) => {
            tx.execute(
                "INSERT INTO inventory (account_id, name, count, updated_at) \
                 VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT (account_id, name) DO UPDATE \
                 SET count = excluded.count, updated_at = excluded.updated_at",
                params![account, name, count, now],
            )?;
        }
    }
    Ok(())
}
