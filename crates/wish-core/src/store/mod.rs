//! SQLite storage: the single source of truth for the player's data (wishes,
//! accounts, the collection, plans and inventories).
//!
//! [`Store`] owns the connection. Its methods are split by subject:
//!
//! - this file: opening, migrations (`MIGRATIONS`, `schema/`), and
//!   [`Changed`], which tells the app whose analysis a write may have changed;
//! - `accounts`: adding, identifying, renaming and removing accounts;
//! - `wishes`: reading wishes, manual counters and game ids, deleting wishes
//!   counted by hand;
//! - `import`: applying an [`ImportBatch`](crate::import::ImportBatch), undoing one, the import log;
//! - `collection`, `plan`, `inventory`: what an account owns, its plan and
//!   its bag; `merge`: a snapshot's collection, plan and inventory merged in;
//! - `backup`: backups, their rotation, and restoring one.
//!
//! The data-safety rules hold for every write: imports only add rows
//! (`ON CONFLICT … DO NOTHING`, never `INSERT OR IGNORE`), a backup comes
//! before any import, migration, restore or bulk edit, and a write that
//! changes several rows is one transaction.

mod accounts;
mod backup;
mod collection;
mod import;
mod inventory;
mod merge;
mod plan;
mod wishes;

use std::collections::BTreeSet;
use std::fs;
use std::time::Duration;

use rusqlite::{Connection, Row};

pub use backup::{BackupFile, BackupReason, KEEP_BACKUPS, KEEP_HOURLY_BACKUPS};
pub use collection::SyncTargets;

use crate::error::{Error, IoAction, Result};
use crate::model::{Account, AccountId, Wish};
use crate::paths::AppPaths;

/// Schema migrations, applied in order. `PRAGMA user_version` holds how many
/// have run. Never edit a released migration: append a new one.
const MIGRATIONS: &[&str] = &[
    include_str!("schema/001_initial.sql"),
    include_str!("schema/002_links_and_undo.sql"),
    include_str!("schema/003_planner.sql"),
    include_str!("schema/004_inventory_and_custom_characters.sql"),
    include_str!("schema/005_planner_weapons.sql"),
    include_str!("schema/006_planner_custom_weapons.sql"),
    include_str!("schema/007_collection.sql"),
];

/// The `wishes` columns [`wish_from_row`] reads, in its order.
const WISH_COLUMNS: &str =
    "id, gacha_type, time, seq, name, item_type, rarity, item_key, manual_pity, source";

/// The database of one data folder. One process at a time opens it
/// ([`AppPaths::lock`]); the app holds it behind a mutex and writes through
/// its write queue, `wishctl` uses it directly.
pub struct Store {
    conn: Connection,
    paths: AppPaths,
    /// What the writes since the last [`Store::take_changed`] changed of what
    /// an analysis reads.
    changed: Changed,
}

/// The accounts whose analysis a write may have changed: their wishes,
/// manual counters or server, what [`crate::engine::analyze`] reads. A cache
/// of analyses works out these again and keeps the others.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Changed {
    /// Every account (the store was just opened, or a backup restored).
    All,
    Accounts(BTreeSet<AccountId>),
}

impl Changed {
    pub fn includes(&self, account: AccountId) -> bool {
        match self {
            Changed::All => true,
            Changed::Accounts(accounts) => accounts.contains(&account),
        }
    }
}

impl Store {
    /// Opens the database under `paths`, creating or migrating it as needed.
    ///
    /// An existing database is backed up before a migration, and once a day.
    /// A database written by a newer build is refused without being modified.
    pub fn open(paths: AppPaths) -> Result<Self> {
        let root = paths.root();
        fs::create_dir_all(root).map_err(Error::io(IoAction::Creating, root))?;
        let db_path = paths.database();
        let existed = db_path.exists();

        let conn = Connection::open(&db_path)?;
        // Check the version before anything else writes to the file.
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let latest = MIGRATIONS.len() as i64;
        if version > latest {
            return Err(Error::SchemaTooNew {
                found: version,
                known: latest,
            });
        }

        conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        // No function with side effects runs from the schema (defence in
        // depth: a restore refuses triggers and views anyway).
        conn.pragma_update(None, "trusted_schema", false)?;
        conn.busy_timeout(Duration::from_secs(5))?;

        let mut store = Self {
            conn,
            paths,
            changed: Changed::All,
        };
        if version < latest {
            if existed && version > 0 {
                store.backup(BackupReason::BeforeMigration)?;
            }
            store.migrate(version)?;
        }
        if existed {
            store.backup_if_due()?;
        }
        Ok(store)
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    fn migrate(&mut self, from: i64) -> Result<()> {
        let pending = MIGRATIONS.iter().enumerate().skip(from as usize);
        for (index, sql) in pending {
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", index as i64 + 1)?;
            tx.commit()?;
        }
        Ok(())
    }

    /// What the writes since the last call changed of what an analysis
    /// reads; from then on, nothing.
    pub fn take_changed(&mut self) -> Changed {
        std::mem::replace(&mut self.changed, Changed::Accounts(BTreeSet::new()))
    }

    /// Notes that `account`'s analysis may have changed.
    fn changed(&mut self, account: AccountId) {
        if let Changed::Accounts(accounts) = &mut self.changed {
            accounts.insert(account);
        }
    }
}

fn account_from_row(row: &Row<'_>) -> rusqlite::Result<Account> {
    Ok(Account {
        id: row.get(0)?,
        label: row.get(1)?,
        uid: row.get(2)?,
        server: row.get(3)?,
    })
}

fn wish_from_row(row: &Row<'_>) -> rusqlite::Result<Wish> {
    Ok(Wish {
        id: row.get(0)?,
        gacha_type: row.get(1)?,
        time: row.get(2)?,
        seq: row.get(3)?,
        name: row.get(4)?,
        item_type: row.get(5)?,
        rarity: row.get(6)?,
        item_key: row.get(7)?,
        manual_pity: row.get(8)?,
        source: row.get(9)?,
    })
}
