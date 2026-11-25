//! Accounts: the ones imports make on their own, and the ones the player adds,
//! identifies, renames or removes by hand.

use chrono::{Local, SecondsFormat};
use rusqlite::{OptionalExtension, params};

use super::{BackupReason, Store, account_from_row};
use crate::error::{Error, Result};
use crate::model::{Account, AccountId, Server};

impl Store {
    /// Every account, oldest first.
    pub fn accounts(&self) -> Result<Vec<Account>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, label, uid, server FROM accounts ORDER BY id")?;
        let accounts = stmt
            .query_map([], account_from_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(accounts)
    }

    /// The account `id`.
    pub fn account(&self, id: AccountId) -> Result<Account> {
        self.conn
            .query_row(
                "SELECT id, label, uid, server FROM accounts WHERE id = ?1",
                [id.0],
                account_from_row,
            )
            .optional()?
            .ok_or(Error::UnknownAccount(id))
    }

    /// Adds an account by hand (a player who imports no wishes, or not yet):
    /// a name, and optionally its UID and server. A UID already taken by
    /// another account is refused; a UID's server is the one its digits say.
    pub fn add_account(
        &mut self,
        label: &str,
        uid: Option<&str>,
        server: Option<Server>,
    ) -> Result<Account> {
        let label = account_label(label)?;
        let uid = self.free_uid(uid)?;
        let server = uid.and_then(Server::from_uid).or(server);
        let now = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
        self.conn.execute(
            "INSERT INTO accounts (label, uid, server, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![label, uid, server, now],
        )?;
        Ok(Account {
            id: AccountId(self.conn.last_insert_rowid()),
            label,
            uid: uid.map(str::to_owned),
            server,
        })
    }

    /// A UID typed for an account: `None` when left empty; refused when it
    /// isn't one, or another account has it.
    fn free_uid<'a>(&self, uid: Option<&'a str>) -> Result<Option<&'a str>> {
        let Some(uid) = uid.map(str::trim).filter(|uid| !uid.is_empty()) else {
            return Ok(None);
        };
        if !(9..=10).contains(&uid.len()) || !uid.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Error::InvalidUid(uid.to_owned()));
        }
        let taken: Option<String> = self
            .conn
            .query_row("SELECT label FROM accounts WHERE uid = ?1", [uid], |row| {
                row.get(0)
            })
            .optional()?;
        match taken {
            Some(account) => Err(Error::UidTaken {
                uid: uid.to_owned(),
                account,
            }),
            None => Ok(Some(uid)),
        }
    }

    /// Gives an account without a UID its UID, or only its server (a player
    /// who added it by hand, or whose wishes came from a file without one).
    /// A UID's server is the one its digits say. An account with a UID keeps
    /// it, and its server: the game import and HoYoLAB find it by them.
    pub fn identify_account(
        &mut self,
        account: AccountId,
        uid: Option<&str>,
        server: Option<Server>,
    ) -> Result<Account> {
        let before = self.account(account)?;
        if before.uid.is_some() {
            return Err(Error::AccountHasUid(before.label));
        }
        let uid = self.free_uid(uid)?;
        let server = uid.and_then(Server::from_uid).or(server);
        self.conn.execute(
            "UPDATE accounts SET uid = ?2, server = ?3 WHERE id = ?1",
            params![account.0, uid, server],
        )?;
        // The server places the banners of wishes in time.
        if server != before.server {
            self.changed(account);
        }
        Ok(Account {
            uid: uid.map(str::to_owned),
            server,
            ..before
        })
    }

    /// Removes an account that has no wishes (one added by mistake, or the
    /// "Main" a GOOD import made, no longer wanted), with its collection,
    /// plan and inventory, after a backup, in one transaction. An account
    /// with wishes, counters or imports in the history, or a chronicle, is
    /// refused: the wish history and what HoYoLAB no longer shows are the
    /// data the app keeps, and undoing an import needs its account.
    pub fn remove_account(&mut self, account: AccountId) -> Result<()> {
        let label = self.account(account)?.label;
        let history: bool = self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM wishes WHERE account_id = ?1)
                 OR EXISTS (SELECT 1 FROM manual_counters WHERE account_id = ?1)
                 OR EXISTS (SELECT 1 FROM imports WHERE account_id = ?1)
                 OR EXISTS (SELECT 1 FROM chronicle WHERE account_id = ?1)",
            [account.0],
            |row| row.get(0),
        )?;
        if history {
            return Err(Error::AccountHasWishes(label));
        }
        self.backup(BackupReason::BeforeRemovingAccount)?;
        let tx = self.conn.transaction()?;
        // A plan entry's custom costs and materials go with it (ON DELETE
        // CASCADE).
        for table in [
            "battle_pass",
            "plan_entries",
            "inventory",
            "owned_weapons",
            "owned_characters",
        ] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE account_id = ?1"),
                [account.0],
            )?;
        }
        tx.execute("DELETE FROM accounts WHERE id = ?1", [account.0])?;
        tx.commit()?;
        Ok(())
    }

    /// The account whose UID is `uid`, if there is one.
    pub fn account_with_uid(&self, uid: &str) -> Result<Option<Account>> {
        Ok(self
            .accounts()?
            .into_iter()
            .find(|account| account.uid.as_deref() == Some(uid)))
    }

    /// Gives `account` another name.
    pub fn rename_account(&mut self, account: AccountId, label: &str) -> Result<()> {
        let label = account_label(label)?;
        let changed = self.conn.execute(
            "UPDATE accounts SET label = ?2 WHERE id = ?1",
            params![account.0, label],
        )?;
        if changed == 0 {
            return Err(Error::UnknownAccount(account));
        }
        Ok(())
    }

    /// Whether an import (undone or not) ever went to `account`: an account
    /// an import made and its undo left empty is not shown, one added by
    /// hand is.
    pub fn has_imports(&self, account: AccountId) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM imports WHERE account_id = ?1)",
            [account.0],
            |row| row.get(0),
        )?)
    }
}

/// An account's name, trimmed; never empty.
pub(super) fn account_label(label: &str) -> Result<String> {
    let label = label.trim();
    if label.is_empty() {
        return Err(Error::EmptyAccountName);
    }
    Ok(label.to_owned())
}
