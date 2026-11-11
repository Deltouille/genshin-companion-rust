//! Turns a fetched history into an import batch, against what is stored.
//!
//! For each fetched wish, per pool:
//! - already known (its id is stored, or recorded as a link): skipped;
//! - newer than everything stored: new;
//! - otherwise it should already be stored as a migrated paimon.moe wish at
//!   the same time and with the same name. If so, its game id is recorded as
//!   a link. If not, it is a gap in the migrated history and is added, unless
//!   the pool holds wishes typed by hand in paimon.moe (their times are made
//!   up, so the game's record of the same wish would not match and would
//!   count twice): then it is held back and reported.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::api::{ApiRow, FetchedHistory};
use super::{AccountBatch, ImportBatch, ImportSource, WishLink};
use crate::data::GameData;
use crate::error::{Error, Result};
use crate::model::{
    Account, GachaType, ItemType, Pool, Rarity, Server, Source, Wish, WishId, WishTime,
};
use crate::store::Store;

/// Where a fetched history can go, with what each choice would add.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiPlan {
    pub uid: Option<String>,
    pub server: Option<Server>,
    /// The account with this UID; or, when there is none, a new account
    /// first, then each account without a UID yet.
    pub targets: Vec<TargetPlan>,
    /// Index into `targets` of the suggested choice.
    pub suggested: usize,
}

impl ApiPlan {
    /// The game returned no wish at all.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Existing(Account),
    New,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetPlan {
    pub target: Target,
    pub batch: ImportBatch,
    pub new_wishes: BTreeMap<Pool, u32>,
    pub linked: u32,
    pub held_back: BTreeMap<Pool, u32>,
}

impl TargetPlan {
    pub fn new_total(&self) -> u32 {
        self.new_wishes.values().sum()
    }

    pub fn held_back_total(&self) -> u32 {
        self.held_back.values().sum()
    }
}

pub fn plan(fetched: FetchedHistory, store: &Store, data: &GameData) -> Result<ApiPlan> {
    let server = fetched.region.as_deref().and_then(Server::from_api_region);
    let Some(uid) = fetched.uid.clone() else {
        return Ok(ApiPlan {
            uid: None,
            server,
            targets: Vec::new(),
            suggested: 0,
        });
    };

    let mut warnings = Vec::new();
    let fetched_wishes = to_wishes(&fetched.rows, data, &mut warnings)?;
    let accounts = store.accounts()?;

    let candidates: Vec<Option<Account>> =
        match accounts.iter().find(|a| a.uid.as_deref() == Some(&uid)) {
            Some(account) => vec![Some(account.clone())],
            None => std::iter::once(None)
                .chain(
                    accounts
                        .iter()
                        .filter(|a| a.uid.is_none())
                        .cloned()
                        .map(Some),
                )
                .collect(),
        };

    let mut targets = Vec::with_capacity(candidates.len());
    for account in candidates {
        let (stored, links) = match &account {
            Some(account) => (store.wishes(account.id)?, store.links(account.id)?),
            None => (Vec::new(), Vec::new()),
        };
        targets.push(plan_target(
            account,
            &uid,
            server,
            &fetched_wishes,
            &stored,
            &links,
            &warnings,
        ));
    }

    // Suggest the account whose history the game's records match best;
    // without any match, a new account.
    let suggested = targets
        .iter()
        .enumerate()
        .filter(|(_, t)| t.linked > 0)
        .max_by_key(|(_, t)| t.linked)
        .map_or(0, |(ix, _)| ix);

    Ok(ApiPlan {
        uid: Some(uid),
        server,
        targets,
        suggested,
    })
}

fn plan_target(
    account: Option<Account>,
    uid: &str,
    server: Option<Server>,
    fetched: &[Wish],
    stored: &[Wish],
    links: &[WishLink],
    warnings: &[String],
) -> TargetPlan {
    let linked_ids: HashSet<&WishId> = links.iter().map(|l| &l.wish_id).collect();
    // Every stored id: game ids, and made-up ids of an earlier UIGF import.
    let known: HashSet<&str> = stored
        .iter()
        .map(|w| w.id.as_str())
        .chain(links.iter().map(|l| l.api_id.as_str()))
        .collect();

    let mut newest: HashMap<Pool, WishTime> = HashMap::new();
    let mut newest_manual: HashMap<Pool, WishTime> = HashMap::new();
    // Migrated wishes still waiting for their game id, per pool and time.
    let mut unlinked: HashMap<(Pool, WishTime), Vec<&Wish>> = HashMap::new();
    for wish in stored {
        let pool = wish.pool();
        newest
            .entry(pool)
            .and_modify(|t| *t = (*t).max(wish.time))
            .or_insert(wish.time);
        if wish.manual_pity.is_some() || wish.item_type == ItemType::Unknown {
            newest_manual
                .entry(pool)
                .and_modify(|t| *t = (*t).max(wish.time))
                .or_insert(wish.time);
        }
        if !wish.id.is_api() && !linked_ids.contains(&wish.id) {
            unlinked.entry((pool, wish.time)).or_default().push(wish);
        }
    }

    let mut new = Vec::new();
    let mut new_wishes: BTreeMap<Pool, u32> = BTreeMap::new();
    let mut held_back: BTreeMap<Pool, u32> = BTreeMap::new();
    let mut found_links = Vec::new();
    for wish in fetched {
        if known.contains(wish.id.as_str()) {
            continue;
        }
        let pool = wish.pool();
        let covered = newest.get(&pool).is_some_and(|n| wish.time <= *n);
        if covered {
            if let Some(group) = unlinked.get_mut(&(pool, wish.time))
                && let Some(at) = group.iter().position(|w| w.name == wish.name)
            {
                let stored = group.remove(at);
                // Only a real game id is worth recording; a made-up one
                // (UIGF) just tells the wish is already stored.
                if wish.id.is_api() {
                    found_links.push(WishLink {
                        wish_id: stored.id.clone(),
                        api_id: wish.id.clone(),
                    });
                }
                continue;
            }
            if newest_manual.get(&pool).is_some_and(|m| wish.time <= *m) {
                *held_back.entry(pool).or_default() += 1;
                continue;
            }
        }
        *new_wishes.entry(pool).or_default() += 1;
        new.push(wish.clone());
    }

    let mut warnings = warnings.to_vec();
    for (pool, count) in &held_back {
        warnings.push(format!(
            "{count} older {} from {} {} not added: this banner’s history has wishes typed by hand \
             in paimon.moe, which they could repeat.",
            if *count == 1 { "wish" } else { "wishes" },
            pool.display_name(),
            if *count == 1 { "was" } else { "were" },
        ));
    }

    let mut batch = ImportBatch::new(ImportSource::Api);
    batch.detail = Some(format!("UID {uid}"));
    batch.warnings = warnings;
    batch.accounts.push(AccountBatch {
        label: account
            .as_ref()
            .map_or_else(|| uid.to_owned(), |a| a.label.clone()),
        uid: Some(uid.to_owned()),
        server,
        wishes: new,
        links: found_links.clone(),
        merge_into: account.as_ref().map(|a| a.id),
        ..AccountBatch::default()
    });

    TargetPlan {
        target: account.map_or(Target::New, Target::Existing),
        batch,
        new_wishes,
        linked: u32::try_from(found_links.len()).unwrap_or(u32::MAX),
        held_back,
    }
}

/// Converts API rows, oldest first. Rows of an unknown banner type are left
/// out and reported; an unknown item never stops the import.
fn to_wishes(rows: &[ApiRow], data: &GameData, warnings: &mut Vec<String>) -> Result<Vec<Wish>> {
    let mut unknown_types: BTreeMap<&str, u32> = BTreeMap::new();
    let mut wishes = Vec::with_capacity(rows.len());
    for row in rows {
        let Ok(gacha_type) = row.gacha_type.parse::<GachaType>() else {
            *unknown_types.entry(&row.gacha_type).or_default() += 1;
            continue;
        };
        let invalid = |what: &str, value: &str| {
            Error::UnexpectedResponse(format!("wish {}: {what} `{value}`", row.id))
        };
        let (id, source) = if crate::uigf::is_made_up_id(&row.id) {
            (WishId::made_up(&row.id), Source::Paimon)
        } else {
            let id = WishId::from_api(&row.id).ok_or_else(|| invalid("id", &row.id))?;
            (id, Source::Api)
        };
        let time: WishTime = row.time.parse().map_err(|_| invalid("time", &row.time))?;
        let rarity = row
            .rank_type
            .parse::<u8>()
            .ok()
            .and_then(Rarity::from_stars)
            .ok_or_else(|| invalid("rank", &row.rank_type))?;
        let item_type = match row.item_type.as_str() {
            "Character" => ItemType::Character,
            "Weapon" => ItemType::Weapon,
            _ => ItemType::Unknown,
        };
        wishes.push(Wish {
            id,
            gacha_type,
            time,
            seq: 0,
            item_key: data.items.by_name(&row.name).map(|item| item.key.clone()),
            name: row.name.clone(),
            item_type,
            rarity,
            manual_pity: None,
            source,
        });
    }
    for (code, count) in unknown_types {
        warnings.push(format!(
            "{count} wish(es) from an unknown banner type ({code}) were left out; update the app."
        ));
    }

    // Oldest first; ids grow with time, and order wishes made together.
    let numeric = |w: &Wish| (w.id.as_str().len(), w.id.as_str().to_owned());
    wishes.sort_by(|a, b| {
        a.time
            .cmp(&b.time)
            .then_with(|| numeric(a).cmp(&numeric(b)))
    });
    let mut seqs: HashMap<(Pool, WishTime), u32> = HashMap::new();
    for wish in &mut wishes {
        let next = seqs.entry((wish.pool(), wish.time)).or_insert(0);
        wish.seq = *next;
        *next += 1;
    }
    Ok(wishes)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use super::super::api::tests::{FakeApi, NO_WAIT, row};
    use super::super::api::{GachaLink, fetch};
    use super::*;
    use crate::AppPaths;
    use crate::import::paimon;

    const SAMPLE: &str = include_str!("../../tests/fixtures/paimon-sample.json");
    const LINK: &str = "https://webstatic-sea.hoyoverse.com/index.html?authkey=abc#/log";

    /// The sample's "Main" character history as the game would return it,
    /// plus a wish missing from it (a gap) and two newer ones. Newest first.
    fn main_character_history() -> Vec<ApiRow> {
        let mut rows = vec![
            row(3002, "301", "2024-02-01 12:00:01", "Diluc", "5"),
            row(3001, "301", "2024-02-01 12:00:00", "Slingshot", "3"),
            row(2014, "301", "2024-01-02 09:01:00", "Slingshot", "3"),
            row(2013, "400", "2024-01-02 09:00:00", "Venti", "5"),
            row(2012, "301", "2024-01-01 12:00:00", "Cool Steel", "3"),
        ];
        let ten_pull = [
            "Debate Club",
            "Debate Club",
            "Cool Steel",
            "Xiangling",
            "Harbinger of Dawn",
            "Slingshot",
            "Debate Club",
            "Cool Steel",
            "Slingshot",
            "Bennett",
        ];
        for (ix, name) in ten_pull.iter().enumerate().rev() {
            let rank = if ["Xiangling", "Bennett"].contains(name) {
                "4"
            } else {
                "3"
            };
            rows.push(row(
                2002 + ix as u64,
                "301",
                "2024-01-01 10:05:00",
                name,
                rank,
            ));
        }
        rows.push(row(2001, "301", "2024-01-01 10:00:00", "Slingshot", "3"));
        rows
    }

    fn store_with_sample(dir: &std::path::Path) -> Store {
        let mut store = Store::open(AppPaths::at(dir)).unwrap();
        let batch = paimon::parse(SAMPLE.as_bytes(), GameData::embedded()).unwrap();
        store.apply_import(batch).unwrap();
        store
    }

    fn fetch_from(api: &FakeApi, store: &Store) -> FetchedHistory {
        let known = store.known_api_ids().unwrap();
        let is_known = |uid: &str, id: &str| known.get(uid).is_some_and(|ids| ids.contains(id));
        let link = GachaLink::parse(LINK).unwrap();
        fetch(
            &link,
            api,
            NO_WAIT,
            &is_known,
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .unwrap()
    }

    #[test]
    fn migrated_wishes_get_linked_and_only_new_ones_are_added() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store_with_sample(dir.path());
        let api = FakeApi::default().with_type("301", main_character_history());

        let fetched = fetch_from(&api, &store);
        let first = plan(fetched, &store, GameData::embedded()).unwrap();
        assert_eq!(first.server, Some(Server::Europe));
        assert_eq!(
            first.targets.len(),
            1,
            "UID 700000001 already has an account"
        );
        let target = &first.targets[0];
        assert!(matches!(&target.target, Target::Existing(a) if a.label == "Main"));
        assert_eq!(
            target.linked, 13,
            "every migrated wish of the pool is matched"
        );
        // The two newer wishes and the gap at 12:00.
        assert_eq!(target.new_wishes[&Pool::CharacterEvent], 3);
        assert_eq!(target.held_back_total(), 0);

        let report = store.apply_import(target.batch.clone()).unwrap();
        assert_eq!(report.added(), 3);
        assert_eq!(report.accounts[0].linked, 13);

        // Fetching again stops at the first known wish and adds nothing.
        let again = fetch_from(&api, &store);
        assert!(again.rows.is_empty());
        let second = plan(again, &store, GameData::embedded()).unwrap();
        assert!(
            second
                .targets
                .iter()
                .all(|t| t.new_total() == 0 && t.linked == 0)
        );
    }

    #[test]
    fn a_new_uid_can_go_to_a_new_account_or_one_without_uid() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_with_sample(dir.path());
        let mut rows = vec![
            row(5002, "200", "2024-04-01 10:00:00", "Slingshot", "3"),
            row(5001, "200", "2024-02-15 10:00:00", "Slingshot", "3"),
        ];
        for r in &mut rows {
            r.uid = "800000002".to_owned();
        }
        let api = FakeApi::default().with_type("200", rows);

        let choices = plan(fetch_from(&api, &store), &store, GameData::embedded()).unwrap();
        // A new account, or "Account 2", which has no UID.
        assert_eq!(choices.targets.len(), 2);
        assert_eq!(
            choices.suggested, 0,
            "nothing matched, so a new account is suggested"
        );
        assert_eq!(choices.targets[0].target, Target::New);
        assert_eq!(choices.targets[0].new_total(), 2);

        // Account 2 counted standard wishes by hand up to 2024-03-02: the
        // older game wish could repeat one of them, so it is held back.
        let merge = &choices.targets[1];
        assert!(matches!(&merge.target, Target::Existing(a) if a.label == "Account 2"));
        assert_eq!(merge.new_total(), 1);
        assert_eq!(merge.held_back_total(), 1);
        assert!(merge.batch.warnings[0].contains("typed by hand"));
    }

    #[test]
    fn unknown_items_and_banner_types_do_not_stop_an_import() {
        let rows = vec![
            row(9002, "999", "2024-04-01 10:00:00", "Slingshot", "3"),
            row(9001, "301", "2024-04-01 10:00:00", "Someone New", "5"),
        ];
        let mut warnings = Vec::new();
        let wishes = to_wishes(&rows, GameData::embedded(), &mut warnings).unwrap();
        assert_eq!(wishes.len(), 1);
        assert_eq!(wishes[0].item_key, None);
        assert_eq!(wishes[0].rarity, Rarity::Five);
        assert_eq!(warnings.len(), 1);
    }
}
