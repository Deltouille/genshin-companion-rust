//! Import of paimon.moe saves: the JSON from Settings → Export
//! (`paimon-moe-local-data.json`), or the save file of its Google Drive sync,
//! which has the same content.
//!
//! The save is a flat object of localforage keys. Wish counters are under
//! `[<account>-]wish-counter-<pool>`: the main account has no prefix, the
//! others are `account2`, `account3`… Each counter is
//! `{ total, legendary, rare, pulls: [{ type, id, time, pity, code?, manualInput? }] }`,
//! with pulls oldest first. The account's `wish-uid` and `server` sit under
//! the same prefix.

use std::collections::{BTreeSet, HashMap};

use serde_json::{Map, Value};

use super::{AccountBatch, ImportBatch, ImportSource};
use crate::data::{GameData, Items};
use crate::error::{Error, Result};
use crate::model::{
    GachaType, ItemType, ManualCounters, Pool, Rarity, Server, Source, Wish, WishId, WishTime,
};

const WISH_COUNTER: &str = "wish-counter-";
const UNKNOWN_3_STAR: &str = "unknown_3_star";

/// Reads a paimon.moe save. Nothing is written; see [`ImportBatch`].
///
/// Fails if any item is unknown to `data` rather than guessing its rarity,
/// which would corrupt every pity count after it.
pub fn parse(json: &[u8], data: &GameData) -> Result<ImportBatch> {
    let root: Value = serde_json::from_slice(json)?;
    let Value::Object(save) = root else {
        return Err(Error::NotPaimonExport);
    };

    let mut counters_by_account: HashMap<&str, Vec<(Pool, &str, &Value)>> = HashMap::new();
    for (key, value) in &save {
        if let Some((account, pool)) = split_counter_key(key) {
            counters_by_account
                .entry(account)
                .or_default()
                .push((pool, key, value));
        }
    }
    if counters_by_account.is_empty() {
        return Err(Error::NotPaimonExport);
    }
    let mut accounts: Vec<_> = counters_by_account.into_iter().collect();
    accounts.sort_by_key(|(account, _)| account_order(account));

    let mut batch = ImportBatch::new(ImportSource::PaimonJson);
    let mut unknown_items = BTreeSet::new();
    for (account, mut counters) in accounts {
        counters.sort_by_key(|(pool, _, _)| *pool);
        let setting = |name: &str| save.get(&format!("{}{name}", key_prefix(account)));

        let mut batch_account = AccountBatch {
            label: account_label(account),
            uid: setting("wish-uid").and_then(uid_from),
            server: setting("server")
                .and_then(Value::as_str)
                .and_then(Server::from_paimon),
            ..AccountBatch::default()
        };

        for (pool, key, value) in counters {
            let counter = Counter::parse(key, value)?;
            let mut pool_reader = PoolReader::new(key, pool, &data.items);
            for (index, pull) in counter.pulls.iter().enumerate() {
                if let Some(wish) = pool_reader.read(index, pull, &mut unknown_items)? {
                    batch_account.wishes.push(wish);
                }
            }
            batch
                .warnings
                .extend(pool_reader.warnings(&batch_account.label));

            let pool_wishes: Vec<&Wish> = batch_account
                .wishes
                .iter()
                .filter(|w| w.pool() == pool)
                .collect();
            if let Some(manual) = counter.manual_counters(pool, &pool_wishes) {
                batch_account.manual_counters.push(manual);
            }
        }

        if !batch_account.wishes.is_empty() || !batch_account.manual_counters.is_empty() {
            batch.accounts.push(batch_account);
        }
    }

    if !unknown_items.is_empty() {
        return Err(Error::UnknownItems(unknown_items.into_iter().collect()));
    }
    Ok(batch)
}

/// `account2-wish-counter-standard` → (`account2`, Standard); the main
/// account's keys have no prefix and give `""`.
fn split_counter_key(key: &str) -> Option<(&str, Pool)> {
    let at = key.rfind(WISH_COUNTER)?;
    let pool: Pool = key[at + WISH_COUNTER.len()..].parse().ok()?;
    match &key[..at] {
        "" => Some(("", pool)),
        prefix => prefix
            .strip_suffix('-')
            .filter(|account| !account.is_empty())
            .map(|account| (account, pool)),
    }
}

fn key_prefix(account: &str) -> String {
    if account.is_empty() {
        String::new()
    } else {
        format!("{account}-")
    }
}

/// paimon.moe shows `account3` as "Account 3".
fn account_label(account: &str) -> String {
    if account.is_empty() {
        return "Main".to_owned();
    }
    match account_number(account) {
        Some(number) => format!("Account {number}"),
        None => account.to_owned(),
    }
}

fn account_number(account: &str) -> Option<u64> {
    account.strip_prefix("account")?.parse().ok()
}

/// Main first, then `account2`, `account3`… in numeric order.
fn account_order(account: &str) -> (bool, Option<u64>, String) {
    (
        !account.is_empty(),
        account_number(account),
        account.to_owned(),
    )
}

fn uid_from(value: &Value) -> Option<String> {
    let uid = match value {
        Value::String(s) => s.trim().to_owned(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    (!uid.is_empty()).then_some(uid)
}

/// A non-negative whole number, written as a number or as text.
fn as_count(value: Option<&Value>) -> Option<u32> {
    match value? {
        Value::Number(n) => n
            .as_u64()
            .or_else(|| {
                n.as_f64()
                    .filter(|f| f.fract() == 0.0 && *f >= 0.0)
                    .map(|f| f as u64)
            })
            .and_then(|n| u32::try_from(n).ok()),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// One `wish-counter-*` entry.
struct Counter<'a> {
    total: u32,
    legendary: u32,
    rare: u32,
    pulls: &'a [Value],
}

impl<'a> Counter<'a> {
    fn parse(key: &str, value: &'a Value) -> Result<Self> {
        let invalid = |message: &str| Error::InvalidEntry {
            key: key.to_owned(),
            message: message.to_owned(),
        };
        let counter: &Map<String, Value> = match value {
            Value::Object(counter) => counter,
            Value::String(_) => {
                return Err(invalid(
                    "stored in paimon.moe's old format; open the wish counter on paimon.moe \
                     once so it converts the data, then export again",
                ));
            }
            _ => return Err(invalid("expected an object")),
        };
        let pulls = match counter.get("pulls") {
            None | Some(Value::Null) => &[][..],
            Some(Value::Array(pulls)) => pulls.as_slice(),
            Some(_) => return Err(invalid("`pulls` is not a list")),
        };
        Ok(Self {
            total: as_count(counter.get("total")).unwrap_or(0),
            legendary: as_count(counter.get("legendary")).unwrap_or(0),
            rare: as_count(counter.get("rare")).unwrap_or(0),
            pulls,
        })
    }

    /// The stored counters, when they say something the wish list does not:
    /// paimon.moe's manual mode lets users type them in. When they match what
    /// the list implies, they are derived values and are not kept.
    fn manual_counters(&self, pool: Pool, wishes: &[&Wish]) -> Option<ManualCounters> {
        let since_last = |rarity: Rarity| {
            let count = wishes
                .iter()
                .rev()
                .take_while(|w| w.rarity != rarity)
                .count();
            u32::try_from(count).unwrap_or(u32::MAX)
        };
        let implied = (
            u32::try_from(wishes.len()).unwrap_or(u32::MAX),
            since_last(Rarity::Five),
            since_last(Rarity::Four),
        );
        let stored = (self.total, self.legendary, self.rare);
        (stored != implied).then_some(ManualCounters {
            pool,
            total: self.total,
            pity5: self.legendary,
            pity4: self.rare,
        })
    }
}

/// Converts the pulls of one counter, in order.
struct PoolReader<'a> {
    key: &'a str,
    pool: Pool,
    items: &'a Items,
    /// Next `seq` for each time already seen in this pool.
    seqs: HashMap<WishTime, u32>,
    previous: Option<WishTime>,
    out_of_order: u32,
    foreign_codes: u32,
}

impl<'a> PoolReader<'a> {
    fn new(key: &'a str, pool: Pool, items: &'a Items) -> Self {
        Self {
            key,
            pool,
            items,
            seqs: HashMap::new(),
            previous: None,
            out_of_order: 0,
            foreign_codes: 0,
        }
    }

    /// `Ok(None)` when the item is unknown; it is recorded in `unknown_items`
    /// so that one error can list all of them.
    fn read(
        &mut self,
        index: usize,
        pull: &Value,
        unknown_items: &mut BTreeSet<String>,
    ) -> Result<Option<Wish>> {
        let invalid = |message: String| Error::InvalidPull {
            key: self.key.to_owned(),
            index,
            message,
        };
        let Value::Object(pull) = pull else {
            return Err(invalid("not an object".to_owned()));
        };
        let field = |name: &str| pull.get(name).and_then(Value::as_str);

        let time = match pull.get("time") {
            Some(Value::String(time)) => time
                .parse::<WishTime>()
                .map_err(|e| invalid(e.to_string()))?,
            Some(Value::Number(_)) => {
                return Err(invalid(
                    "time is in paimon.moe's old numeric format; open the wish counter on \
                     paimon.moe once so it converts the data, then export again"
                        .to_owned(),
                ));
            }
            _ => return Err(invalid("missing `time`".to_owned())),
        };
        if self.previous.is_some_and(|previous| time < previous) {
            self.out_of_order += 1;
        }
        self.previous = Some(time);

        // Counted for every pull, so ids only depend on the pull's position.
        let next_seq = self.seqs.entry(time).or_insert(0);
        let seq = *next_seq;
        *next_seq += 1;

        let gacha_type = self.gacha_type(pull.get("code"));
        let manual = pull.get("manualInput").and_then(Value::as_bool) == Some(true);
        let manual_pity = if manual {
            as_count(pull.get("pity"))
        } else {
            None
        };

        let kind = field("type").ok_or_else(|| invalid("missing `type`".to_owned()))?;
        let (name, item_type, rarity, item_key) = match kind {
            UNKNOWN_3_STAR => ("Unknown".to_owned(), ItemType::Unknown, Rarity::Three, None),
            "character" | "weapon" => {
                let expected = if kind == "character" {
                    ItemType::Character
                } else {
                    ItemType::Weapon
                };
                let key = field("id").ok_or_else(|| invalid("missing `id`".to_owned()))?;
                match self
                    .items
                    .by_key(key)
                    .filter(|item| item.item_type == expected)
                {
                    Some(item) => (
                        item.name.clone(),
                        item.item_type,
                        item.rarity,
                        Some(item.key.clone()),
                    ),
                    None => {
                        unknown_items.insert(format!("{kind} `{key}`"));
                        return Ok(None);
                    }
                }
            }
            other => return Err(invalid(format!("unknown type `{other}`"))),
        };

        Ok(Some(Wish {
            id: WishId::paimon(self.pool, time, seq),
            gacha_type,
            time,
            seq,
            name,
            item_type,
            rarity,
            item_key,
            manual_pity,
            source: Source::Paimon,
        }))
    }

    /// The pull's own gacha type (`code`), when it belongs to this pool.
    /// paimon.moe records it for imported pulls (301 or 400 on character banners).
    fn gacha_type(&mut self, code: Option<&Value>) -> GachaType {
        let code = match code {
            Some(Value::String(code)) => code.parse::<GachaType>().ok(),
            Some(Value::Number(code)) => code.to_string().parse::<GachaType>().ok(),
            _ => None,
        };
        match code {
            Some(code) if code.pool() == self.pool => code,
            Some(_) => {
                self.foreign_codes += 1;
                self.pool.default_gacha_type()
            }
            None => self.pool.default_gacha_type(),
        }
    }

    fn warnings(&self, account: &str) -> Vec<String> {
        let pool = self.pool.display_name();
        let mut warnings = Vec::new();
        if self.out_of_order > 0 {
            warnings.push(format!(
                "{account}, {pool}: {} wish(es) are not in time order in paimon.moe's data; \
                 they are sorted by time here, which can change their pity",
                self.out_of_order,
            ));
        }
        if self.foreign_codes > 0 {
            warnings.push(format!(
                "{account}, {pool}: {} wish(es) carried another banner's gacha type; \
                 recorded as {}",
                self.foreign_codes,
                self.pool.default_gacha_type(),
            ));
        }
        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_keys_split_into_account_and_pool() {
        assert_eq!(
            split_counter_key("wish-counter-character-event"),
            Some(("", Pool::CharacterEvent))
        );
        assert_eq!(
            split_counter_key("account12-wish-counter-standard"),
            Some(("account12", Pool::Standard))
        );
        assert_eq!(split_counter_key("wish-counter-setting"), None);
        assert_eq!(split_counter_key("-wish-counter-standard"), None);
        assert_eq!(split_counter_key("todos"), None);
    }

    #[test]
    fn accounts_get_paimon_labels_and_order() {
        assert_eq!(account_label(""), "Main");
        assert_eq!(account_label("account3"), "Account 3");
        let mut accounts = vec!["account10", "", "account2"];
        accounts.sort_by_key(|a| account_order(a));
        assert_eq!(accounts, ["", "account2", "account10"]);
    }

    #[test]
    fn counts_accept_numbers_and_text() {
        assert_eq!(as_count(Some(&Value::from(7))), Some(7));
        assert_eq!(as_count(Some(&Value::from(7.0))), Some(7));
        assert_eq!(as_count(Some(&Value::from("12"))), Some(12));
        assert_eq!(as_count(Some(&Value::from(-1))), None);
        assert_eq!(as_count(None), None);
    }
}
