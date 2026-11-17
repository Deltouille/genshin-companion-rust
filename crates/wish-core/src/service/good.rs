//! Importing a GOOD file (from Irminsul, Inventory Kamera or Genshin
//! Optimizer): reading it, and what it would change in an account's
//! collection and inventory. Writing it is
//! [`Store::apply_good`](crate::store::Store::apply_good), once the player
//! has confirmed.

use std::path::Path;

use crate::collection::{Collection, GoodFile, GoodPlan, plan_good};
use crate::error::{Error, IoAction, Result};
use crate::model::{Account, AccountId};
use crate::planner::{Inventory, PlannerData};
use crate::store::Store;

/// A GOOD file and what importing it would change.
#[derive(Clone, Debug)]
pub struct GoodPreview {
    pub file: GoodFile,
    /// Where it goes: `None` when there is no account yet (a new one,
    /// "Main").
    pub account: Option<Account>,
    pub plan: GoodPlan,
}

/// Reads the GOOD file at `path`.
pub fn read(path: &Path) -> Result<GoodFile> {
    let bytes = std::fs::read(path).map_err(Error::io(IoAction::Reading, path))?;
    GoodFile::parse(&bytes)
}

/// What importing `file` into `account` would change. Nothing is written.
pub fn preview(
    file: GoodFile,
    store: &Store,
    account: Option<AccountId>,
    planner: &PlannerData,
) -> Result<GoodPreview> {
    let (account, current, inventory) = match account {
        Some(id) => (
            Some(store.account(id)?),
            store.collection(id)?,
            store.inventory(id)?,
        ),
        None => (None, Collection::default(), Inventory::default()),
    };
    let plan = plan_good(&file, &current, &inventory, planner);
    Ok(GoodPreview {
        file,
        account,
        plan,
    })
}
