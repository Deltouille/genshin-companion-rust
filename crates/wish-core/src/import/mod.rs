//! Importers turn a source (file, API) into an [`ImportBatch`] held in memory.
//! Nothing is written until the batch is passed to
//! [`Store::apply_import`](crate::store::Store::apply_import).

pub mod api;
pub mod api_plan;
pub mod game_cache;
pub mod manual;
pub mod paimon;
pub mod paimon_check;

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::data::GameData;
use crate::error::{Error, Result};
use crate::model::{Account, AccountId, ManualCounters, Pool, Server, Wish, WishId, text_enum};
use crate::snapshot::Snapshot;
use crate::store::Store;

/// A file read and checked for import, not yet written anywhere.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedImport {
    pub batch: ImportBatch,
    /// For a paimon.moe save: how this app's pity and 50/50 results compare
    /// with the ones paimon.moe stored in it.
    pub paimon_check: Option<paimon_check::Comparison>,
}

/// Reads a file chosen by the user, recognising it from its content (not
/// its name): a snapshot, a UIGF file, a paimon.moe JSON save, or a
/// spreadsheet in paimon.moe's export layout.
///
/// A spreadsheet without a UID goes to `target` (the account on screen).
/// UIGF accounts are planned against `store` like a game import.
pub fn prepare(
    file_name: &str,
    bytes: Vec<u8>,
    data: &GameData,
    store: &Store,
    target: Option<AccountId>,
) -> Result<PreparedImport> {
    if crate::excel::is_xlsx(&bytes) {
        let mut batch = crate::excel::parse(&bytes, data)?;
        for account in &mut batch.accounts {
            if account.uid.is_none() {
                account.merge_into = target;
            }
        }
        return Ok(PreparedImport {
            batch: batch.with_original(file_name, bytes),
            paimon_check: None,
        });
    }

    let root: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| Error::UnrecognizedFile)?;
    if crate::uigf::is_uigf(&root) {
        let batch = prepare_uigf(&root, data, store)?;
        return Ok(PreparedImport {
            batch: batch.with_original(file_name, bytes),
            paimon_check: None,
        });
    }
    // A file that says it is a snapshot is read as one, so a damaged or newer
    // snapshot reports its own error instead of "not a paimon.moe save".
    if root.get("format").and_then(|f| f.as_str()) == Some(crate::snapshot::FORMAT) {
        let snapshot = Snapshot::from_json(&bytes)?;
        return Ok(PreparedImport {
            batch: snapshot.into_batch().with_original(file_name, bytes),
            paimon_check: None,
        });
    }

    let batch = match paimon::parse(&bytes, data) {
        Ok(batch) => batch,
        Err(Error::NotPaimonExport | Error::Json(_)) => return Err(Error::UnrecognizedFile),
        Err(other) => return Err(other),
    };
    let paimon_check = paimon_check::compare(&bytes, data)?;
    Ok(PreparedImport {
        batch: batch.with_original(file_name, bytes),
        paimon_check: Some(paimon_check),
    })
}

/// Each UIGF account is planned like a game import, into the suggested
/// account (the one with its UID, else the best match, else a new one).
fn prepare_uigf(root: &serde_json::Value, data: &GameData, store: &Store) -> Result<ImportBatch> {
    let mut batch = ImportBatch::new(ImportSource::Uigf);
    for account in crate::uigf::parse(root, data)? {
        let fetched = api::FetchedHistory {
            uid: Some(account.uid),
            region: None,
            rows: account.rows,
        };
        let plan = api_plan::plan(fetched, store, data)?;
        let Some(target) = plan.targets.into_iter().nth(plan.suggested) else {
            continue;
        };
        batch.warnings.extend(target.batch.warnings);
        for mut planned in target.batch.accounts {
            planned.server = planned.server.or(account.server);
            batch.accounts.push(planned);
        }
    }
    Ok(batch)
}

text_enum! {
    /// Kind of source an import came from, as recorded in the import log.
    pub enum ImportSource {
        PaimonJson => "paimon-json",
        PaimonExcel => "paimon-excel",
        Snapshot => "snapshot",
        Uigf => "uigf",
        Api => "api",
        Manual => "manual",
    }
}

/// Wishes read from one source, not yet written anywhere.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportBatch {
    pub source: ImportSource,
    /// Human-readable origin, such as the file name.
    pub detail: Option<String>,
    pub accounts: Vec<AccountBatch>,
    /// Problems that did not stop the import but deserve a look.
    pub warnings: Vec<String>,
    /// The file as received. It is archived when the batch is applied, so even
    /// data this app does not understand yet is never thrown away.
    pub original: Option<OriginalFile>,
}

impl ImportBatch {
    pub fn new(source: ImportSource) -> Self {
        Self {
            source,
            detail: None,
            accounts: Vec::new(),
            warnings: Vec::new(),
            original: None,
        }
    }

    /// Records where the batch came from, and keeps the raw file for archiving.
    pub fn with_original(mut self, file_name: impl Into<String>, bytes: Vec<u8>) -> Self {
        let name = file_name.into();
        self.detail = Some(name.clone());
        self.original = Some(OriginalFile { name, bytes });
        self
    }

    pub fn wish_count(&self) -> usize {
        self.accounts.iter().map(|a| a.wishes.len()).sum()
    }
}

/// Wishes of one account in a batch. The account is `merge_into` when set,
/// otherwise matched to a local one (by UID, then by label) or created when
/// the batch is applied.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountBatch {
    pub label: String,
    pub uid: Option<String>,
    pub server: Option<Server>,
    pub wishes: Vec<Wish>,
    pub manual_counters: Vec<ManualCounters>,
    /// Game ids learned for wishes already stored without one.
    pub links: Vec<WishLink>,
    /// An existing account (without a UID yet) chosen by the user.
    pub merge_into: Option<AccountId>,
}

/// The game's id for a wish stored without one (migrated from paimon.moe).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WishLink {
    pub wish_id: WishId,
    pub api_id: WishId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// What applying a batch changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportReport {
    /// Backup taken just before writing (none when the database was empty).
    pub backup: Option<PathBuf>,
    /// Where the original file was archived.
    pub archived: Option<PathBuf>,
    pub accounts: Vec<AccountReport>,
    pub warnings: Vec<String>,
}

impl ImportReport {
    pub fn added(&self) -> u32 {
        self.accounts
            .iter()
            .flat_map(|a| a.pools.values())
            .map(|c| c.added)
            .sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountReport {
    pub account: Account,
    /// The account did not exist locally before this import.
    pub created: bool,
    pub pools: BTreeMap<Pool, PoolCounts>,
    /// Game ids newly recorded for stored wishes.
    pub linked: u32,
}

/// One entry of the import log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportRecord {
    pub id: i64,
    pub account: AccountId,
    pub account_label: String,
    pub source: ImportSource,
    pub detail: Option<String>,
    /// Local time, RFC 3339.
    pub imported_at: String,
    pub added: u32,
    pub undone_at: Option<String>,
}

/// What undoing an import removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndoReport {
    pub backup: PathBuf,
    pub removed_wishes: u32,
    pub removed_links: u32,
    pub removed_counters: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolCounts {
    pub added: u32,
    /// Wishes of the batch that were already stored (and left untouched).
    pub already_present: u32,
}
