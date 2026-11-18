//! Applying an import batch, undoing one, and the import log.
//!
//! An import only adds: wishes whose id is stored are left as they are (a
//! different content is reported, never written), and every row it adds
//! carries its `import_id`, so [`Store::undo_import`] can take exactly those
//! out again.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, SecondsFormat};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{BackupReason, Store, WISH_COLUMNS, account_from_row, merge, wish_from_row};
use crate::error::{Error, IoAction, Result};
use crate::import::{
    AccountBatch, AccountReport, ImportBatch, ImportRecord, ImportReport, ImportSource,
    OriginalFile, PoolCounts, UndoReport, WishLink,
};
use crate::model::{Account, AccountId, GachaType, ManualCounters, Pool, Source, Wish, WishTime};
use crate::paths::write_atomic;

impl Store {
    /// The latest imports, newest first.
    pub fn recent_imports(&self, limit: u32) -> Result<Vec<ImportRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT i.id, i.account_id, a.label, i.source, i.detail, i.imported_at, i.added, i.undone_at \
             FROM imports i JOIN accounts a ON a.id = i.account_id ORDER BY i.id DESC LIMIT ?1",
        )?;
        let imports = stmt
            .query_map([limit], |row| {
                Ok(ImportRecord {
                    id: row.get(0)?,
                    account: row.get(1)?,
                    account_label: row.get(2)?,
                    source: row.get(3)?,
                    detail: row.get(4)?,
                    imported_at: row.get(5)?,
                    added: row.get(6)?,
                    undone_at: row.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(imports)
    }

    /// Removes what one import added: its wishes, the game ids it recorded
    /// and its manual counters. A backup is taken first; the import stays in
    /// the log, marked as undone.
    pub fn undo_import(&mut self, import_id: i64) -> Result<UndoReport> {
        let undone: Option<(AccountId, Option<String>)> = self
            .conn
            .query_row(
                "SELECT account_id, undone_at FROM imports WHERE id = ?1",
                [import_id],
                |row| Ok((AccountId(row.get(0)?), row.get(1)?)),
            )
            .optional()?;
        let account = match undone {
            None => return Err(Error::UnknownImport(import_id)),
            Some((_, Some(_))) => return Err(Error::AlreadyUndone(import_id)),
            Some((account, None)) => account,
        };

        let backup = self.backup(BackupReason::BeforeUndo)?;
        let now = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
        let tx = self.conn.transaction()?;
        // Links recorded by this import, then links (from later imports) on
        // the wishes about to go, which would otherwise dangle.
        let mut removed_links =
            tx.execute("DELETE FROM wish_links WHERE import_id = ?1", [import_id])?;
        removed_links += tx.execute(
            "DELETE FROM wish_links WHERE (account_id, wish_id) IN \
             (SELECT account_id, id FROM wishes WHERE import_id = ?1)",
            [import_id],
        )?;
        let removed_wishes = tx.execute("DELETE FROM wishes WHERE import_id = ?1", [import_id])?;
        let removed_counters = tx.execute(
            "DELETE FROM manual_counters WHERE import_id = ?1",
            [import_id],
        )?;
        tx.execute(
            "UPDATE imports SET undone_at = ?1 WHERE id = ?2",
            params![now, import_id],
        )?;
        tx.commit()?;
        self.changed(account);

        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Ok(UndoReport {
            backup,
            removed_wishes: count(removed_wishes),
            removed_links: count(removed_links),
            removed_counters: count(removed_counters),
        })
    }

    /// Writes a batch: backup first, then everything in one transaction.
    ///
    /// Wishes whose id is already stored are left untouched, so applying the
    /// same batch twice adds nothing. Existing data is never modified, except
    /// for filling in an account's missing UID or server.
    pub fn apply_import(&mut self, batch: ImportBatch) -> Result<ImportReport> {
        // A store with no account yet has nothing to back up; one with only
        // characters or a plan does.
        let backup = if self.wish_count()? == 0 && self.accounts()?.is_empty() {
            None
        } else if batch.source == ImportSource::Manual {
            self.backup_for_counting()?
        } else {
            Some(self.backup(BackupReason::BeforeImport)?)
        };
        let archived = batch
            .original
            .as_ref()
            .map(|file| archive_original(&self.paths.imports(), file))
            .transpose()?;

        let now = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
        let mut warnings = batch.warnings;
        let mut accounts = Vec::with_capacity(batch.accounts.len());

        let tx = self.conn.transaction()?;
        for incoming in &batch.accounts {
            let (account, created) = resolve_account(&tx, incoming, &now)?;
            tx.execute(
                "INSERT INTO imports (account_id, source, detail, imported_at, added) \
                 VALUES (?1, ?2, ?3, ?4, 0)",
                params![account.id, batch.source, batch.detail, now],
            )?;
            let import_id = tx.last_insert_rowid();

            let pools = insert_wishes(&tx, &account, import_id, &incoming.wishes, &mut warnings)?;
            let added: u32 = pools.values().map(|c| c.added).sum();
            tx.execute(
                "UPDATE imports SET added = ?1 WHERE id = ?2",
                params![added, import_id],
            )?;

            for counters in &incoming.manual_counters {
                insert_manual_counters(&tx, &account, import_id, counters, &mut warnings)?;
            }
            let linked = insert_links(&tx, &account, import_id, &incoming.links, &mut warnings)?;
            let extras = merge::merge_extras(&tx, account.id, &incoming.extras, &now)?;
            accounts.push(AccountReport {
                account,
                created,
                pools,
                linked,
                extras,
            });
        }
        tx.commit()?;
        for report in &accounts {
            self.changed(report.account.id);
        }

        Ok(ImportReport {
            backup,
            archived,
            accounts,
            warnings,
        })
    }
}

/// Finds the local account for `incoming`, creating it if there is none:
/// the account chosen by the user (`merge_into`), else the same UID, else an
/// account with the same label and no UID yet.
fn resolve_account(
    tx: &Transaction<'_>,
    incoming: &AccountBatch,
    now: &str,
) -> Result<(Account, bool)> {
    let find = |condition: &str, value: &str| -> Result<Option<Account>> {
        let sql = format!(
            "SELECT id, label, uid, server FROM accounts WHERE {condition} ORDER BY id LIMIT 1"
        );
        Ok(tx.query_row(&sql, [value], account_from_row).optional()?)
    };
    let claim = |mut account: Account, uid: &str| -> Result<Account> {
        tx.execute(
            "UPDATE accounts SET uid = ?1 WHERE id = ?2",
            params![uid, account.id],
        )?;
        account.uid = Some(uid.to_owned());
        Ok(account)
    };

    let existing = match (incoming.merge_into, incoming.uid.as_deref()) {
        (Some(target), uid) => {
            let account =
                find("id = ?1", &target.to_string())?.ok_or(Error::UnknownAccount(target))?;
            match (account.uid.as_deref(), uid) {
                (Some(theirs), Some(ours)) if theirs != ours => {
                    return Err(Error::AccountMismatch {
                        account: account.label,
                        uid: ours.to_owned(),
                    });
                }
                (None, Some(ours)) => Some(claim(account, ours)?),
                _ => Some(account),
            }
        }
        (None, Some(uid)) => match find("uid = ?1", uid)? {
            Some(account) => Some(account),
            None => match find("uid IS NULL AND label = ?1", &incoming.label)? {
                Some(account) => Some(claim(account, uid)?),
                None => None,
            },
        },
        (None, None) => find("label = ?1", &incoming.label)?,
    };

    if let Some(mut account) = existing {
        if account.server.is_none() && incoming.server.is_some() {
            tx.execute(
                "UPDATE accounts SET server = ?1 WHERE id = ?2",
                params![incoming.server, account.id],
            )?;
            account.server = incoming.server;
        }
        return Ok((account, false));
    }

    tx.execute(
        "INSERT INTO accounts (label, uid, server, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![incoming.label, incoming.uid, incoming.server, now],
    )?;
    let account = Account {
        id: AccountId(tx.last_insert_rowid()),
        label: incoming.label.clone(),
        uid: incoming.uid.clone(),
        server: incoming.server,
    };
    Ok((account, true))
}

/// Inserts wishes whose id is not stored yet. A stored wish with the same id
/// but different content is kept as is, and reported.
fn insert_wishes(
    tx: &Transaction<'_>,
    account: &Account,
    import_id: i64,
    wishes: &[Wish],
    warnings: &mut Vec<String>,
) -> Result<BTreeMap<Pool, PoolCounts>> {
    // ON CONFLICT DO NOTHING only covers the primary key; unlike INSERT OR
    // IGNORE, a CHECK failure stays an error instead of silently dropping a row.
    let mut insert = tx.prepare_cached(
        "INSERT INTO wishes (account_id, id, gacha_type, time, seq, name, item_type, rarity, \
                             item_key, manual_pity, source, import_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12) \
         ON CONFLICT (account_id, id) DO NOTHING",
    )?;
    let mut stored = tx.prepare_cached(&format!(
        "SELECT {WISH_COLUMNS} FROM wishes WHERE account_id = ?1 AND id = ?2"
    ))?;
    let mut same_id =
        tx.prepare_cached("SELECT 1 FROM wishes WHERE account_id = ?1 AND id = ?2")?;

    // A migrated wish carries no game id, so it can't be matched by id with
    // the game's record of the same wish, stored by an earlier game import.
    // Match it by content instead: same pool, time and item, counted as a
    // multiset since a 10-pull can hold the same 3★ twice. Only wishes
    // stored before this batch count.
    let mut from_game: HashMap<(Pool, WishTime, String), u32> = HashMap::new();
    if wishes.iter().any(|w| w.source == Source::Paimon) {
        let mut select = tx.prepare_cached(
            "SELECT gacha_type, time, name FROM wishes WHERE account_id = ?1 AND source = 'api'",
        )?;
        let rows = select.query_map([account.id], |row| {
            Ok((row.get::<_, GachaType>(0)?.pool(), row.get(1)?, row.get(2)?))
        })?;
        for row in rows {
            *from_game.entry(row?).or_default() += 1;
        }
    }

    let mut pools: BTreeMap<Pool, PoolCounts> = BTreeMap::new();
    let mut conflicts: BTreeMap<Pool, u32> = BTreeMap::new();
    for wish in wishes {
        if wish.source == Source::Paimon
            && !same_id.exists(params![account.id, wish.id])?
            && let Some(left) = from_game
                .get_mut(&(wish.pool(), wish.time, wish.name.clone()))
                .filter(|left| **left > 0)
        {
            *left -= 1;
            pools.entry(wish.pool()).or_default().already_present += 1;
            continue;
        }
        let inserted = insert.execute(params![
            account.id,
            wish.id,
            wish.gacha_type,
            wish.time,
            wish.seq,
            wish.name,
            wish.item_type,
            wish.rarity,
            wish.item_key,
            wish.manual_pity,
            wish.source,
            import_id,
        ])?;
        let counts = pools.entry(wish.pool()).or_default();
        if inserted == 1 {
            counts.added += 1;
        } else {
            counts.already_present += 1;
            let existing = stored.query_row(params![account.id, wish.id], wish_from_row)?;
            if !same_record(&existing, wish) {
                *conflicts.entry(wish.pool()).or_default() += 1;
            }
        }
    }

    for (pool, count) in conflicts {
        warnings.push(t!(
            "import-conflicts",
            account = account.label.as_str(),
            pool = pool.display_name(),
            count = count,
        ));
    }
    Ok(pools)
}

/// Whether two records describe the same wish. `source` is ignored: the same
/// wish can arrive from a snapshot and from the API.
fn same_record(a: &Wish, b: &Wish) -> bool {
    (
        a.gacha_type,
        a.time,
        a.seq,
        &a.name,
        a.item_type,
        a.rarity,
        &a.item_key,
        a.manual_pity,
    ) == (
        b.gacha_type,
        b.time,
        b.seq,
        &b.name,
        b.item_type,
        b.rarity,
        &b.item_key,
        b.manual_pity,
    )
}

fn insert_manual_counters(
    tx: &Transaction<'_>,
    account: &Account,
    import_id: i64,
    counters: &ManualCounters,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let inserted = tx.execute(
        "INSERT INTO manual_counters (account_id, pool, total, pity5, pity4, import_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT (account_id, pool) DO NOTHING",
        params![
            account.id,
            counters.pool,
            counters.total,
            counters.pity5,
            counters.pity4,
            import_id
        ],
    )?;
    if inserted == 0 {
        let existing: (u32, u32, u32) = tx.query_row(
            "SELECT total, pity5, pity4 FROM manual_counters WHERE account_id = ?1 AND pool = ?2",
            params![account.id, counters.pool],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if existing != (counters.total, counters.pity5, counters.pity4) {
            warnings.push(t!(
                "import-counters-kept",
                account = account.label.as_str(),
                pool = counters.pool.display_name(),
                stored_total = existing.0,
                stored_five = existing.1,
                stored_four = existing.2,
                total = counters.total,
                five = counters.pity5,
                four = counters.pity4,
            ));
        }
    }
    Ok(())
}

/// Records game ids for stored wishes. A wish that already has one, or an id
/// already used by another wish, is left alone; a different id is reported.
fn insert_links(
    tx: &Transaction<'_>,
    account: &Account,
    import_id: i64,
    links: &[WishLink],
    warnings: &mut Vec<String>,
) -> Result<u32> {
    let mut insert = tx.prepare_cached(
        "INSERT INTO wish_links (account_id, wish_id, api_id, import_id) VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT DO NOTHING",
    )?;
    let mut linked = 0;
    let mut conflicts = 0;
    for link in links {
        if insert.execute(params![account.id, link.wish_id, link.api_id, import_id])? == 1 {
            linked += 1;
        } else {
            let existing: Option<String> = tx
                .query_row(
                    "SELECT api_id FROM wish_links WHERE account_id = ?1 AND wish_id = ?2",
                    params![account.id, link.wish_id],
                    |row| row.get(0),
                )
                .optional()?;
            if existing.as_deref() != Some(link.api_id.as_str()) {
                conflicts += 1;
            }
        }
    }
    if conflicts > 0 {
        warnings.push(t!(
            "import-links-kept",
            account = account.label.as_str(),
            count = conflicts,
        ));
    }
    Ok(linked)
}

/// Keeps a copy of an imported file as received, under `imports/`.
/// Importing the same file again reuses the existing copy.
fn archive_original(dir: &Path, file: &OriginalFile) -> Result<PathBuf> {
    let name = Path::new(&file.name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "import".to_owned());
    if let Some(existing) = find_archived(dir, &name, &file.bytes)? {
        return Ok(existing);
    }
    let stamp = Local::now().format("%Y%m%d-%H%M%S-%3f");
    let path = dir.join(format!("{stamp}-{name}"));
    write_atomic(&path, &file.bytes)?;
    Ok(path)
}

fn find_archived(dir: &Path, name: &str, bytes: &[u8]) -> Result<Option<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(IoAction::Listing, dir)(e)),
    };
    let suffix = format!("-{name}");
    for entry in entries {
        let entry = entry.map_err(Error::io(IoAction::Listing, dir))?;
        let path = entry.path();
        let same_name = entry.file_name().to_string_lossy().ends_with(&suffix);
        let same_size = entry
            .metadata()
            .is_ok_and(|m| m.len() == bytes.len() as u64);
        if same_name
            && same_size
            && fs::read(&path).map_err(Error::io(IoAction::Reading, &path))? == bytes
        {
            return Ok(Some(path));
        }
    }
    Ok(None)
}
