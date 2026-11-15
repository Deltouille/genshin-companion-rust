//! What each account owns, in the database: its characters and its weapons
//! (one row per copy). Every write is one transaction, after at most one
//! backup an hour (`before-planning`, like the plan's and the inventory's).

use std::path::PathBuf;

use chrono::{Local, SecondsFormat};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use super::{Store, account_from_row, account_label};
use crate::archive::ArchiveData;
use crate::collection::{Change, Collection, OwnedCharacter, OwnedWeapon, SyncPlan, plan_sync};
use crate::error::{Error, Result};
use crate::model::{Account, AccountId, Server};
use crate::planner::{Level, PlannerData};
use crate::store::BackupReason;

impl Store {
    /// What `account` owns.
    pub fn collection(&self, account: AccountId) -> Result<Collection> {
        read_collection(&self.conn, account)
    }

    /// Adds `character` to `account`, or replaces what was entered for it.
    pub fn save_owned_character(
        &mut self,
        account: AccountId,
        character: &OwnedCharacter,
    ) -> Result<()> {
        self.save_owned_characters(account, std::slice::from_ref(character))
    }

    /// Adds or replaces several characters in one transaction.
    pub fn save_owned_characters(
        &mut self,
        account: AccountId,
        characters: &[OwnedCharacter],
    ) -> Result<()> {
        self.backup_for_planning()?;
        let tx = self.conn.transaction()?;
        let now = now();
        for character in characters {
            save_character(&tx, account, character, &now)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Forgets that `account` owns `key`. The weapon it held is kept,
    /// held by nobody.
    pub fn remove_owned_character(&mut self, account: AccountId, key: &str) -> Result<()> {
        self.backup_for_planning()?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM owned_characters WHERE account_id = ?1 AND character_key = ?2",
            params![account.0, key],
        )?;
        tx.execute(
            "UPDATE owned_weapons SET holder_key = NULL WHERE account_id = ?1 AND holder_key = ?2",
            params![account.0, key],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Adds a copy of a weapon to `account`; returns its id. A holder who
    /// held another weapon lets go of it.
    pub fn add_owned_weapon(&mut self, account: AccountId, weapon: &OwnedWeapon) -> Result<i64> {
        self.backup_for_planning()?;
        let tx = self.conn.transaction()?;
        let now = now();
        tx.execute(
            "INSERT INTO owned_weapons (account_id, weapon_key, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?3)",
            params![account.0, weapon.key, now],
        )?;
        let id = tx.last_insert_rowid();
        write_weapon(&tx, account, id, weapon, &now)?;
        tx.commit()?;
        Ok(id)
    }

    /// Replaces what was entered for the copy `weapon.id`.
    pub fn update_owned_weapon(&mut self, account: AccountId, weapon: &OwnedWeapon) -> Result<()> {
        self.backup_for_planning()?;
        let tx = self.conn.transaction()?;
        write_weapon(&tx, account, weapon.id, weapon, &now())?;
        tx.commit()?;
        Ok(())
    }

    /// Forgets the copy `id`.
    pub fn remove_owned_weapon(&mut self, account: AccountId, id: i64) -> Result<()> {
        self.backup_for_planning()?;
        self.conn.execute(
            "DELETE FROM owned_weapons WHERE account_id = ?1 AND id = ?2",
            params![account.0, id],
        )?;
        Ok(())
    }

    /// The accounts a HoYoLAB sync of `uid` can go to, as the game import
    /// offers them: the account with that UID; without one, a new account
    /// first, then each account without a UID yet (its history may be this
    /// player's, from a file without a UID or counted by hand). Suggested:
    /// the account `shown` when it has no UID, else the only account without
    /// one, else a new account.
    pub fn sync_targets(&self, uid: &str, shown: Option<AccountId>) -> Result<SyncTargets> {
        let accounts = self.accounts()?;
        if let Some(account) = accounts.iter().find(|a| a.uid.as_deref() == Some(uid)) {
            return Ok(SyncTargets {
                targets: vec![Some(account.clone())],
                suggested: 0,
            });
        }
        let free: Vec<Account> = accounts.into_iter().filter(|a| a.uid.is_none()).collect();
        let suggested = match free.iter().position(|a| Some(a.id) == shown) {
            Some(ix) => ix + 1,
            None if free.len() == 1 => 1,
            None => 0,
        };
        Ok(SyncTargets {
            targets: std::iter::once(None)
                .chain(free.into_iter().map(Some))
                .collect(),
            suggested,
        })
    }

    /// Applies a HoYoLAB sync of `game` once the player confirmed its
    /// preview, into `target` (one of [`Store::sync_targets`]; `None` for a
    /// new account, named after the game's nickname). An account without a
    /// UID takes the game's, and its server if it has none, in the same
    /// transaction; if another sync gave the UID to an account meanwhile,
    /// a new account goes there instead. A backup first, then the plan worked
    /// out again against the collection as it is now (so what was entered
    /// meanwhile is never lowered either), written in one transaction.
    /// Returns the account, what changed and the backup.
    pub fn apply_sync(
        &mut self,
        target: Option<AccountId>,
        game: &hoyolab::GameAccount,
        details: &[hoyolab::CharacterDetail],
        planner: &PlannerData,
        archive: &ArchiveData,
    ) -> Result<(AccountId, SyncPlan, PathBuf)> {
        let backup = self.backup(BackupReason::BeforeSync)?;
        let tx = self.conn.transaction()?;
        let now = now();
        let account = sync_account(&tx, target, game, &now)?;
        let plan = plan_sync(details, &read_collection(&tx, account)?, planner, archive);
        for change in plan.characters.iter().filter(|c| c.changes()) {
            save_character(&tx, account, change.after(), &now)?;
        }
        for change in &plan.weapons {
            match change {
                Change::Added(weapon) => {
                    tx.execute(
                        "INSERT INTO owned_weapons (account_id, weapon_key, created_at, updated_at) \
                         VALUES (?1, ?2, ?3, ?3)",
                        params![account.0, weapon.key, now],
                    )?;
                    let id = tx.last_insert_rowid();
                    write_weapon(&tx, account, id, weapon, &now)?;
                }
                Change::Raised { after, .. } => write_weapon(&tx, account, after.id, after, &now)?,
                Change::Unchanged(_) => {}
            }
        }
        tx.commit()?;
        // The account may be new, or have taken a server.
        self.changed(account);
        Ok((account, plan, backup))
    }
}

/// Where a HoYoLAB sync can go: `None` is a new account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncTargets {
    pub targets: Vec<Option<Account>>,
    /// Index into `targets` of the suggested choice.
    pub suggested: usize,
}

/// The account a sync of `game` writes into, given the player's choice:
/// `target` takes the game's UID if it has none, or a new account is made
/// (unless an account has the UID by now).
fn sync_account(
    tx: &Transaction,
    target: Option<AccountId>,
    game: &hoyolab::GameAccount,
    now: &str,
) -> Result<AccountId> {
    let uid = game.uid.trim();
    if !(9..=10).contains(&uid.len()) || !uid.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::InvalidUid(uid.to_owned()));
    }
    let server = Server::from_api_region(&game.region);
    let find = |condition: &str, value: &dyn rusqlite::ToSql| -> Result<Option<Account>> {
        let sql = format!(
            "SELECT id, label, uid, server FROM accounts WHERE {condition} ORDER BY id LIMIT 1"
        );
        Ok(tx.query_row(&sql, [value], account_from_row).optional()?)
    };
    let holder = find("uid = ?1", &uid)?;
    let account = match target {
        Some(id) => {
            let mut account = find("id = ?1", &id.0)?.ok_or(Error::UnknownAccount(id))?;
            match (account.uid.as_deref(), holder) {
                (Some(theirs), _) if theirs != uid => {
                    return Err(Error::AccountMismatch {
                        account: account.label,
                        uid: uid.to_owned(),
                    });
                }
                (Some(_), _) => {}
                (None, Some(holder)) => {
                    return Err(Error::UidTaken {
                        uid: uid.to_owned(),
                        account: holder.label,
                    });
                }
                (None, None) => {
                    tx.execute(
                        "UPDATE accounts SET uid = ?1 WHERE id = ?2",
                        params![uid, account.id],
                    )?;
                    account.uid = Some(uid.to_owned());
                }
            }
            account
        }
        None => match holder {
            Some(holder) => holder,
            None => {
                let label = account_label(match game.nickname.trim() {
                    "" => uid,
                    nickname => nickname,
                })?;
                tx.execute(
                    "INSERT INTO accounts (label, uid, server, created_at) VALUES (?1, ?2, ?3, ?4)",
                    params![label, uid, server, now],
                )?;
                Account {
                    id: AccountId(tx.last_insert_rowid()),
                    label,
                    uid: Some(uid.to_owned()),
                    server,
                }
            }
        },
    };
    if account.server.is_none() && server.is_some() {
        tx.execute(
            "UPDATE accounts SET server = ?1 WHERE id = ?2",
            params![server, account.id],
        )?;
    }
    Ok(account.id)
}

/// What `account` owns, read through `conn` (a transaction too).
pub(super) fn read_collection(conn: &Connection, account: AccountId) -> Result<Collection> {
    let mut characters = conn.prepare(
        "SELECT character_key, level, phase, constellation, talent1, talent2, talent3, \
         favourite FROM owned_characters WHERE account_id = ?1 ORDER BY character_key",
    )?;
    let characters = characters
        .query_map([account.0], read_character)?
        .collect::<rusqlite::Result<_>>()?;
    let mut weapons = conn.prepare(
        "SELECT id, weapon_key, level, phase, refinement, holder_key, favourite \
         FROM owned_weapons WHERE account_id = ?1 ORDER BY id",
    )?;
    let weapons = weapons
        .query_map([account.0], read_weapon)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Collection {
        characters,
        weapons,
    })
}

pub(super) fn now() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

pub(super) fn save_character(
    tx: &Transaction,
    account: AccountId,
    character: &OwnedCharacter,
    now: &str,
) -> Result<()> {
    let [t1, t2, t3] = character.talents;
    tx.execute(
        "INSERT INTO owned_characters (account_id, character_key, level, phase, constellation, \
         talent1, talent2, talent3, favourite, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10) \
         ON CONFLICT (account_id, character_key) DO UPDATE SET \
         level = excluded.level, phase = excluded.phase, \
         constellation = excluded.constellation, talent1 = excluded.talent1, \
         talent2 = excluded.talent2, talent3 = excluded.talent3, \
         favourite = excluded.favourite, updated_at = excluded.updated_at",
        params![
            account.0,
            character.key,
            character.level.level,
            character.level.phase,
            character.constellation,
            t1,
            t2,
            t3,
            character.favourite,
            now
        ],
    )?;
    Ok(())
}

pub(super) fn write_weapon(
    tx: &Transaction,
    account: AccountId,
    id: i64,
    weapon: &OwnedWeapon,
    now: &str,
) -> Result<()> {
    if let Some(holder) = &weapon.holder {
        tx.execute(
            "UPDATE owned_weapons SET holder_key = NULL \
             WHERE account_id = ?1 AND holder_key = ?2 AND id <> ?3",
            params![account.0, holder, id],
        )?;
    }
    tx.execute(
        "UPDATE owned_weapons SET weapon_key = ?3, level = ?4, phase = ?5, refinement = ?6, \
         holder_key = ?7, favourite = ?8, updated_at = ?9 WHERE account_id = ?1 AND id = ?2",
        params![
            account.0,
            id,
            weapon.key,
            weapon.level.level,
            weapon.level.phase,
            weapon.refinement,
            weapon.holder,
            weapon.favourite,
            now
        ],
    )?;
    Ok(())
}

/// A level and phase as stored; the CHECKs keep both in range, and a level
/// outside its phase is read as the phase's nearest level.
fn read_level(row: &Row, level: usize, phase: usize) -> rusqlite::Result<Level> {
    let (level, phase): (u8, u8) = (row.get(level)?, row.get(phase)?);
    Ok(Level::new(level, phase).unwrap_or_else(|| Level::clamped(level, phase)))
}

fn read_character(row: &Row) -> rusqlite::Result<OwnedCharacter> {
    Ok(OwnedCharacter {
        key: row.get(0)?,
        level: read_level(row, 1, 2)?,
        constellation: row.get(3)?,
        talents: [row.get(4)?, row.get(5)?, row.get(6)?],
        favourite: row.get(7)?,
    })
}

fn read_weapon(row: &Row) -> rusqlite::Result<OwnedWeapon> {
    Ok(OwnedWeapon {
        id: row.get(0)?,
        key: row.get(1)?,
        level: read_level(row, 2, 3)?,
        refinement: row.get(4)?,
        holder: row.get(5)?,
        favourite: row.get(6)?,
    })
}
