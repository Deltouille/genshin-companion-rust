//! Lossless JSON snapshot of every account, for backups outside the app's
//! folder (the mirror folder) and for moving data between machines.
//!
//! UIGF is not used here: it requires numeric API ids and HoYoverse item ids,
//! which migrated paimon.moe wishes do not have. A snapshot keeps every field
//! of every record, so importing one gives back exactly what was exported.

use std::path::{Path, PathBuf};

use chrono::{Local, SecondsFormat};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::import::{AccountBatch, ImportBatch, ImportSource, WishLink};
use crate::model::{ManualCounters, Server, Wish};
use crate::paths::write_atomic;
use crate::store::Store;

// Named after the app's former name (Genshin Wishes), kept so that every
// snapshot written so far still reads, and the mirror file stays the same.
pub const FORMAT: &str = "genshin-wishes-snapshot";
pub const VERSION: u32 = 1;

/// Name of the snapshot kept in the mirror folder. One fixed name, so a
/// syncing tool replaces the file instead of piling up copies.
pub const MIRROR_FILE: &str = "genshin-wishes-snapshot.json";

/// Writes a snapshot of the whole store to `folder`, atomically.
pub fn write_mirror(store: &Store, folder: &Path) -> Result<PathBuf> {
    let path = folder.join(MIRROR_FILE);
    Snapshot::from_store(store)?.write_to(&path)?;
    Ok(path)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub format: String,
    pub version: u32,
    /// Local time of the export, RFC 3339.
    pub exported_at: String,
    pub app_version: String,
    pub accounts: Vec<SnapshotAccount>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotAccount {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<Server>,
    pub wishes: Vec<Wish>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub manual_counters: Vec<ManualCounters>,
    /// Game ids learned for migrated wishes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<WishLink>,
}

impl Snapshot {
    /// Everything in the store, accounts in creation order and wishes in
    /// chronological order.
    pub fn from_store(store: &Store) -> Result<Self> {
        let accounts = store
            .accounts()?
            .into_iter()
            .map(|account| {
                Ok(SnapshotAccount {
                    wishes: store.wishes(account.id)?,
                    manual_counters: store.manual_counters(account.id)?,
                    links: store.links(account.id)?,
                    label: account.label,
                    uid: account.uid,
                    server: account.server,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            format: FORMAT.to_owned(),
            version: VERSION,
            exported_at: Local::now().to_rfc3339_opts(SecondsFormat::Secs, false),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            accounts,
        })
    }

    pub fn from_json(json: &[u8]) -> Result<Self> {
        #[derive(Deserialize)]
        struct Header {
            #[serde(default)]
            format: String,
            #[serde(default)]
            version: u32,
        }

        // Check the header first so a newer file gets a clear message
        // rather than a field-level parse error.
        let header: Header = serde_json::from_slice(json)?;
        if header.format != FORMAT {
            return Err(Error::NotSnapshot {
                expected: FORMAT,
                found: header.format,
            });
        }
        if header.version > VERSION {
            return Err(Error::SnapshotTooNew {
                found: header.version,
                known: VERSION,
            });
        }
        Ok(serde_json::from_slice(json)?)
    }

    pub fn to_json(&self) -> Vec<u8> {
        let mut json = serde_json::to_vec_pretty(self).expect("snapshot types always serialize");
        json.push(b'\n');
        json
    }

    /// Writes the snapshot atomically: readers never see a partial file.
    pub fn write_to(&self, path: &Path) -> Result<()> {
        write_atomic(path, &self.to_json())
    }

    /// A batch that merges this snapshot into a store.
    pub fn into_batch(self) -> ImportBatch {
        let mut batch = ImportBatch::new(ImportSource::Snapshot);
        batch.accounts = self
            .accounts
            .into_iter()
            .map(|account| AccountBatch {
                label: account.label,
                uid: account.uid,
                server: account.server,
                wishes: account.wishes,
                manual_counters: account.manual_counters,
                links: account.links,
                merge_into: None,
            })
            .collect();
        batch
    }
}
