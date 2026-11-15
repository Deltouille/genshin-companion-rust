//! Exports: the bytes of a file and what it holds, for the app to save where
//! the player chose (or share) and to put into words. Nothing is written
//! here.

use crate::data::GameData;
use crate::engine;
use crate::error::Result;
use crate::excel;
use crate::model::AccountId;
use crate::snapshot::Snapshot;
use crate::store::Store;
use crate::uigf;

/// An export's file, and what it holds.
#[derive(Clone, Debug)]
pub struct Exported {
    pub bytes: Vec<u8>,
    pub summary: ExportSummary,
}

/// What an export holds, for a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportSummary {
    /// Every account, with everything it has.
    Snapshot { wishes: usize, accounts: usize },
    /// The accounts with a UID; `without_uid` names the others, left out.
    Uigf {
        wishes: usize,
        accounts: usize,
        without_uid: Vec<String>,
    },
    /// One account's wishes, as paimon.moe's spreadsheet.
    Spreadsheet { account: String },
}

/// A snapshot of the whole store: every account, its wishes, characters,
/// weapons, plan and inventory.
pub fn snapshot(store: &Store) -> Result<Exported> {
    let snapshot = Snapshot::from_store(store)?;
    let summary = ExportSummary::Snapshot {
        wishes: snapshot.accounts.iter().map(|a| a.wishes.len()).sum(),
        accounts: snapshot.accounts.len(),
    };
    Ok(Exported {
        bytes: snapshot.to_json(),
        summary,
    })
}

/// The wishes of every account with a UID, as UIGF v4.
pub fn uigf(store: &Store, data: &GameData) -> Result<Exported> {
    let export = uigf::export(store, data)?;
    Ok(Exported {
        bytes: export.json,
        summary: ExportSummary::Uigf {
            wishes: export.wishes,
            accounts: export.accounts,
            without_uid: export.without_uid,
        },
    })
}

/// `account`'s wishes as paimon.moe's spreadsheet, with its pity and 50/50
/// worked out again.
pub fn spreadsheet(store: &Store, account: AccountId, data: &GameData) -> Result<Exported> {
    let account = store.account(account)?;
    let analysis = engine::analyze(
        store.wishes(account.id)?,
        account.server,
        &store.manual_counters(account.id)?,
        data,
    );
    Ok(Exported {
        bytes: excel::export(&account, &analysis, data)?,
        summary: ExportSummary::Spreadsheet {
            account: account.label,
        },
    })
}
