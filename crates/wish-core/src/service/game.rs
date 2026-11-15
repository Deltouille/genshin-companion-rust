//! Importing from the game: the wish history behind a link the player gives
//! (or the app finds in the game's cache), fetched up to the first wish
//! already stored, then planned. Writing the plan is
//! [`Store::apply_import`](crate::store::Store::apply_import), once the
//! player confirms.

use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use super::lock;
use crate::data::GameData;
use crate::error::Result;
use crate::import::api::{self, FetchProgress, GachaLink, Pacing, Transport};
use crate::import::api_plan::{self, ApiPlan};
use crate::store::Store;

/// Fetches the history behind `link` and plans its import: the new wishes,
/// those matched with migrated ones, those held back, for each account the
/// import can go to. Nothing is written. The store is read before the
/// requests (the wishes already stored, where fetching stops) and after
/// them, never during.
pub fn fetch(
    link: &GachaLink,
    transport: &dyn Transport,
    pacing: Pacing,
    store: &Mutex<Store>,
    data: &GameData,
    progress: &mut dyn FnMut(FetchProgress),
    cancelled: &AtomicBool,
) -> Result<ApiPlan> {
    let known = lock(store).known_api_ids()?;
    let is_known = |uid: &str, id: &str| known.get(uid).is_some_and(|ids| ids.contains(id));
    let fetched = api::fetch(link, transport, pacing, &is_known, progress, cancelled)?;
    api_plan::plan(fetched, &lock(store), data)
}
