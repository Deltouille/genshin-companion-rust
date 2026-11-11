use std::io;
use std::path::{Path, PathBuf};

/// Everything that can go wrong in `wish-core`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("{action} {}: {source}", path.display())]
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },

    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("could not find a data directory for this user")]
    NoDataDir,

    #[error(
        "the database uses schema version {found}, but this build only knows up to {known}; \
         it was written by a newer version of the app and is left untouched"
    )]
    SchemaTooNew { found: i64, known: i64 },

    #[error("invalid game data: {0}")]
    GameData(String),

    #[error("not a paimon.moe export: no `wish-counter-*` entries found")]
    NotPaimonExport,

    #[error(
        "this file is neither a paimon.moe save (Settings → Export, or the Google Drive file) \
         nor a Genshin Companion snapshot"
    )]
    UnrecognizedFile,

    #[error(
        "the export contains items this build does not know: {}. \
         Update the game data (`git -C paimon-moe pull` then `node tools/extract-paimon-data.mjs`)",
        .0.join(", ")
    )]
    UnknownItems(Vec<String>),

    #[error("`{key}` pull #{index}: {message}")]
    InvalidPull {
        key: String,
        index: usize,
        message: String,
    },

    #[error("`{key}`: {message}")]
    InvalidEntry { key: String, message: String },

    #[error("not a snapshot: expected format `{expected}`, found `{found}`")]
    NotSnapshot {
        expected: &'static str,
        found: String,
    },

    #[error(
        "snapshot format version {found} is newer than this build supports ({known}); update the app"
    )]
    SnapshotTooNew { found: u32, known: u32 },

    #[error("{} isn’t a backup of this app’s database", .0.display())]
    NotABackup(PathBuf),

    #[error("{} is damaged ({problem}); choose another backup", file.display())]
    DamagedBackup { file: PathBuf, problem: String },

    #[error("couldn’t read or write the spreadsheet: {0}")]
    Excel(String),

    #[error("this spreadsheet isn’t a paimon.moe wish history export")]
    NotPaimonExcel,

    #[error("there is no account {0}")]
    UnknownAccount(crate::model::AccountId),

    #[error(
        "“{account}” already belongs to another UID, so the wishes of UID {uid} can’t be added to it"
    )]
    AccountMismatch { account: String, uid: String },

    #[error("wish {0} wasn’t entered by hand, so it can’t be deleted; undo its import instead")]
    NotManual(String),

    #[error("there is no import {0}")]
    UnknownImport(i64),

    #[error("import {0} was already undone")]
    AlreadyUndone(i64),

    #[error(
        "this isn’t a wish history link. Copy the whole link: it starts with https:// and contains “authkey”"
    )]
    InvalidLink,

    #[error(
        "the wish history link has expired. Open the wish history in the game again, then use the new link"
    )]
    LinkExpired,

    #[error(
        "the game server didn’t accept this link. Copy the whole link again; if it still fails, open the wish history in the game again for a new one"
    )]
    LinkRejected,

    #[error("the game server refused the request: {message} ({retcode})")]
    Api { retcode: i64, message: String },

    #[error("couldn’t reach the game server: {0}")]
    Network(String),

    #[error("unexpected answer from the game server: {0}")]
    UnexpectedResponse(String),

    #[error("couldn’t download the image {url}: {problem}")]
    ImageDownload { url: String, problem: String },

    #[error(
        "the history mixes two accounts (UIDs {first} and {second}); use a link from one account"
    )]
    MixedAccounts { first: String, second: String },

    #[error("cancelled")]
    Cancelled,
}

impl Error {
    pub(crate) fn io(action: &'static str, path: &Path) -> impl FnOnce(io::Error) -> Self {
        let path = path.to_path_buf();
        move |source| Self::Io {
            action,
            path,
            source,
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
