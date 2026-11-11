//! The player's inventory in the database: one row per material they have
//! some of. Every write is one transaction, after at most one backup an hour
//! (`before-planning`, like the plan's).

use chrono::{Local, SecondsFormat};
use rusqlite::{Transaction, params};

use super::Store;
use super::plan::item_columns;
use crate::error::Result;
use crate::planner::{CustomItem, Inventory};

impl Store {
    /// Everything the player has entered.
    pub fn inventory(&self) -> Result<Inventory> {
        let mut stmt = self
            .conn
            .prepare("SELECT material_id, name, count FROM inventory")?;
        let mut rows = stmt.query([])?;
        let mut inventory = Inventory::default();
        while let Some(row) = rows.next()? {
            let count = u64::try_from(row.get::<_, i64>(2)?).unwrap_or(0);
            let item = match row.get::<_, Option<i64>>(0)? {
                Some(id) => CustomItem::Material(id as u32),
                None => CustomItem::Named(row.get(1)?),
            };
            inventory.set(&item, count);
        }
        Ok(inventory)
    }

    /// Records that the player has `count` of `item`; none forgets it.
    pub fn set_owned(&mut self, item: &CustomItem, count: u64) -> Result<()> {
        self.set_owned_many(&[(item.clone(), count)])
    }

    /// Records several counts at once (the tiers of a series), in one
    /// transaction.
    pub fn set_owned_many(&mut self, counts: &[(CustomItem, u64)]) -> Result<()> {
        self.backup_for_planning()?;
        let tx = self.conn.transaction()?;
        let now = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
        for (item, count) in counts {
            set_owned(&tx, item, *count, &now)?;
        }
        tx.commit()?;
        Ok(())
    }
}

fn set_owned(tx: &Transaction, item: &CustomItem, count: u64, now: &str) -> Result<()> {
    // No inventory comes near the limit of the signed column.
    let count = i64::try_from(count).unwrap_or(i64::MAX);
    match item_columns(item) {
        (Some(id), _) if count == 0 => {
            tx.execute("DELETE FROM inventory WHERE material_id = ?1", [id])?;
        }
        (None, name) if count == 0 => {
            tx.execute("DELETE FROM inventory WHERE name = ?1", [name])?;
        }
        (Some(id), _) => {
            tx.execute(
                "INSERT INTO inventory (material_id, count, updated_at) VALUES (?1, ?2, ?3) \
                 ON CONFLICT (material_id) DO UPDATE \
                 SET count = excluded.count, updated_at = excluded.updated_at",
                params![id, count, now],
            )?;
        }
        (None, name) => {
            tx.execute(
                "INSERT INTO inventory (name, count, updated_at) VALUES (?1, ?2, ?3) \
                 ON CONFLICT (name) DO UPDATE \
                 SET count = excluded.count, updated_at = excluded.updated_at",
                params![name, count, now],
            )?;
        }
    }
    Ok(())
}
