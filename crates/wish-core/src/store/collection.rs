//! What each account owns, in the database: its characters and its weapons
//! (one row per copy). Every write is one transaction, after at most one
//! backup an hour (`before-planning`, like the plan's and the inventory's).

use chrono::{Local, SecondsFormat};
use rusqlite::{Row, Transaction, params};

use super::Store;
use crate::collection::{Collection, OwnedCharacter, OwnedWeapon};
use crate::error::Result;
use crate::model::AccountId;
use crate::planner::Level;

impl Store {
    /// What `account` owns.
    pub fn collection(&self, account: AccountId) -> Result<Collection> {
        let mut characters = self.conn.prepare(
            "SELECT character_key, level, phase, constellation, talent1, talent2, talent3, \
             favourite FROM owned_characters WHERE account_id = ?1 ORDER BY character_key",
        )?;
        let characters = characters
            .query_map([account.0], read_character)?
            .collect::<rusqlite::Result<_>>()?;
        let mut weapons = self.conn.prepare(
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
}

fn now() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

fn save_character(
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

fn write_weapon(
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
