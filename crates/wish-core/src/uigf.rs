//! UIGF, the format other wish trackers exchange (Snap Hutao, Starward,
//! Genshin Wish Export…): <https://uigf.org/en/standards/uigf.html>.
//!
//! Export writes UIGF v4. UIGF needs a numeric game id for every wish;
//! migrated paimon.moe wishes without a recorded game id get a made-up one,
//! `9` followed by the wish's Unix time and a counter. Real game ids start
//! with their Unix time (a `1` until 2286), so a made-up id is always
//! recognisable, and importing it back never passes it off as a game record.
//!
//! Import reads UIGF v4 and v3 (Genshin only).

use std::collections::HashMap;

use chrono::{Local, TimeDelta};
use serde_json::{Value, json};

use crate::data::GameData;
use crate::engine::DEFAULT_SERVER;
use crate::error::{Error, Result};
use crate::import::api::ApiRow;
use crate::model::{GachaType, ItemType, Server, Wish, WishId};
use crate::store::Store;

pub const VERSION: &str = "v4.0";

/// Whether a UIGF id was made up (by this app) rather than given by the game.
pub fn is_made_up_id(id: &str) -> bool {
    id.len() == 19 && id.starts_with('9') && id.bytes().all(|b| b.is_ascii_digit())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UigfExport {
    pub json: Vec<u8>,
    pub accounts: usize,
    pub wishes: usize,
    /// Accounts left out: UIGF needs a UID.
    pub without_uid: Vec<String>,
    /// Wishes exported with a made-up id.
    pub made_up_ids: usize,
    /// Wishes without a HoYoverse item id (unknown items, hand-counted 3★).
    pub without_item_id: usize,
}

/// Every account with a UID, as UIGF v4.
pub fn export(store: &Store, data: &GameData) -> Result<UigfExport> {
    let mut report = UigfExport {
        json: Vec::new(),
        accounts: 0,
        wishes: 0,
        without_uid: Vec::new(),
        made_up_ids: 0,
        without_item_id: 0,
    };
    let mut accounts = Vec::new();
    for account in store.accounts()? {
        let Some(uid) = account.uid.clone() else {
            report.without_uid.push(account.label);
            continue;
        };
        let offset = account.server.unwrap_or(DEFAULT_SERVER).utc_offset_hours();
        let links: HashMap<WishId, WishId> = store
            .links(account.id)?
            .into_iter()
            .map(|link| (link.wish_id, link.api_id))
            .collect();
        let mut made_up_per_second: HashMap<i64, u32> = HashMap::new();

        let mut list = Vec::new();
        for wish in store.wishes(account.id)? {
            let id = if wish.id.is_api() {
                wish.id.to_string()
            } else if let Some(api_id) = links.get(&wish.id) {
                api_id.to_string()
            } else {
                report.made_up_ids += 1;
                let utc = wish.time.naive() - TimeDelta::hours(i64::from(offset));
                let seconds = utc.and_utc().timestamp();
                let n = made_up_per_second.entry(seconds).or_insert(0);
                *n += 1;
                format!("9{seconds:010}{n:08}")
            };
            let item_id = item_id(&wish, data);
            if item_id.is_none() {
                report.without_item_id += 1;
            }
            list.push(record(&wish, &id, item_id));
        }
        report.wishes += list.len();
        report.accounts += 1;
        accounts.push(json!({
            "uid": uid,
            "timezone": offset,
            "lang": "en-us",
            "list": list,
        }));
    }

    let document = json!({
        "info": {
            "export_timestamp": Local::now().timestamp(),
            "export_app": "Genshin Companion",
            "export_app_version": env!("CARGO_PKG_VERSION"),
            "version": VERSION,
        },
        "hk4e": accounts,
    });
    report.json = serde_json::to_vec_pretty(&document).expect("UIGF documents always serialize");
    report.json.push(b'\n');
    Ok(report)
}

fn item_id(wish: &Wish, data: &GameData) -> Option<u32> {
    let item = match &wish.item_key {
        Some(key) => data.items.by_key(key),
        None => data.items.by_name(&wish.name),
    };
    item.and_then(|item| item.item_id)
}

fn record(wish: &Wish, id: &str, item_id: Option<u32>) -> Value {
    let uigf_gacha_type = match wish.gacha_type {
        GachaType::CharacterEvent2 => GachaType::CharacterEvent,
        other => other,
    };
    let (name, item_type) = match wish.item_type {
        // A 3★ counted by hand in paimon.moe has no item.
        ItemType::Unknown => ("", ""),
        known => (wish.name.as_str(), known.as_str()),
    };
    json!({
        "uigf_gacha_type": uigf_gacha_type.as_str(),
        "gacha_type": wish.gacha_type.as_str(),
        "item_id": item_id.map(|id| id.to_string()).unwrap_or_default(),
        "count": "1",
        "time": wish.time.to_string(),
        "name": name,
        "item_type": item_type,
        "rank_type": wish.rarity.stars().to_string(),
        "id": id,
    })
}

/// One account of a UIGF file, as the game's API would return it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UigfAccount {
    pub uid: String,
    pub server: Option<Server>,
    pub rows: Vec<ApiRow>,
}

/// Whether a JSON document looks like UIGF.
pub fn is_uigf(root: &Value) -> bool {
    root.get("info").is_some_and(Value::is_object)
        && (root.get("hk4e").is_some_and(Value::is_array)
            || root.get("list").is_some_and(Value::is_array))
}

/// Reads the Genshin accounts of a UIGF v4 (or v3) document. Records without
/// a name are completed from their item id.
pub fn parse(root: &Value, data: &GameData) -> Result<Vec<UigfAccount>> {
    let invalid = |message: String| Error::InvalidEntry {
        key: "UIGF".to_owned(),
        message,
    };
    let version = root["info"]["version"]
        .as_str()
        .or_else(|| root["info"]["uigf_version"].as_str())
        .unwrap_or_default();

    // v4: { info, hk4e: [{ uid, timezone, list }] }; v3: { info: { uid, region_time_zone }, list }.
    let accounts: Vec<(Value, Option<i64>, &Vec<Value>)> =
        match root.get("hk4e").and_then(Value::as_array) {
            Some(accounts) => accounts
                .iter()
                .map(|account| {
                    let list = account["list"]
                        .as_array()
                        .ok_or_else(|| invalid("an account has no `list`".to_owned()))?;
                    Ok((account["uid"].clone(), account["timezone"].as_i64(), list))
                })
                .collect::<Result<_>>()?,
            None if version.starts_with("v3") || version.starts_with("v2") => {
                let list = root["list"]
                    .as_array()
                    .ok_or_else(|| invalid("no `list`".to_owned()))?;
                vec![(
                    root["info"]["uid"].clone(),
                    root["info"]["region_time_zone"].as_i64(),
                    list,
                )]
            }
            None => {
                return Err(invalid(format!(
                    "UIGF version `{version}` is not supported"
                )));
            }
        };

    let mut unknown = Vec::new();
    let mut parsed = Vec::new();
    for (uid, timezone, list) in accounts {
        let uid = match uid {
            Value::String(uid) => uid,
            Value::Number(uid) => uid.to_string(),
            _ => return Err(invalid("an account has no `uid`".to_owned())),
        };
        let server = match timezone {
            Some(-5) => Some(Server::America),
            Some(1) => Some(Server::Europe),
            // Asia, TW/HK/MO and China share UTC+8, which is what banner times need.
            Some(8) => Some(Server::Asia),
            _ => None,
        };
        let mut rows = Vec::with_capacity(list.len());
        for record in list {
            let text = |field: &str| match &record[field] {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            };
            let item = text("item_id")
                .parse::<u32>()
                .ok()
                .and_then(|id| data.items.by_item_id(id))
                .or_else(|| data.items.by_name(&text("name")));
            let name = Some(text("name"))
                .filter(|n| !n.is_empty())
                .or_else(|| item.map(|i| i.name.clone()));
            let rank = Some(text("rank_type"))
                .filter(|r| !r.is_empty())
                .or_else(|| item.map(|i| i.rarity.stars().to_string()));
            let (Some(name), Some(rank_type)) = (name, rank) else {
                unknown.push(format!("item {}", text("item_id")));
                continue;
            };
            let item_type = Some(text("item_type"))
                .filter(|t| !t.is_empty())
                .or_else(|| item.map(|i| i.item_type.as_str().to_owned()))
                .unwrap_or_default();
            rows.push(ApiRow {
                uid: uid.clone(),
                gacha_type: text("gacha_type"),
                time: text("time"),
                name,
                item_type,
                rank_type,
                id: text("id"),
            });
        }
        // Newest first, like the game's API.
        rows.reverse();
        parsed.push(UigfAccount { uid, server, rows });
    }
    if !unknown.is_empty() {
        unknown.sort();
        unknown.dedup();
        return Err(Error::UnknownItems(unknown));
    }
    Ok(parsed)
}
