//! Automatic database backups under `<root>/backups/`.
//!
//! File names carry the time (UTC) and the reason, e.g.
//! `wishes-20261109-115400-123Z-before-import.db`. Names written before the
//! times were UTC have no `Z` and local times; they still read.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, NaiveDateTime, TimeDelta, TimeZone as _, Utc};
use rusqlite::backup::Progress;
use rusqlite::{Connection, MAIN_DB, OpenFlags};

use super::Store;
use crate::error::{Error, Result};
use crate::model::text_enum;

/// How many backups are kept, besides the hourly ones; older ones are
/// deleted.
pub const KEEP_BACKUPS: usize = 30;
/// How many of the hourly backups (taken while counting by hand or editing
/// the plan) are kept. They are counted apart, so a long session can't push
/// the backups taken before imports, undos and restores out of the folder.
pub const KEEP_HOURLY_BACKUPS: usize = 10;

const PREFIX: &str = "wishes-";
const EXTENSION: &str = ".db";
/// UTC, down to the millisecond so that back-to-back backups differ, then
/// `Z`. In local time, a clock set back (summer time ending, another time
/// zone) would make a newer backup look older, and the wrong one pruned.
const STAMP_FORMAT: &str = "%Y%m%d-%H%M%S-%3fZ";
const STAMP_LEN: usize = "20261109-115400-123".len();

text_enum! {
    pub enum BackupReason {
        Daily => "daily",
        BeforeImport => "before-import",
        BeforeMigration => "before-migration",
        Manual => "manual",
        BeforeUndo => "before-undo",
        BeforeRestore => "before-restore",
        BeforeDelete => "before-delete",
        BeforeCounting => "before-counting",
        BeforePlanning => "before-planning",
    }
}

/// While counting by hand or editing the plan, one backup covers this long: a
/// backup per click would soon push the backups taken before real imports
/// out of the folder.
const COUNTING_BACKUP_INTERVAL: TimeDelta = TimeDelta::hours(1);

impl BackupReason {
    /// Whether this backup is one of those taken at most once an hour, kept
    /// apart ([`KEEP_HOURLY_BACKUPS`]).
    pub fn hourly(self) -> bool {
        matches!(
            self,
            BackupReason::BeforeCounting | BackupReason::BeforePlanning
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupFile {
    pub path: PathBuf,
    /// When the backup was taken, shown in local time.
    pub taken_at: DateTime<Local>,
    pub reason: Option<BackupReason>,
}

impl Store {
    /// Copies the whole database to a new backup file, then prunes old ones.
    pub fn backup(&self, reason: BackupReason) -> Result<PathBuf> {
        let path = self.write_backup(reason)?;
        prune(&self.paths.backups())?;
        Ok(path)
    }

    /// Replaces the whole database with `file`: one of the backups, or any
    /// database file written by this app. The current state is backed up
    /// first; an older schema is migrated. Returns that safety backup.
    pub fn restore(&mut self, file: &Path) -> Result<PathBuf> {
        let latest = super::MIGRATIONS.len() as i64;
        {
            fs::metadata(file).map_err(Error::io("reading", file))?;
            let not_a_backup = |_| Error::NotABackup(file.to_path_buf());
            let source = Connection::open_with_flags(file, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(not_a_backup)?;
            let version: i64 = source
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .map_err(not_a_backup)?;
            let has_wishes: bool = source
                .query_row(
                    "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'wishes')",
                    [],
                    |row| row.get(0),
                )
                .map_err(not_a_backup)?;
            // The schema is copied with the data: a trigger or a view, which
            // this app never makes, could act on the next import (a trigger
            // deleting every wish). A file holding one is no backup of ours.
            let foreign: i64 = source
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type IN ('trigger', 'view')",
                    [],
                    |row| row.get(0),
                )
                .map_err(not_a_backup)?;
            if version == 0 || !has_wishes || foreign > 0 {
                return Err(Error::NotABackup(file.to_path_buf()));
            }
            if version > latest {
                return Err(Error::SchemaTooNew {
                    found: version,
                    known: latest,
                });
            }
            let check: String = source.pragma_query_value(None, "quick_check", |row| row.get(0))?;
            if check != "ok" {
                return Err(Error::DamagedBackup {
                    file: file.to_path_buf(),
                    problem: check,
                });
            }
        }

        // Not pruned yet: `file` may be the oldest backup, which pruning
        // would delete before it is read.
        let safety = self.write_backup(BackupReason::BeforeRestore)?;
        self.conn.restore(MAIN_DB, file, None::<fn(Progress)>)?;
        self.conn
            .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
        let version: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version < latest {
            self.migrate(version)?;
        }
        prune(&self.paths.backups())?;
        Ok(safety)
    }

    /// Copies the whole database to a new backup file.
    fn write_backup(&self, reason: BackupReason) -> Result<PathBuf> {
        let dir = self.paths.backups();
        fs::create_dir_all(&dir).map_err(Error::io("creating", &dir))?;

        // Two backups within the same millisecond would share a name and the
        // second would replace the first; wait for the clock to move instead.
        let (name, path) = loop {
            let name = format!(
                "{PREFIX}{}-{reason}{EXTENSION}",
                Utc::now().format(STAMP_FORMAT)
            );
            let path = dir.join(&name);
            if !path.exists() {
                break (name, path);
            }
            std::thread::yield_now();
        };
        // Written under a temp name so a crash never leaves a truncated
        // file that looks like a valid backup.
        let tmp = dir.join(format!(".{name}.tmp"));
        self.conn.backup(MAIN_DB, &tmp, None::<fn(Progress)>)?;
        fs::rename(&tmp, &path).map_err(Error::io("renaming", &tmp))?;
        Ok(path)
    }

    /// Existing backups, newest first.
    pub fn backups(&self) -> Result<Vec<BackupFile>> {
        list(&self.paths.backups())
    }

    /// Backs up before a wish counted by hand, unless one was taken for
    /// counting less than an hour ago. Every other write backs up for its own
    /// reason, so only hand-counted wishes (each undoable) can be missing from
    /// that backup.
    pub(super) fn backup_for_counting(&self) -> Result<Option<PathBuf>> {
        self.backup_hourly(BackupReason::BeforeCounting)
    }

    /// Before an edit of the plan: at most one backup an hour, as for counting.
    pub(super) fn backup_for_planning(&self) -> Result<Option<PathBuf>> {
        self.backup_hourly(BackupReason::BeforePlanning)
    }

    /// A backup for `reason`, unless one for that reason was taken less than
    /// an hour ago and no other kind of write (an import…) has backed up
    /// since. The other hourly reason doesn't count: counting by hand and
    /// editing the plan in turn would otherwise back up at every click.
    fn backup_hourly(&self, reason: BackupReason) -> Result<Option<PathBuf>> {
        let newest = (self.backups()?.into_iter())
            .find(|b| b.reason == Some(reason) || !b.reason.is_some_and(BackupReason::hourly));
        let newest = newest.filter(|b| b.reason == Some(reason));
        let covered =
            newest.is_some_and(|b| Utc::now() - b.taken_at.to_utc() < COUNTING_BACKUP_INTERVAL);
        if covered {
            Ok(None)
        } else {
            self.backup(reason).map(Some)
        }
    }

    /// Takes a daily backup if the newest backup is more than a day old.
    pub(super) fn backup_if_due(&self) -> Result<Option<PathBuf>> {
        let newest = self.backups()?.into_iter().next();
        let due = newest.is_none_or(|b| Utc::now() - b.taken_at.to_utc() >= TimeDelta::days(1));
        if due {
            self.backup(BackupReason::Daily).map(Some)
        } else {
            Ok(None)
        }
    }
}

fn list(dir: &Path) -> Result<Vec<BackupFile>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error::io("listing", dir)(e)),
    };

    let mut backups = Vec::new();
    for entry in entries {
        let entry = entry.map_err(Error::io("listing", dir))?;
        let name = entry.file_name();
        if let Some((taken_at, reason)) = name.to_str().and_then(parse_name) {
            backups.push(BackupFile {
                path: entry.path(),
                taken_at,
                reason,
            });
        }
    }
    // Newest first, by the time taken (old local names and UTC ones mix);
    // the names, with their milliseconds, break ties.
    backups.sort_by(|a, b| (b.taken_at, &b.path).cmp(&(a.taken_at, &a.path)));
    Ok(backups)
}

fn parse_name(name: &str) -> Option<(DateTime<Local>, Option<BackupReason>)> {
    let rest = name.strip_prefix(PREFIX)?.strip_suffix(EXTENSION)?;
    let stamp = rest.get(..STAMP_LEN)?;
    let naive = NaiveDateTime::parse_from_str(stamp, "%Y%m%d-%H%M%S-%3f").ok()?;
    let (taken_at, rest) = match rest.get(STAMP_LEN..)?.strip_prefix('Z') {
        Some(rest) => (Utc.from_utc_datetime(&naive).with_timezone(&Local), rest),
        // An older name, in local time. An hour that came twice (summer time
        // ending) reads as the first; one that never was, as UTC.
        None => (
            Local
                .from_local_datetime(&naive)
                .earliest()
                .unwrap_or_else(|| Utc.from_utc_datetime(&naive).with_timezone(&Local)),
            rest.get(STAMP_LEN..)?,
        ),
    };
    let reason = rest.strip_prefix('-').and_then(|r| r.parse().ok());
    Some((taken_at, reason))
}

/// Deletes all but the newest [`KEEP_HOURLY_BACKUPS`] hourly backups and
/// the newest [`KEEP_BACKUPS`] others.
fn prune(dir: &Path) -> Result<()> {
    let (mut hourly, mut others) = (0, 0);
    for backup in list(dir)? {
        let (seen, keep) = if backup.reason.is_some_and(BackupReason::hourly) {
            (&mut hourly, KEEP_HOURLY_BACKUPS)
        } else {
            (&mut others, KEEP_BACKUPS)
        };
        *seen += 1;
        if *seen > keep {
            fs::remove_file(&backup.path).map_err(Error::io("deleting", &backup.path))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppPaths;

    #[test]
    fn names_parse_back() {
        let (taken_at, reason) =
            parse_name("wishes-20261109-115400-123Z-before-import.db").unwrap();
        assert_eq!(taken_at.to_utc().to_string(), "2026-11-09 11:54:00.123 UTC");
        assert_eq!(reason, Some(BackupReason::BeforeImport));

        // A name from before the times were UTC: local time, no `Z`.
        let (taken_at, reason) = parse_name("wishes-20261109-115400-123-daily.db").unwrap();
        assert_eq!(
            taken_at.naive_local().to_string(),
            "2026-11-09 11:54:00.123"
        );
        assert_eq!(reason, Some(BackupReason::Daily));

        assert!(parse_name("wishes.db").is_none());
        assert!(parse_name(".wishes-20261109-115400-123Z-daily.db.tmp").is_none());
    }

    #[test]
    fn hourly_backups_never_push_out_the_others() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(AppPaths::at(dir.path())).unwrap();
        let import = store.backup(BackupReason::BeforeImport).unwrap();
        for _ in 0..KEEP_HOURLY_BACKUPS + 5 {
            store.backup(BackupReason::BeforePlanning).unwrap();
            store.backup(BackupReason::BeforeCounting).unwrap();
        }
        let backups = store.backups().unwrap();
        assert!(backups.iter().any(|b| b.path == import));
        let hourly = backups
            .iter()
            .filter(|b| b.reason.is_some_and(BackupReason::hourly));
        assert_eq!(hourly.count(), KEEP_HOURLY_BACKUPS);
    }

    #[test]
    fn counting_and_planning_in_turn_back_up_once_an_hour_each() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(AppPaths::at(dir.path())).unwrap();
        assert!(store.backup_for_counting().unwrap().is_some());
        assert!(store.backup_for_planning().unwrap().is_some());
        assert!(store.backup_for_counting().unwrap().is_none());
        assert!(store.backup_for_planning().unwrap().is_none());
    }

    #[test]
    fn old_local_names_and_utc_ones_sort_by_time() {
        let dir = tempfile::tempdir().unwrap();
        let backups = dir.path().join("backups");
        fs::create_dir_all(&backups).unwrap();
        // Written by the earlier versions, in local time, long ago.
        let old = backups.join("wishes-20200101-120000-000-before-import.db");
        fs::write(&old, b"").unwrap();
        let store = Store::open(AppPaths::at(dir.path())).unwrap();
        let new = store.backup(BackupReason::Manual).unwrap();
        let listed: Vec<_> = store
            .backups()
            .unwrap()
            .into_iter()
            .map(|b| b.path)
            .collect();
        assert_eq!(listed.first(), Some(&new));
        assert_eq!(listed.last(), Some(&old));
    }

    #[test]
    fn backups_are_valid_databases_and_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(AppPaths::at(dir.path())).unwrap();

        let first = store.backup(BackupReason::Manual).unwrap();
        {
            // Closed before pruning: Windows can't delete a file still open.
            let copy = rusqlite::Connection::open(&first).unwrap();
            let version: i64 = copy
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            assert_eq!(version, super::super::MIGRATIONS.len() as i64);
        }

        for _ in 0..KEEP_BACKUPS + 3 {
            store.backup(BackupReason::Manual).unwrap();
        }
        let backups = store.backups().unwrap();
        assert_eq!(backups.len(), KEEP_BACKUPS);
        assert!(
            !backups.iter().any(|b| b.path == first),
            "oldest backup should be pruned"
        );
    }
}
