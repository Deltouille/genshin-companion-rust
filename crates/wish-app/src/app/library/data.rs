//! The data itself: imports (from a file, the game, HoYoLAB), undoing one,
//! exports, backups and accounts.

use super::*;

impl Library {
    /// Adds an account by hand: a name, and optionally its UID and server.
    pub fn add_account(
        &mut self,
        label: String,
        uid: Option<String>,
        server: Option<Server>,
        cx: &mut Context<Self>,
    ) -> Task<Result<Account>> {
        // The mirror's snapshot lists the accounts.
        self.write(
            cx,
            true,
            move |store, _| Ok(store.add_account(&label, uid.as_deref(), server)?),
            |_, account| account,
        )
    }

    /// Gives an account another name.
    pub fn rename_account(
        &mut self,
        account: AccountId,
        label: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.write(
            cx,
            true,
            move |store, _| Ok(store.rename_account(account, &label)?),
            |_, ()| (),
        )
    }

    /// Renames an account and, for one without a UID, gives it its UID or
    /// server (`identity`), in one write. The UID's problems (taken, not a
    /// UID) are found before the name changes.
    pub fn edit_account(
        &mut self,
        account: AccountId,
        label: String,
        identity: Option<(Option<String>, Option<Server>)>,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.write(
            cx,
            true,
            move |store, _| {
                if let Some((uid, server)) = identity {
                    store.identify_account(account, uid.as_deref(), server)?;
                }
                Ok(store.rename_account(account, &label)?)
            },
            |_, ()| (),
        )
    }

    /// Removes an account without wishes, with what it owns and plans,
    /// after a backup.
    pub fn remove_account(
        &mut self,
        account: AccountId,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.write(
            cx,
            true,
            move |store, _| Ok(store.remove_account(account)?),
            |_, ()| (),
        )
    }

    /// Reads and checks a file for import. Nothing is written.
    /// A spreadsheet without a UID goes to `target`, the account on screen.
    pub fn prepare_import(
        &self,
        path: PathBuf,
        target: Option<AccountId>,
        cx: &mut Context<Self>,
    ) -> Task<Result<PreparedImport>> {
        let (Some(store), Some(data)) = (self.store.clone(), self.data.clone()) else {
            return Task::ready(Err(not_open()));
        };
        cx.background_spawn(async move {
            // The player reads what failed, in their language.
            let reading = |source| wish_core::Error::Io {
                action: IoAction::Reading,
                path: path.clone(),
                source,
            };
            // A paimon.moe save is a few MB; a far larger file is no export.
            let size = std::fs::metadata(&path).map_err(reading)?.len();
            if size > MAX_IMPORT_BYTES {
                return Err(anyhow!(t!(
                    "app-import-too-large",
                    file = path.display().to_string(),
                    size = wish_ui::format::bytes(size)
                )));
            }
            let bytes = std::fs::read(&path).map_err(reading)?;
            let name = file_name(&path);
            let store = lock(&store);
            Ok(import::prepare(&name, bytes, &data, &store, target)?)
        })
    }

    /// Looks for the wish history link on this computer: in `folder` when
    /// given, else in the remembered game folder, then the usual places.
    pub fn find_game_link(
        &self,
        folder: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) -> Task<Option<FoundLink>> {
        let remembered = self.settings(cx).game_folder.clone();
        cx.background_spawn(async move {
            match folder {
                Some(folder) => game_cache::find_link_in_game_folder(&folder),
                None => remembered
                    .and_then(|folder| game_cache::find_link_in_game_folder(&folder))
                    .or_else(game_cache::find_link),
            }
        })
    }

    /// Deletes wishes entered by hand (after a backup).
    pub fn delete_manual(
        &mut self,
        account: AccountId,
        ids: Vec<WishId>,
        cx: &mut Context<Self>,
    ) -> Task<Result<u32>> {
        self.write(
            cx,
            true,
            move |store, _| Ok(store.delete_manual_wishes(account, &ids)?),
            |_, deleted| deleted,
        )
    }

    /// Fetches the game's history behind `link` and plans its import.
    /// Nothing is written. The fetch stops at the first wish already stored.
    pub fn fetch_from_game(
        &self,
        link: GachaLink,
        control: Arc<FetchControl>,
        cx: &mut Context<Self>,
    ) -> Task<Result<ApiPlan>> {
        let (Some(store), Some(data)) = (self.store.clone(), self.data.clone()) else {
            return Task::ready(Err(not_open()));
        };
        let (transport, pacing) = (self.services.game.clone(), self.services.pacing);
        cx.background_spawn(async move {
            let mut report = |progress: FetchProgress| {
                *lock(&control.progress) = Some(progress);
            };
            Ok(service::game::fetch(
                &link,
                &*transport,
                pacing,
                &store,
                &data,
                &mut report,
                &control.cancelled,
            )?)
        })
    }

    /// Asks HoYoLAB, with the player's `cookie`, for the characters of every
    /// Genshin account behind it, and works out what a sync would change in
    /// each app account it can go to (`shown`, the account on screen, is
    /// suggested when it has no UID). The cookie is dropped as soon as the
    /// requests are done, and nothing is written. The store isn't locked
    /// during the requests.
    pub fn fetch_from_hoyolab(
        &self,
        cookie: hoyolab::Cookie,
        shown: Option<AccountId>,
        control: Arc<FetchControl>,
        cx: &mut Context<Self>,
    ) -> Task<Result<Vec<HoyolabAccount>>> {
        let Some(store) = self.store.clone() else {
            return Task::ready(Err(not_open()));
        };
        let (planner, archive) = (self.planner.clone(), self.archive.clone());
        let transport = self.services.hoyolab.clone();
        cx.background_spawn(async move {
            Ok(service::hoyolab::fetch_preview(
                cookie,
                &*transport,
                &store,
                shown,
                &planner,
                &archive,
                &control.cancelled,
            )?)
        })
    }

    /// Writes a HoYoLAB sync the player confirmed into `target`, the account
    /// they chose (`None`: a new one, named after the game's nickname); an
    /// account without a UID takes the game's, so the next import from the
    /// game goes there too. The plan is worked out again against the stored
    /// collection, then written in one transaction after a backup. Returns
    /// the account and what changed.
    pub fn apply_hoyolab_sync(
        &mut self,
        fetched: HoyolabAccount,
        target: Option<AccountId>,
        cx: &mut Context<Self>,
    ) -> Task<Result<(AccountId, SyncPlan)>> {
        let (planner, archive) = (self.planner.clone(), self.archive.clone());
        // The mirror's snapshot lists the accounts, and one may be made.
        self.write(
            cx,
            true,
            move |store, _| {
                let (account, plan, _) = store.apply_sync(
                    target,
                    &fetched.account,
                    &fetched.details,
                    &planner,
                    &archive,
                )?;
                Ok((account, plan))
            },
            |_, applied| applied,
        )
    }

    /// Reads a GOOD file from `source` and works out what importing it into
    /// `shown`, the account on screen, would change (`None`, no account yet:
    /// a new one). Nothing is written.
    pub fn preview_good(
        &self,
        source: GoodSource,
        shown: Option<AccountId>,
        cx: &mut Context<Self>,
    ) -> Task<Result<GoodPreview>> {
        let Some(store) = self.store.clone() else {
            return Task::ready(Err(not_open()));
        };
        let planner = self.planner.clone();
        cx.background_spawn(async move {
            let file = match source {
                GoodSource::File(path) => {
                    let size = std::fs::metadata(&path)
                        .map_err(|source| wish_core::error::Error::Io {
                            action: IoAction::Reading,
                            path: path.clone(),
                            source,
                        })?
                        .len();
                    if size > MAX_IMPORT_BYTES {
                        return Err(anyhow!(t!(
                            "app-import-too-large",
                            file = path.display().to_string(),
                            size = wish_ui::format::bytes(size)
                        )));
                    }
                    service::good::read(&path)?
                }
                GoodSource::Text(text) => GoodFile::parse(text.as_bytes())?,
            };
            let store = lock(&store);
            Ok(service::good::preview(file, &store, shown, &planner)?)
        })
    }

    /// Writes a GOOD import the player confirmed, into the account its
    /// preview names (a new "Main" when there was none). The plan is worked
    /// out again against what is stored, then written in one transaction
    /// after a backup. Returns the account and what changed.
    pub fn apply_good(
        &mut self,
        preview: GoodPreview,
        cx: &mut Context<Self>,
    ) -> Task<Result<(AccountId, GoodPlan)>> {
        let planner = self.planner.clone();
        let target = preview.account.as_ref().map(|account| account.id);
        // The mirror's snapshot holds the collection and the inventory, and
        // an account may be made.
        self.write(
            cx,
            true,
            move |store, _| {
                let (account, plan, _) = store.apply_good(target, &preview.file, &planner)?;
                Ok((account, plan))
            },
            |_, applied| applied,
        )
    }

    /// Writes a prepared batch (backup first, one transaction), then
    /// recomputes every account and refreshes the mirror folder.
    pub fn apply_import(
        &mut self,
        batch: ImportBatch,
        cx: &mut Context<Self>,
    ) -> Task<Result<ImportReport>> {
        self.write(
            cx,
            true,
            move |store, _| Ok(store.apply_import(batch)?),
            |_, report| report,
        )
    }

    /// Removes what one import added (backup first).
    pub fn undo_import(
        &mut self,
        import_id: i64,
        cx: &mut Context<Self>,
    ) -> Task<Result<UndoReport>> {
        self.write(
            cx,
            true,
            move |store, _| Ok(store.undo_import(import_id)?),
            |_, report| report,
        )
    }

    /// Writes an export to `path`. Read-only for the database.
    pub fn export(
        &self,
        kind: ExportKind,
        path: PathBuf,
        account: Option<AccountId>,
        cx: &mut Context<Self>,
    ) -> Task<Result<String>> {
        let (Some(store), Some(data)) = (self.store.clone(), self.data.clone()) else {
            return Task::ready(Err(not_open()));
        };
        cx.background_spawn(async move {
            let exported = {
                let store = lock(&store);
                match kind {
                    ExportKind::Snapshot => service::export::snapshot(&store)?,
                    ExportKind::Uigf => service::export::uigf(&store, &data)?,
                    ExportKind::Spreadsheet => {
                        let id = account.ok_or_else(|| anyhow!(t!("app-choose-account-first")))?;
                        service::export::spreadsheet(&store, id, &data)?
                    }
                }
            };
            wish_core::paths::write_atomic(&path, &exported.bytes)?;
            Ok(export_message(&exported.summary))
        })
    }

    /// Replaces the whole database (wishes, plan, inventory) with a backup,
    /// after backing up the current state. Returns that safety backup.
    pub fn restore_backup(
        &mut self,
        file: PathBuf,
        cx: &mut Context<Self>,
    ) -> Task<Result<PathBuf>> {
        self.write(
            cx,
            true,
            move |store, _| Ok(store.restore(&file)?),
            |_, safety| safety,
        )
    }

    pub fn back_up(&mut self, cx: &mut Context<Self>) -> Task<Result<PathBuf>> {
        self.write(
            cx,
            false,
            |store, _| Ok(store.backup(BackupReason::Manual)?),
            |_, path| path,
        )
    }
}

/// The largest file an import reads.
const MAX_IMPORT_BYTES: u64 = 64 * 1024 * 1024;

/// What an export holds, in words.
fn export_message(summary: &ExportSummary) -> String {
    match summary {
        ExportSummary::Uigf {
            wishes,
            accounts,
            without_uid,
        } if !without_uid.is_empty() => t!(
            "app-exported-left-out",
            wishes = *wishes,
            accounts = *accounts,
            left_out = without_uid.join(", ")
        ),
        ExportSummary::Snapshot { wishes, accounts }
        | ExportSummary::Uigf {
            wishes, accounts, ..
        } => t!("app-exported", wishes = *wishes, accounts = *accounts),
        ExportSummary::Spreadsheet { account } => {
            t!("app-exported-account", account = account.as_str())
        }
    }
}
