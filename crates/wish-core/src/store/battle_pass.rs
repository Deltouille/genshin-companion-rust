//! Each account's Battle Pass in the database: one row, written whole, after at
//! most one backup an hour (`before-planning`, like the plan's).
//!
//! It isn't in snapshots or the mirror: a pass lasts one period, and its level
//! is entered again in the next.

use chrono::{DateTime, Local, SecondsFormat};
use rusqlite::{OptionalExtension as _, params};

use super::Store;
use crate::error::Result;
use crate::model::AccountId;
use crate::planner::{BATTLE_PASS_LEVELS, BattlePass, BoxPicks};

impl Store {
    /// `account`'s pass, if it was ever entered.
    pub fn battle_pass(&self, account: AccountId) -> Result<Option<BattlePass>> {
        let row = self
            .conn
            .query_row(
                "SELECT level, period_start, premium, reward_plan, counted, \
                 guides_pick, philosophies_pick, specialties_pick \
                 FROM battle_pass WHERE account_id = ?1",
                [account.0],
                |row| {
                    // Material ids out of range are an error, never another
                    // material.
                    let picks = BoxPicks {
                        guides: row.get::<_, Option<u32>>(5)?,
                        philosophies: row.get::<_, Option<u32>>(6)?,
                        specialties: row.get::<_, Option<u32>>(7)?,
                    };
                    Ok((
                        row.get::<_, u8>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, bool>(2)?,
                        row.get(3)?,
                        row.get::<_, bool>(4)?,
                        picks,
                    ))
                },
            )
            .optional()?;
        let Some((level, period, premium, plan, counted, picks)) = row else {
            return Ok(None);
        };
        // Out of range (a file restored from elsewhere): an error, never
        // another level.
        if level > BATTLE_PASS_LEVELS {
            return Err(rusqlite::Error::IntegralValueOutOfRange(0, i64::from(level)).into());
        }
        let period = DateTime::parse_from_rfc3339(&period).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;
        Ok(Some(BattlePass {
            level,
            period,
            premium,
            plan,
            counted,
            picks,
        }))
    }

    /// Records `account`'s pass, all of it.
    pub fn set_battle_pass(&mut self, account: AccountId, pass: &BattlePass) -> Result<()> {
        self.backup_for_planning()?;
        let now = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
        self.conn.execute(
            "INSERT INTO battle_pass \
             (account_id, level, period_start, premium, reward_plan, counted, \
             guides_pick, philosophies_pick, specialties_pick, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
             ON CONFLICT (account_id) DO UPDATE SET level = excluded.level, \
             period_start = excluded.period_start, premium = excluded.premium, \
             reward_plan = excluded.reward_plan, counted = excluded.counted, \
             guides_pick = excluded.guides_pick, \
             philosophies_pick = excluded.philosophies_pick, \
             specialties_pick = excluded.specialties_pick, updated_at = excluded.updated_at",
            params![
                account.0,
                pass.level.min(BATTLE_PASS_LEVELS),
                pass.period.to_rfc3339_opts(SecondsFormat::Secs, false),
                pass.premium,
                pass.plan,
                pass.counted,
                pass.picks.guides,
                pass.picks.philosophies,
                pass.picks.specialties,
                now,
            ],
        )?;
        Ok(())
    }
}
