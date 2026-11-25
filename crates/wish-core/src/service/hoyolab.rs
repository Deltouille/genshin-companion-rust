//! Syncing from HoYoLAB: every Genshin account behind the player's cookie,
//! its characters' details and its chronicle (the Spiral Abyss, the
//! Imaginarium Theater and the Traveler's Diary HoYoLAB still shows), and
//! what a sync would change in each app account it can go to. Writing one is
//! [`Store::apply_hoyolab`](crate::store::Store::apply_hoyolab), once the
//! player has chosen.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use hoyolab::chronicle::{Currency, DIARY_PAGE_SIZE, DiaryMonth};
use hoyolab::{CharacterDetail, Client, Cookie, GameAccount, Transport};

use super::lock;
use crate::archive::ArchiveData;
use crate::chronicle::{Record, RecordsPlan};
use crate::collection::{Collection, SyncPlan, plan_sync};
use crate::error::{Error, Result};
use crate::import::api::pause;
use crate::model::{Account, AccountId, Server};
use crate::planner::PlannerData;
use crate::store::Store;

/// The most pages of a month's primogem log asked for: a month has a few
/// hundred rewards at most, so this only stops an answer that never ends.
const MAX_LOG_PAGES: u32 = 30;

/// What HoYoLAB showed of one Genshin account.
#[derive(Clone, Debug)]
pub struct Fetched {
    pub account: GameAccount,
    pub details: Vec<CharacterDetail>,
    /// The chronicle's periods it still shows, each kept whole.
    pub records: Vec<Record>,
}

/// One Genshin account HoYoLAB showed: its characters and its chronicle,
/// and what a sync would change in each app account it can go to.
#[derive(Clone, Debug)]
pub struct HoyolabAccount {
    pub account: GameAccount,
    pub details: Vec<CharacterDetail>,
    pub records: Vec<Record>,
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
    pub records: RecordsPlan,
}

/// What a sync asks HoYoLAB for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wanted {
    /// The characters and the chronicle: the sync the player starts, with
    /// its preview.
    Everything,
    /// The chronicle alone: the sync with the login the keyring keeps,
    /// which writes without a preview ([`keep_chronicle`]).
    Chronicle,
}

/// Every Genshin account behind `cookie`, each with its characters' details
/// (unless only the chronicle is `wanted`) and its chronicle, as HoYoLAB
/// shows them at `fetched_at`. `delay` spaces the requests. The cookie lives
/// in the client, and is dropped (and wiped) when this returns. No account at
/// all is [`hoyolab::Error::NoGameAccount`].
pub fn fetch(
    cookie: Cookie,
    transport: &dyn Transport,
    delay: Duration,
    fetched_at: DateTime<Utc>,
    wanted: Wanted,
    cancelled: &AtomicBool,
) -> Result<Vec<Fetched>> {
    let stop = || match cancelled.load(Ordering::Relaxed) {
        true => Err(Error::Cancelled),
        false => Ok(()),
    };
    let wait = || pause(delay, cancelled);
    let client = Client::new(cookie, transport);
    let mut fetched = Vec::new();
    for account in client.game_accounts()? {
        let details = match wanted {
            Wanted::Everything => {
                stop()?;
                let characters = client.characters(&account)?;
                let ids: Vec<u32> = characters.iter().map(|c| c.id).collect();
                stop()?;
                client.character_details(&account, &ids)?
            }
            Wanted::Chronicle => Vec::new(),
        };
        let records = chronicle(&client, &account, &wait, fetched_at)?;
        fetched.push(Fetched {
            account,
            details,
            records,
        });
    }
    if fetched.is_empty() {
        return Err(hoyolab::Error::NoGameAccount.into());
    }
    Ok(fetched)
}

/// `account`'s chronicle as HoYoLAB shows it: the Abyss's two periods, the
/// Theater's, and the Diary's months with their primogem logs. `wait` comes
/// before each request.
fn chronicle(
    client: &Client,
    account: &GameAccount,
    wait: &dyn Fn() -> Result<()>,
    fetched_at: DateTime<Utc>,
) -> Result<Vec<Record>> {
    let mut records = Vec::new();
    for previous in [false, true] {
        wait()?;
        let data = client.spiral_abyss_raw(account, previous)?;
        records.extend(Record::abyss(data, fetched_at)?);
    }
    wait()?;
    records.extend(Record::theater(client.theater_raw(account)?, fetched_at)?);

    let server = Server::from_api_region(&account.region).ok_or_else(|| {
        hoyolab::Error::Format(format!("{} isn't a server HoYoLAB serves", account.region))
    })?;
    wait()?;
    let this_month = client.diary_raw(account, None)?;
    let read: DiaryMonth = hoyolab::chronicle::read(&this_month)?;
    let current = read.data_month;
    let mut months = read.optional_month.clone();
    if !months.contains(&current) {
        months.push(current);
    }
    for month in months {
        let totals = if month == current {
            this_month.clone()
        } else {
            wait()?;
            client.diary_raw(account, Some(month))?
        };
        let mut pages = Vec::new();
        for page in 1..=MAX_LOG_PAGES {
            wait()?;
            let data = client.diary_log_raw(account, month, Currency::Primogems, page)?;
            let length = data["list"].as_array().map_or(0, Vec::len);
            pages.push(data);
            if length < DIARY_PAGE_SIZE {
                break;
            }
        }
        records.push(Record::diary(totals, pages, current, server, fetched_at)?);
    }
    Ok(records)
}

/// What syncing `fetched` would change in each account it can go to
/// (`shown`, the account on screen, is suggested when it has no UID).
/// Nothing is written.
pub fn preview(
    fetched: Fetched,
    store: &Store,
    shown: Option<AccountId>,
    planner: &PlannerData,
    archive: &ArchiveData,
) -> Result<HoyolabAccount> {
    let Fetched {
        account,
        details,
        records,
    } = fetched;
    let targets = store.sync_targets(&account.uid, shown)?;
    let choices = (targets.targets.into_iter())
        .map(|target| {
            let (current, stored) = match &target {
                Some(target) => (store.collection(target.id)?, store.chronicle(target.id)?),
                None => (Collection::default(), Vec::new()),
            };
            let plan = plan_sync(&details, &current, planner, archive);
            let records = RecordsPlan::new(&records, &stored);
            Ok(SyncChoice {
                target,
                plan,
                records,
            })
        })
        .collect::<Result<_>>()?;
    Ok(HoyolabAccount {
        account,
        details,
        records,
        choices,
        suggested: targets.suggested,
    })
}

/// [`fetch`], then [`preview`] for every account: the store is locked after
/// the requests only.
#[expect(
    clippy::too_many_arguments,
    reason = "what a fetch reaches, each its own"
)]
pub fn fetch_preview(
    cookie: Cookie,
    transport: &dyn Transport,
    delay: Duration,
    store: &Mutex<Store>,
    shown: Option<AccountId>,
    planner: &PlannerData,
    archive: &ArchiveData,
    cancelled: &AtomicBool,
) -> Result<Vec<HoyolabAccount>> {
    let fetched = fetch(
        cookie,
        transport,
        delay,
        Utc::now(),
        Wanted::Everything,
        cancelled,
    )?;
    let store = lock(store);
    (fetched.into_iter())
        .map(|fetched| preview(fetched, &store, shown, planner, archive))
        .collect()
}

/// What a sync of the chronicle alone kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChronicleSync {
    /// The game UIDs behind the login.
    pub uids: Vec<String>,
    /// What changed, all accounts together.
    pub kept: RecordsPlan,
    /// The UIDs no account here has: their chronicle waits for a sync the
    /// player starts, which links an account to them.
    pub unlinked: Vec<String>,
}

/// Writes the chronicle of each account `fetched` into the app's account
/// with its UID, without a preview ([`Store::keep_chronicle`]: it only
/// adds, and a period kept once over never changes). An account no app
/// account has the UID of is left for a sync the player starts: this makes
/// no account, and links none.
pub fn keep_chronicle(store: &mut Store, fetched: Vec<Fetched>) -> Result<ChronicleSync> {
    let mut sync = ChronicleSync::default();
    for Fetched {
        account, records, ..
    } in fetched
    {
        match store.account_with_uid(&account.uid)? {
            Some(target) => {
                let kept = store.keep_chronicle(target.id, &records)?;
                for (all, one) in [
                    (&mut sync.kept.abyss, kept.abyss),
                    (&mut sync.kept.theater, kept.theater),
                    (&mut sync.kept.diary, kept.diary),
                ] {
                    all.added += one.added;
                    all.updated += one.updated;
                    all.kept += one.kept;
                }
            }
            None => sync.unlinked.push(account.uid.clone()),
        }
        sync.uids.push(account.uid);
    }
    Ok(sync)
}
