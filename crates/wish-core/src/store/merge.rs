//! Merging what a snapshot carries besides the wishes (an account's
//! characters, weapons, plan and inventory) into an account, inside an
//! import's transaction. Nothing stored is lowered or replaced:
//!
//! - a character is added, or raised to the higher level, constellation and
//!   talents (a favourite stays one);
//! - a weapon copy is matched to a stored copy of the same weapon (the same
//!   holder first), each stored copy once, and raised like a character; a
//!   copy left over is added. A holder who already holds a copy keeps it;
//! - a plan entry is added at the end unless the plan has one for the same
//!   character, weapon or custom name;
//! - a material is counted only when the account has none: the counts it
//!   has are newer than a snapshot's.
//!
//! So importing the same snapshot twice changes nothing the second time.

use std::collections::HashSet;

use rusqlite::{Transaction, params};

use super::collection::{read_collection, save_character, write_weapon};
use super::inventory::{read_inventory, set_owned};
use super::plan::{insert_entry, read_plan};
use crate::error::Result;
use crate::import::{AccountExtras, ExtrasReport};
use crate::model::AccountId;
use crate::planner::{CustomItem, PlanTarget};

/// Merges a snapshot account's characters, weapons, plan and inventory into
/// `account`, inside the import's transaction: only adds or raises, never
/// lowers or removes.
pub(super) fn merge_extras(
    tx: &Transaction,
    account: AccountId,
    extras: &AccountExtras,
    now: &str,
) -> Result<ExtrasReport> {
    let mut report = ExtrasReport::default();
    if extras.is_empty() {
        return Ok(report);
    }
    let current = read_collection(tx, account)?;

    for incoming in &extras.characters {
        let incoming = incoming.clone().clamped();
        match current.character(&incoming.key) {
            None => {
                save_character(tx, account, &incoming, now)?;
                report.characters_added += 1;
            }
            Some(before) => {
                let mut after = before.clone();
                after.level = after.level.max(incoming.level);
                after.constellation = after.constellation.max(incoming.constellation);
                for (have, read) in after.talents.iter_mut().zip(incoming.talents) {
                    *have = (*have).max(read);
                }
                after.favourite |= incoming.favourite;
                if after != *before {
                    save_character(tx, account, &after, now)?;
                    report.characters_raised += 1;
                }
            }
        }
    }

    // Each stored copy matches one incoming copy at most.
    let mut claimed: HashSet<i64> = HashSet::new();
    let mut holders: HashSet<String> = (current.weapons.iter())
        .filter_map(|w| w.holder.clone())
        .collect();
    for incoming in &extras.weapons {
        let incoming = incoming.clone().clamped();
        let candidates = || {
            (current.weapons.iter()).filter(|w| w.key == incoming.key && !claimed.contains(&w.id))
        };
        let matched = candidates()
            .find(|w| w.holder == incoming.holder)
            .or_else(|| candidates().next());
        let holder_free =
            |holder: &Option<String>| holder.as_ref().is_some_and(|h| !holders.contains(h));
        match matched {
            Some(before) => {
                claimed.insert(before.id);
                let mut after = before.clone();
                after.level = after.level.max(incoming.level);
                after.refinement = after.refinement.max(incoming.refinement);
                after.favourite |= incoming.favourite;
                if after.holder.is_none() && holder_free(&incoming.holder) {
                    after.holder = incoming.holder.clone();
                }
                if after != *before {
                    if let Some(holder) = &after.holder {
                        holders.insert(holder.clone());
                    }
                    write_weapon(tx, account, before.id, &after, now)?;
                    report.weapons_raised += 1;
                }
            }
            None => {
                let mut copy = incoming.clone();
                if !holder_free(&copy.holder) {
                    copy.holder = None;
                }
                tx.execute(
                    "INSERT INTO owned_weapons (account_id, weapon_key, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?3)",
                    params![account.0, copy.key, now],
                )?;
                let id = tx.last_insert_rowid();
                if let Some(holder) = &copy.holder {
                    holders.insert(holder.clone());
                }
                write_weapon(tx, account, id, &copy, now)?;
                report.weapons_added += 1;
            }
        }
    }

    let mut planned: Vec<PlanKey> = (read_plan(tx, account)?.iter())
        .map(|entry| PlanKey::of(&entry.target))
        .collect();
    if extras
        .plan
        .iter()
        .any(|t| !planned.contains(&PlanKey::of(t)))
    {
        let mut position: i64 = tx.query_row(
            "SELECT coalesce(max(position), -1) + 1 FROM plan_entries WHERE account_id = ?1",
            [account.0],
            |row| row.get(0),
        )?;
        for target in &extras.plan {
            let key = PlanKey::of(target);
            if planned.contains(&key) {
                continue;
            }
            insert_entry(tx, account, position, now, target)?;
            planned.push(key);
            position += 1;
            report.plan_added += 1;
        }
    }

    let have = read_inventory(tx, account)?;
    let incoming = (extras.inventory.materials.iter())
        .map(|(id, count)| (CustomItem::Material(*id), *count))
        .chain(
            (extras.inventory.named.iter())
                .map(|(name, count)| (CustomItem::Named(name.clone()), *count)),
        );
    for (item, count) in incoming {
        if count > 0 && have.owned(&item) == 0 {
            set_owned(tx, account, &item, count, now)?;
            report.inventory_added += 1;
        }
    }
    Ok(report)
}

/// What a plan entry is about, whatever its goal: one per character, weapon
/// or custom name.
#[derive(PartialEq, Eq)]
enum PlanKey {
    Character(String),
    Weapon(String),
    Custom(String),
}

impl PlanKey {
    fn of(target: &PlanTarget) -> Self {
        match target {
            PlanTarget::Character { key, .. } => PlanKey::Character(key.clone()),
            PlanTarget::Weapon { key, .. } => PlanKey::Weapon(key.clone()),
            PlanTarget::CustomCharacter { name, .. }
            | PlanTarget::CustomWeapon { name, .. }
            | PlanTarget::Custom { name, .. } => PlanKey::Custom(name.trim().to_lowercase()),
        }
    }
}
