//! The core's one error type, [`Error`], and [`IoAction`], what was being
//! done to a file when it failed.
//!
//! Each variant's message is an `error-…` text of `locales/<code>/core.ftl`,
//! so it reads in the current language; the app shows it as it is, often after
//! "what failed:" (`Notice::failure`). A test checks that no message names a
//! developer's step.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Everything that can go wrong in `wish-core`.
///
/// The messages reach the player as they are, in the current language
/// (`locales/<language>/core.ftl`): they say what happened and what the
/// player can do, never a developer's step (a command to run, a script).
/// What a developer needs goes in the data the variant carries.
#[derive(Debug)]
pub enum Error {
    Database(rusqlite::Error),

    Io {
        action: IoAction,
        path: PathBuf,
        source: io::Error,
    },

    Json(serde_json::Error),

    NoDataDir,

    SchemaTooNew {
        found: i64,
        known: i64,
    },

    /// A schema version no app writes (a negative `user_version`): the file
    /// is refused untouched, since no migration could bring it up to date.
    SchemaDamaged {
        found: i64,
    },

    GameData(String),

    NotPaimonExport,

    UnrecognizedFile,

    /// Not a GOOD file (Irminsul's, Inventory Kamera's, Genshin Optimizer's).
    NotGoodFile,

    UnknownItems(Vec<String>),

    InvalidPull {
        key: String,
        index: usize,
        message: String,
    },

    InvalidEntry {
        key: String,
        message: String,
    },

    NotSnapshot {
        expected: &'static str,
        found: String,
    },

    SnapshotTooNew {
        found: u32,
        known: u32,
    },

    NotABackup(PathBuf),

    DamagedBackup {
        file: PathBuf,
        problem: String,
    },

    Excel(String),

    NotPaimonExcel,

    UnknownAccount(crate::model::AccountId),

    EmptyAccountName,

    InvalidUid(String),

    UidTaken {
        uid: String,
        account: String,
    },

    AccountMismatch {
        account: String,
        uid: String,
    },

    /// An account's UID came from the game, or was given once: it stays.
    AccountHasUid(String),

    /// An account with wishes (or counters, or imports in the history)
    /// isn't removed.
    AccountHasWishes(String),

    NotManual(String),

    UnknownImport(i64),

    AlreadyUndone(i64),

    InvalidLink,

    LinkExpired,

    LinkRejected,

    Api {
        retcode: i64,
        message: String,
    },

    Network(String),

    UnexpectedResponse(String),

    ImageDownload {
        url: String,
        problem: String,
    },

    MixedAccounts {
        first: String,
        second: String,
    },

    Cancelled,

    /// HoYoLAB's answer, in words that say what the player can do.
    Hoyolab(hoyolab::Error),
}

/// What was being done to a file when it failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoAction {
    Reading,
    Writing,
    Creating,
    Copying,
    /// Copying the former data folder into a new one, at the first start.
    CopyingDataFolder,
    Deleting,
    /// Making sure a file written is on the disk.
    Flushing,
    Listing,
    Locking,
    Opening,
    Removing,
    Renaming,
    Replacing,
}

impl Error {
    pub(crate) fn io(action: IoAction, path: &Path) -> impl FnOnce(io::Error) -> Self {
        let path = path.to_path_buf();
        move |source| Self::Io {
            action,
            path,
            source,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Error::Database(e) => t!("error-database", detail = e.to_string()),
            Error::Io {
                action,
                path,
                source,
            } => {
                let (path, detail) = (path.display().to_string(), source.to_string());
                match action {
                    IoAction::Reading => t!("error-io-reading", path = path, detail = detail),
                    IoAction::Writing => t!("error-io-writing", path = path, detail = detail),
                    IoAction::Creating => t!("error-io-creating", path = path, detail = detail),
                    IoAction::Copying => t!("error-io-copying", path = path, detail = detail),
                    IoAction::CopyingDataFolder => {
                        t!("error-io-copying-data-folder", path = path, detail = detail)
                    }
                    IoAction::Deleting => t!("error-io-deleting", path = path, detail = detail),
                    IoAction::Flushing => t!("error-io-flushing", path = path, detail = detail),
                    IoAction::Listing => t!("error-io-listing", path = path, detail = detail),
                    IoAction::Locking => t!("error-io-locking", path = path, detail = detail),
                    IoAction::Opening => t!("error-io-opening", path = path, detail = detail),
                    IoAction::Removing => t!("error-io-removing", path = path, detail = detail),
                    IoAction::Renaming => t!("error-io-renaming", path = path, detail = detail),
                    IoAction::Replacing => t!("error-io-replacing", path = path, detail = detail),
                }
            }
            Error::Json(e) => t!("error-json", detail = e.to_string()),
            Error::NoDataDir => t!("error-no-data-dir"),
            Error::SchemaTooNew { found, known } => {
                t!("error-schema-too-new", found = *found, known = *known)
            }
            Error::SchemaDamaged { found } => t!("error-schema-damaged", found = *found),
            Error::GameData(detail) => t!("error-game-data", detail = detail.as_str()),
            Error::NotPaimonExport => t!("error-not-paimon-export"),
            Error::UnrecognizedFile => t!("error-unrecognized-file"),
            Error::NotGoodFile => t!("error-not-good-file"),
            Error::UnknownItems(items) => t!("error-unknown-items", items = items.join(", ")),
            Error::InvalidPull {
                key,
                index,
                message,
            } => t!(
                "error-invalid-pull",
                key = key.as_str(),
                index = index.to_string(),
                detail = message.as_str()
            ),
            Error::InvalidEntry { key, message } => {
                t!(
                    "error-invalid-entry",
                    key = key.as_str(),
                    detail = message.as_str()
                )
            }
            Error::NotSnapshot { expected, found } => {
                t!(
                    "error-not-snapshot",
                    found = found.as_str(),
                    expected = *expected
                )
            }
            Error::SnapshotTooNew { found, known } => {
                t!("error-snapshot-too-new", found = *found, known = *known)
            }
            Error::NotABackup(file) => t!("error-not-a-backup", file = file.display().to_string()),
            Error::DamagedBackup { file, problem } => t!(
                "error-damaged-backup",
                file = file.display().to_string(),
                problem = problem.as_str()
            ),
            Error::Excel(detail) => t!("error-excel", detail = detail.as_str()),
            Error::NotPaimonExcel => t!("error-not-paimon-excel"),
            Error::UnknownAccount(account) => {
                t!("error-unknown-account", account = account.to_string())
            }
            Error::EmptyAccountName => t!("error-empty-account-name"),
            Error::AccountHasUid(account) => {
                t!("error-account-has-uid", account = account.as_str())
            }
            Error::AccountHasWishes(account) => {
                t!("error-account-has-wishes", account = account.as_str())
            }
            Error::InvalidUid(uid) => t!("error-invalid-uid", uid = uid.as_str()),
            Error::UidTaken { uid, account } => {
                t!(
                    "error-uid-taken",
                    uid = uid.as_str(),
                    account = account.as_str()
                )
            }
            Error::AccountMismatch { account, uid } => {
                t!(
                    "error-account-mismatch",
                    account = account.as_str(),
                    uid = uid.as_str()
                )
            }
            Error::NotManual(wish) => t!("error-not-manual", wish = wish.as_str()),
            Error::UnknownImport(import) => t!("error-unknown-import", import = import.to_string()),
            Error::AlreadyUndone(import) => {
                t!("error-already-undone", import = import.to_string())
            }
            Error::InvalidLink => t!("error-invalid-link"),
            Error::LinkExpired => t!("error-link-expired"),
            Error::LinkRejected => t!("error-link-rejected"),
            Error::Api { retcode, message } => t!(
                "error-api",
                message = message.as_str(),
                retcode = retcode.to_string()
            ),
            Error::Network(detail) => t!("error-network", detail = detail.as_str()),
            Error::UnexpectedResponse(detail) => {
                t!("error-unexpected-response", detail = detail.as_str())
            }
            Error::ImageDownload { url, problem } => {
                t!(
                    "error-image-download",
                    url = url.as_str(),
                    problem = problem.as_str()
                )
            }
            Error::MixedAccounts { first, second } => {
                t!(
                    "error-mixed-accounts",
                    first = first.as_str(),
                    second = second.as_str()
                )
            }
            Error::Cancelled => t!("error-cancelled"),
            Error::Hoyolab(error) => hoyolab_message(error),
        };
        f.write_str(&text)
    }
}

/// HoYoLAB's errors, in the current language. The `hoyolab` crate words
/// them in English for itself.
fn hoyolab_message(error: &hoyolab::Error) -> String {
    use hoyolab::Error as H;
    match error {
        H::MissingCookie(field) => t!("hoyolab-missing-cookie", field = *field),
        H::InvalidCookie => t!("hoyolab-invalid-cookie"),
        H::DataNotPublic => t!("hoyolab-data-not-public"),
        H::Captcha => t!("hoyolab-captcha"),
        H::TooFrequent => t!("hoyolab-too-frequent"),
        H::NoGameAccount => t!("hoyolab-no-game-account"),
        H::NotHoyoverse(host) => t!("hoyolab-not-hoyoverse", host = host.as_str()),
        H::Api { retcode, message } => t!(
            "hoyolab-api",
            retcode = retcode.to_string(),
            message = message.as_str()
        ),
        H::Network(detail) => t!("hoyolab-network", detail = detail.as_str()),
        H::Format(detail) => t!("hoyolab-format", detail = detail.as_str()),
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Database(e) => Some(e),
            Error::Io { source, .. } => Some(source),
            Error::Json(e) => Some(e),
            Error::Hoyolab(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Error::Database(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Json(error)
    }
}

impl From<hoyolab::Error> for Error {
    fn from(error: hoyolab::Error) -> Self {
        Error::Hoyolab(error)
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use crate::i18n::{Language, with_language};

    use super::*;

    /// The messages reach the player: none names a developer's step.
    #[test]
    fn messages_name_no_developer_step() {
        for source in [
            include_str!("../locales/en/core.ftl"),
            include_str!("../locales/fr/core.ftl"),
        ] {
            for step in ["git ", "node ", ".mjs", "cargo ", "this build", "tools/"] {
                assert!(!source.contains(step), "a message names `{step}`");
            }
        }
    }

    #[test]
    fn messages_come_in_the_current_language() {
        let error = Error::UidTaken {
            uid: "700000123".to_owned(),
            account: "Main".to_owned(),
        };
        assert_eq!(
            with_language(Language::English, || error.to_string()),
            "UID 700000123 is already the account “Main”"
        );
        assert_eq!(
            with_language(Language::French, || error.to_string()),
            "l’UID 700000123 est déjà le compte «\u{a0}Main\u{a0}»"
        );
        let hoyolab = Error::Hoyolab(hoyolab::Error::TooFrequent);
        assert!(with_language(Language::French, || hoyolab.to_string()).starts_with("HoYoLAB"));
    }
}
