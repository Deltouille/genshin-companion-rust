//! Syncing characters and weapons from HoYoLAB: every Genshin account behind
//! the player's cookie, its characters' details, and what a sync would change
//! in each app account it can go to. Writing one is
//! [`Store::apply_sync`](crate::store::Store::apply_sync), once the player
//! has chosen.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use hoyolab::{CharacterDetail, Client, Cookie, GameAccount, Transport};

use super::lock;
use crate::archive::ArchiveData;
use crate::collection::{Collection, SyncPlan, plan_sync};
use crate::error::{Error, Result};
use crate::model::{Account, AccountId};
use crate::planner::PlannerData;
use crate::store::Store;

/// One Genshin account HoYoLAB showed: its characters, and what a sync would
/// change in each app account it can go to.
#[derive(Clone, Debug)]
pub struct HoyolabAccount {
    pub account: GameAccount,
    pub details: Vec<CharacterDetail>,
    /// Where the sync can go, each with what it would change there: the
    /// app's account with that UID; without one, a new account first, then
    /// each account without a UID ([`Store::sync_targets`]).
    pub choices: Vec<SyncChoice>,
    /// Index into `choices` of the suggested one.
    pub suggested: usize,
}

/// One place a HoYoLAB sync can go, and what it would change there.
#[derive(Clone, Debug)]
pub struct SyncChoice {
    /// `None`: a new account, named after the game's nickname.
    pub target: Option<Account>,
    pub plan: SyncPlan,
}

/// Every Genshin account behind `cookie`, each with its characters' details.
/// The cookie lives in the client, and is dropped (and wiped) when this
/// returns. No account at all is [`hoyolab::Error::NoGameAccount`].
pub fn fetch(
    cookie: Cookie,
    transport: &dyn Transport,
    cancelled: &AtomicBool,
) -> Result<Vec<(GameAccount, Vec<CharacterDetail>)>> {
    let stop = || match cancelled.load(Ordering::Relaxed) {
        true => Err(Error::Cancelled),
        false => Ok(()),
    };
    let client = Client::new(cookie, transport);
    let mut fetched = Vec::new();
    for account in client.game_accounts()? {
        stop()?;
        let characters = client.characters(&account)?;
        let ids: Vec<u32> = characters.iter().map(|c| c.id).collect();
        stop()?;
        let details = client.character_details(&account, &ids)?;
        fetched.push((account, details));
    }
    if fetched.is_empty() {
        return Err(hoyolab::Error::NoGameAccount.into());
    }
    Ok(fetched)
}

/// What syncing `details` would change in each account it can go to
/// (`shown`, the account on screen, is suggested when it has no UID).
/// Nothing is written.
pub fn preview(
    account: GameAccount,
    details: Vec<CharacterDetail>,
    store: &Store,
    shown: Option<AccountId>,
    planner: &PlannerData,
    archive: &ArchiveData,
) -> Result<HoyolabAccount> {
    let targets = store.sync_targets(&account.uid, shown)?;
    let choices = (targets.targets.into_iter())
        .map(|target| {
            let current = match &target {
                Some(target) => store.collection(target.id)?,
                None => Collection::default(),
            };
            let plan = plan_sync(&details, &current, planner, archive);
            Ok(SyncChoice { target, plan })
        })
        .collect::<Result<_>>()?;
    Ok(HoyolabAccount {
        account,
        details,
        choices,
        suggested: targets.suggested,
    })
}

/// [`fetch`], then [`preview`] for every account: the store is locked after
/// the requests only.
pub fn fetch_preview(
    cookie: Cookie,
    transport: &dyn Transport,
    store: &Mutex<Store>,
    shown: Option<AccountId>,
    planner: &PlannerData,
    archive: &ArchiveData,
    cancelled: &AtomicBool,
) -> Result<Vec<HoyolabAccount>> {
    let fetched = fetch(cookie, transport, cancelled)?;
    let store = lock(store);
    (fetched.into_iter())
        .map(|(account, details)| preview(account, details, &store, shown, planner, archive))
        .collect()
}
