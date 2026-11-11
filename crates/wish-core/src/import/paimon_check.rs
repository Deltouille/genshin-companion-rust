//! Compares the engine with the numbers paimon.moe stored in a save.
//!
//! A save holds what paimon.moe computed at its last import: each pull's
//! `pity` and `rate`, and each counter's `total`, `legendary`, `rare`,
//! `guaranteed` and `rateoff`. Its counter cards and summary show exactly
//! those values, so no difference means this app shows the same numbers.

use std::collections::HashMap;

use serde_json::{Map, Value};

use super::AccountBatch;
use super::paimon;
use crate::data::GameData;
use crate::engine::{self, Counters, Guarantee, PoolAnalysis, Pull, RateUpOutcome};
use crate::error::{Error, Result};
use crate::model::{Rarity, WishId};
use crate::stats::RateStats;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Comparison {
    /// How many stored values were compared.
    pub checked: u32,
    /// One line per value that differs.
    pub differences: Vec<String>,
}

impl Comparison {
    pub fn matches(&self) -> bool {
        self.differences.is_empty()
    }

    fn check(&mut self, ok: bool, key: &str, what: impl FnOnce() -> String) {
        self.checked += 1;
        if !ok {
            self.differences.push(format!("{key}: {}", what()));
        }
    }
}

pub fn compare(json: &[u8], data: &GameData) -> Result<Comparison> {
    let save: Map<String, Value> = match serde_json::from_slice(json)? {
        Value::Object(save) => save,
        _ => return Err(Error::NotPaimonExport),
    };
    let batch = paimon::parse(json, data)?;
    let mut comparison = Comparison::default();

    for account in &batch.accounts {
        let prefix = key_prefix(&account.label);
        let analysis = engine::analyze(
            account.wishes.clone(),
            account.server,
            &account.manual_counters,
            data,
        );
        // The importer keeps stored counters that differ from the pull list
        // as manual counters, which would hide a counting bug here: compare
        // with the derived ones unless the user really counted by hand.
        let derived = engine::analyze(account.wishes.clone(), account.server, &[], data);
        let manual_mode = [String::new(), prefix.clone()].iter().any(|p| {
            save.get(&format!("{p}wish-counter-setting"))
                .is_some_and(|s| s["manualInput"] == true)
        });

        for pool in &analysis.pools {
            let key = format!("{prefix}wish-counter-{}", pool.pool);
            let Some(stored) = save.get(&key) else {
                continue;
            };
            compare_pulls(&mut comparison, &key, account, pool, stored);

            let manual = manual_mode || pool.pulls.iter().any(|p| p.wish.manual_pity.is_some());
            let counters = match derived.pool(pool.pool) {
                Some(derived) if !manual => derived.counters,
                _ => pool.counters,
            };
            compare_counter(&mut comparison, &key, pool, counters, stored);
        }
    }
    Ok(comparison)
}

/// The importer keeps the save's order, so the n-th stored pull is the n-th
/// wish of that pool in the batch.
fn compare_pulls(
    comparison: &mut Comparison,
    key: &str,
    account: &AccountBatch,
    pool: &PoolAnalysis,
    stored: &Value,
) {
    let by_id: HashMap<&WishId, &Pull> = pool.pulls.iter().map(|p| (&p.wish.id, p)).collect();
    let wishes = account.wishes.iter().filter(|w| w.pool() == pool.pool);
    let stored_pulls = stored["pulls"].as_array().map_or(&[][..], Vec::as_slice);

    for (raw, wish) in stored_pulls.iter().zip(wishes) {
        let Some(pull) = by_id.get(&wish.id) else {
            continue;
        };
        let at = || format!("{} {}", wish.time, wish.name);
        // paimon.moe stores 1 as the pity of every 3★.
        if wish.rarity != Rarity::Three
            && let Some(pity) = raw["pity"].as_u64()
        {
            let ours = pull.pity.map(u64::from);
            comparison.check(ours == Some(pity), key, || {
                format!("{}: pity {ours:?}, paimon.moe {pity}", at())
            });
        }
        if let Some(rate) = raw["rate"].as_u64() {
            let ours = pull.rate_up.map(|r| u64::from(r.paimon_rate()));
            comparison.check(ours == Some(rate), key, || {
                format!("{}: rate {ours:?}, paimon.moe {rate}", at())
            });
        }
    }
}

fn compare_counter(
    comparison: &mut Comparison,
    key: &str,
    pool: &PoolAnalysis,
    counters: Counters,
    stored: &Value,
) {
    let theirs = (
        stored["total"].as_u64(),
        stored["legendary"].as_u64(),
        stored["rare"].as_u64(),
    );
    let ours = (
        Some(u64::from(counters.total)),
        Some(u64::from(counters.pity5)),
        Some(u64::from(counters.pity4)),
    );
    comparison.check(ours == theirs, key, || {
        format!("counters (total, 5★, 4★) {ours:?}, paimon.moe {theirs:?}")
    });

    if let Some(guaranteed) = stored.get("guaranteed") {
        let ours = pool.guarantee.unwrap_or(Guarantee {
            five: false,
            four: false,
        });
        for (rarity, field, ours) in [
            (Rarity::Five, "legendary", ours.five),
            (Rarity::Four, "rare", ours.four),
        ] {
            // paimon.moe derives the flag from the last `rate` being 0, which
            // only differs from the game rule after an impossible outcome.
            if last_outcome(pool, rarity) == Some(RateUpOutcome::LostWhileGuaranteed) {
                continue;
            }
            if let Some(theirs) = guaranteed[field].as_bool() {
                comparison.check(theirs == ours, key, || {
                    format!("{field} guaranteed {ours}, paimon.moe {theirs}")
                });
            }
        }
    }

    if let Some(rateoff) = stored.get("rateoff") {
        for (field, ours) in [
            ("legendary", pool.stats.rate_five),
            ("rare", pool.stats.rate_four),
        ] {
            let theirs = &rateoff[field];
            let theirs = RateStats {
                won: count(&theirs["win"]),
                lost: count(&theirs["lose"]),
                longest_win_streak: count(&theirs["maxStreak"]),
                current_win_streak: count(&theirs["currentStreak"]),
            };
            comparison.check(ours == Some(theirs), key, || {
                format!("{field} rate-up record {ours:?}, paimon.moe {theirs:?}")
            });
        }
    }
}

fn last_outcome(pool: &PoolAnalysis, rarity: Rarity) -> Option<RateUpOutcome> {
    pool.pulls
        .iter()
        .rev()
        .find(|p| p.wish.rarity == rarity)
        .and_then(|p| p.rate_up)
        .map(|r| r.outcome())
}

fn count(value: &Value) -> u32 {
    value
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(0)
}

/// Inverse of the importer's labels: "Main" → "", "Account 3" → "account3-".
fn key_prefix(label: &str) -> String {
    match label.strip_prefix("Account ") {
        Some(number) => format!("account{number}-"),
        None if label == "Main" => String::new(),
        None => format!("{label}-"),
    }
}
