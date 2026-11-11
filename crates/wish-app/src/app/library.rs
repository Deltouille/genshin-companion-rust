//! The user's wish history as the window sees it: the store, the game data,
//! every account's analysis, and the operations that change them.
//!
//! `Library` owns this state; views observe it. Database work runs on the
//! background executor, behind a mutex, one write at a time.

use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow};
use chrono::{DateTime, Local, NaiveDateTime};
use futures_channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures_channel::oneshot;
use futures_util::StreamExt as _;
use gpui_kit::{AppContext as _, Context, SharedString, Subscription, Task};
use wish_core::archive::ArchiveData;
use wish_core::data::GameData;
use wish_core::images::{self, Collection, Remote};

use super::images::{Fetched, Job};
use wish_core::planner::{CustomItem, Inventory, PlanEntry, PlanTarget, PlannerData};

use super::images::Images;
use wish_core::engine::{self, Analysis};
use wish_core::import::api::{self, FetchProgress, GachaLink, HttpTransport, Pacing};
use wish_core::import::api_plan::{self, ApiPlan};
use wish_core::import::game_cache::{self, FoundLink};
use wish_core::import::{
    self, ImportBatch, ImportRecord, ImportReport, PreparedImport, UndoReport,
};
use wish_core::model::{Account, AccountId, WishId};
use wish_core::settings::Settings;
use wish_core::snapshot;
use wish_core::store::{BackupFile, BackupReason};
use wish_core::{AppPaths, Store};

/// How many imports the import log shows.
const RECENT_IMPORTS: u32 = 10;
/// How many images download at once.
const IMAGE_DOWNLOADS: usize = 6;
/// A download of every image gives up after this many failures without a
/// single image: the sites are out of reach.
const GIVE_UP_AFTER: usize = 20;

/// Every image the app can show, by collection.
#[derive(Debug, Default)]
struct ImageCatalog {
    icons: Vec<Remote>,
    everything: Vec<Remote>,
}

/// A download of every image of a collection, for offline use.
#[derive(Clone, Debug)]
pub struct ImageDownload {
    pub collection: Collection,
    /// The images missing when it started.
    pub total: usize,
    /// Of those, the ones done with: downloaded, failed or skipped.
    pub done: usize,
    /// Downloaded.
    pub stored: usize,
    /// Not on their sites.
    pub not_found: usize,
    /// Not downloaded: no connection, a site down…
    pub failed: usize,
    pub state: DownloadState,
    cancel: Arc<AtomicBool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadState {
    Running,
    /// Stop asked: the downloads under way finish.
    Stopping,
    Stopped,
    /// Nothing could be downloaded: no connection, or the sites are down.
    Unreachable,
    Done,
}

impl ImageDownload {
    pub fn is_running(&self) -> bool {
        matches!(self.state, DownloadState::Running | DownloadState::Stopping)
    }
}

pub enum LoadState {
    Loading,
    Ready,
    Failed(SharedString),
}

/// One account with everything derived from its wishes.
#[derive(Clone)]
pub struct AccountSummary {
    pub account: Account,
    pub analysis: Arc<Analysis>,
}

#[derive(Clone, Debug, Default)]
pub struct BackupStatus {
    pub count: usize,
    /// When the newest backup was taken.
    pub newest: Option<DateTime<Local>>,
    /// Every backup, newest first.
    pub files: Vec<BackupFile>,
}

/// Outcome of the latest write to the mirror folder.
#[derive(Clone, Debug)]
pub enum MirrorStatus {
    Written(NaiveDateTime),
    Failed(SharedString),
}

/// What an export writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportKind {
    /// Everything, losslessly, for this app.
    Snapshot,
    /// Accounts with a UID, for other trackers.
    Uigf,
    /// One account, in paimon.moe's spreadsheet layout.
    Spreadsheet,
}

/// Shared between a running fetch and the view that shows it.
#[derive(Debug, Default)]
pub struct FetchControl {
    cancelled: AtomicBool,
    progress: Mutex<Option<FetchProgress>>,
}

impl FetchControl {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn progress(&self) -> Option<FetchProgress> {
        self.progress.lock().ok().and_then(|p| *p)
    }
}

pub struct Library {
    paths: AppPaths,
    state: LoadState,
    store: Option<Arc<Mutex<Store>>>,
    data: Option<Arc<GameData>>,
    /// The planner's costs and the character pages, from genshin-db.
    planner: Arc<PlannerData>,
    archive: Arc<ArchiveData>,
    /// Why a copy of them in the game-data folder was set aside, if one was.
    game_data_warning: Option<SharedString>,
    images: Arc<Images>,
    image_catalog: Arc<ImageCatalog>,
    /// The download of every image running, or the last one.
    image_download: Option<ImageDownload>,
    accounts: Vec<AccountSummary>,
    imports: Vec<ImportRecord>,
    /// The ascension plan, as the player entered it.
    plan: Vec<PlanEntry>,
    /// What the player has, as they entered it.
    inventory: Inventory,
    settings: Settings,
    backups: BackupStatus,
    mirror: Option<MirrorStatus>,
    /// Database writes queued or running.
    writes: usize,
    /// Writes not finished yet, counted where they run: quitting waits for
    /// them, since the app thread that counts `writes` is then blocked.
    unfinished: Arc<AtomicUsize>,
    /// Settings changes made so far, in order.
    settings_changes: u64,
    /// The latest settings change on disk: an older save never overwrites
    /// a newer one.
    settings_saved: Arc<Mutex<u64>>,
    /// Why `settings.json` was set aside, if it was.
    settings_warning: Option<SharedString>,
    _wait_on_quit: Subscription,
    /// Counts writes in the order they take the store lock.
    generations: Arc<AtomicU64>,
    /// The write whose reload is installed.
    loaded_generation: u64,
}

/// Whether the library downloads the images the pages ask for. Tests turn
/// it off: they never touch the network.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageDownloads {
    On,
    Off,
}

impl Library {
    /// Starts opening the database in the background; observers are
    /// notified when it is ready or has failed.
    pub fn open(paths: AppPaths, downloads: ImageDownloads, cx: &mut Context<Self>) -> Self {
        let image_requests = match downloads {
            ImageDownloads::On => {
                let (requests, queue) = mpsc::unbounded();
                Self::download_images(queue, paths.image_cache(), cx);
                Some(requests)
            }
            ImageDownloads::Off => None,
        };
        let task_paths = paths.clone();
        let unfinished = Arc::new(AtomicUsize::new(0));
        cx.spawn(async move |this, cx| {
            let opened = cx
                .background_spawn(async move { open(task_paths, image_requests) })
                .await;
            this.update(cx, |this, cx| {
                match opened {
                    Ok(opened) => {
                        this.store = Some(Arc::new(Mutex::new(opened.store)));
                        this.data = Some(Arc::new(opened.data));
                        this.planner = Arc::new(opened.planner);
                        this.archive = Arc::new(opened.archive);
                        this.game_data_warning = opened.game_data_warning.map(Into::into);
                        this.images = Arc::new(opened.images);
                        this.image_catalog = Arc::new(opened.image_catalog);
                        this.settings = opened.settings;
                        this.settings_warning = opened.settings_warning.map(Into::into);
                        this.set_loaded(opened.loaded);
                        this.state = LoadState::Ready;
                    }
                    Err(err) => this.state = LoadState::Failed(format!("{err:#}").into()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();

        Self {
            paths,
            state: LoadState::Loading,
            store: None,
            data: None,
            planner: Arc::new(PlannerData::embedded().clone()),
            archive: Arc::new(ArchiveData::embedded().clone()),
            game_data_warning: None,
            images: Arc::new(Images::default()),
            image_catalog: Arc::default(),
            image_download: None,
            accounts: Vec::new(),
            imports: Vec::new(),
            plan: Vec::new(),
            inventory: Inventory::default(),
            settings: Settings::default(),
            backups: BackupStatus::default(),
            mirror: None,
            writes: 0,
            unfinished: unfinished.clone(),
            settings_changes: 0,
            settings_saved: Arc::default(),
            settings_warning: None,
            _wait_on_quit: cx.on_app_quit(move |_, _| {
                // A write cut off by quitting is rolled back (one transaction
                // each), but the change is lost: give it a few seconds.
                let since = Instant::now();
                while unfinished.load(Ordering::SeqCst) > 0 && since.elapsed() < QUIT_WAIT {
                    std::thread::sleep(Duration::from_millis(10));
                }
                async {}
            }),
            generations: Arc::default(),
            loaded_generation: 0,
        }
    }

    /// Downloads the images the pages ask for, a few at a time, and redraws
    /// as they arrive. A failed download leaves its place empty until the
    /// next launch.
    fn download_images(queue: UnboundedReceiver<Job>, cache: PathBuf, cx: &mut Context<Self>) {
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let mut downloads = queue
                .map(move |job| {
                    let cache = cache.clone();
                    executor.spawn(async move {
                        let fetched = job.run(&cache);
                        (job, fetched)
                    })
                })
                .buffer_unordered(IMAGE_DOWNLOADS);
            while let Some((job, fetched)) = downloads.next().await {
                let open = this.update(cx, |library, cx| {
                    library.images.finished(job.file(), fetched);
                    cx.notify();
                });
                if open.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// The images of `collection` not in the cache yet: how many, and about
    /// how big.
    pub fn missing_images(&self, collection: Collection) -> (usize, u64) {
        let catalog = match collection {
            Collection::Icons => &self.image_catalog.icons,
            Collection::Everything => &self.image_catalog.everything,
        };
        (catalog.iter().filter(|remote| self.images.missing(remote)))
            .fold((0, 0), |(count, bytes), remote| {
                (count + 1, bytes + remote.typical_bytes())
            })
    }

    /// The download of every image running, or the last one.
    pub fn image_download(&self) -> Option<&ImageDownload> {
        self.image_download.as_ref()
    }

    /// Downloads every image of `collection` missing from the cache, for
    /// offline use: a few at a time, like a page's. Gives up when nothing
    /// can be reached.
    pub fn download_all_images(&mut self, collection: Collection, cx: &mut Context<Self>) {
        let running = self
            .image_download
            .as_ref()
            .is_some_and(ImageDownload::is_running);
        if running || !self.images.downloads() {
            return;
        }
        let catalog = match collection {
            Collection::Icons => &self.image_catalog.icons,
            Collection::Everything => &self.image_catalog.everything,
        };
        let missing: Vec<Remote> = (catalog.iter().filter(|remote| self.images.missing(remote)))
            .cloned()
            .collect();
        let cancel = Arc::new(AtomicBool::new(false));
        self.image_download = Some(ImageDownload {
            collection,
            total: missing.len(),
            done: 0,
            stored: 0,
            not_found: 0,
            failed: 0,
            state: DownloadState::Running,
            cancel: cancel.clone(),
        });
        cx.notify();

        let shared = self.images.clone();
        let cache = self.paths.image_cache();
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let mut downloads = futures_util::stream::iter(missing)
                .map(move |remote| {
                    let (shared, cache, cancel) = (shared.clone(), cache.clone(), cancel.clone());
                    executor.spawn(async move {
                        // Skipped once stopped, and when a page has it.
                        if cancel.load(Ordering::Relaxed) || !shared.claim(&remote) {
                            return None;
                        }
                        let fetched = Fetched::of(images::download(&remote, &cache));
                        shared.finished(&remote.file, fetched);
                        Some(fetched)
                    })
                })
                .buffer_unordered(IMAGE_DOWNLOADS);
            while let Some(fetched) = downloads.next().await {
                let open = this.update(cx, |library, cx| {
                    if let Some(download) = &mut library.image_download {
                        download.done += 1;
                        match fetched {
                            Some(Fetched::NotFound) => download.not_found += 1,
                            Some(Fetched::Failed) => download.failed += 1,
                            Some(Fetched::Stored(_)) => download.stored += 1,
                            None => {}
                        }
                        let nothing =
                            download.failed >= GIVE_UP_AFTER && download.failed == download.done;
                        if nothing && download.state == DownloadState::Running {
                            download.cancel.store(true, Ordering::Relaxed);
                            download.state = DownloadState::Unreachable;
                        }
                    }
                    cx.notify();
                });
                if open.is_err() {
                    return;
                }
            }
            this.update(cx, |library, cx| {
                if let Some(download) = &mut library.image_download {
                    download.state = match download.state {
                        DownloadState::Running => DownloadState::Done,
                        DownloadState::Stopping => DownloadState::Stopped,
                        other => other,
                    };
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Stops the download of every image; the images under way finish.
    pub fn stop_image_download(&mut self, cx: &mut Context<Self>) {
        if let Some(download) = &mut self.image_download
            && download.state == DownloadState::Running
        {
            download.cancel.store(true, Ordering::Relaxed);
            download.state = DownloadState::Stopping;
            cx.notify();
        }
    }

    /// Deletes the downloaded images; pages download them again as they show
    /// them. Nothing in the cache is the player's.
    pub fn clear_image_cache(&mut self, cx: &mut Context<Self>) -> Task<Result<()>> {
        if self
            .image_download
            .as_ref()
            .is_some_and(ImageDownload::is_running)
        {
            return Task::ready(Err(anyhow!("images are downloading: stop that first")));
        }
        let cache = self.paths.image_cache();
        cx.spawn(async move |this, cx| {
            cx.background_spawn(async move {
                match std::fs::remove_dir_all(&cache) {
                    Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                        Err(anyhow::Error::from(err)
                            .context(format!("deleting {}", cache.display())))
                    }
                    _ => Ok(()),
                }
            })
            .await?;
            this.update(cx, |library, cx| {
                library.images.forget();
                library.image_download = None;
                cx.notify();
            })?;
            Ok(())
        })
    }

    pub fn state(&self) -> &LoadState {
        &self.state
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn accounts(&self) -> &[AccountSummary] {
        &self.accounts
    }

    pub fn account(&self, id: AccountId) -> Option<&AccountSummary> {
        self.accounts.iter().find(|a| a.account.id == id)
    }

    /// The latest imports, newest first.
    pub fn imports(&self) -> &[ImportRecord] {
        &self.imports
    }

    pub fn data(&self) -> Option<&Arc<GameData>> {
        self.data.as_ref()
    }

    /// The planner's characters, weapons, materials and costs.
    pub fn planner(&self) -> &Arc<PlannerData> {
        &self.planner
    }

    /// The character pages.
    pub fn archive(&self) -> &Arc<ArchiveData> {
        &self.archive
    }

    /// The ascension plan, in order.
    pub fn plan(&self) -> &[PlanEntry] {
        &self.plan
    }

    /// Adds an entry at the end of the plan.
    pub fn add_plan_entry(
        &mut self,
        target: PlanTarget,
        cx: &mut Context<Self>,
    ) -> Task<Result<i64>> {
        self.write(
            cx,
            false,
            move |store, _| Ok(store.add_plan_entry(&target)?),
            |_, id| id,
        )
    }

    /// Saves an entry's new goal, or a custom entry's new name and costs.
    pub fn update_plan_entry(
        &mut self,
        entry: PlanEntry,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        // Shown at once; the reload after the write confirms it.
        if let Some(stored) = self.plan.iter_mut().find(|e| e.id == entry.id) {
            *stored = entry.clone();
        }
        self.write(
            cx,
            false,
            move |store, _| Ok(store.update_plan_entry(&entry)?),
            |_, ()| (),
        )
    }

    /// Moves entry `id` to where entry `onto` is, the entries between
    /// shifting along: the order the inventory is shared out in.
    pub fn move_plan_entry(
        &mut self,
        id: i64,
        onto: i64,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let from = self.plan.iter().position(|entry| entry.id == id);
        let to = self.plan.iter().position(|entry| entry.id == onto);
        let (Some(from), Some(to)) = (from, to) else {
            return Task::ready(Ok(()));
        };
        if from == to {
            return Task::ready(Ok(()));
        }
        // Shown at once; the reload after the write confirms it.
        let entry = self.plan.remove(from);
        self.plan.insert(to, entry);
        cx.notify();
        let ids: Vec<i64> = self.plan.iter().map(|entry| entry.id).collect();
        self.write(
            cx,
            false,
            move |store, _| Ok(store.reorder_plan(&ids)?),
            |_, ()| (),
        )
    }

    pub fn remove_plan_entry(&mut self, id: i64, cx: &mut Context<Self>) -> Task<Result<()>> {
        self.write(
            cx,
            false,
            move |store, _| Ok(store.remove_plan_entry(id)?),
            |_, ()| (),
        )
    }

    /// What the player has, as they entered it.
    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    /// Records how many of `item` the player has.
    pub fn set_owned(
        &mut self,
        item: CustomItem,
        count: u64,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.set_owned_many(vec![(item, count)], cx)
    }

    /// Records several counts at once (the tiers of a series), in one write.
    pub fn set_owned_many(
        &mut self,
        counts: Vec<(CustomItem, u64)>,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        // Shown at once; the reload after the write confirms it.
        for (item, count) in &counts {
            self.inventory.set(item, *count);
        }
        cx.notify();
        self.write(
            cx,
            false,
            move |store, _| Ok(store.set_owned_many(&counts)?),
            |_, ()| (),
        )
    }

    /// Set when a copy of the planner or archive data in the game-data folder
    /// could not be read, and the built-in data is used instead.
    pub fn game_data_warning(&self) -> Option<&SharedString> {
        self.game_data_warning.as_ref()
    }

    /// Icons and banner art found on disk.
    pub fn images(&self) -> &Arc<Images> {
        &self.images
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn backups(&self) -> &BackupStatus {
        &self.backups
    }

    pub fn mirror(&self) -> Option<&MirrorStatus> {
        self.mirror.as_ref()
    }

    pub fn is_busy(&self) -> bool {
        self.writes > 0
    }

    /// Why `settings.json` was set aside at startup, if it was.
    pub fn settings_warning(&self) -> Option<&SharedString> {
        self.settings_warning.as_ref()
    }

    /// Changes the settings at once, then saves them in the background. Each
    /// change builds on the ones before, saved or not, and the saves are
    /// written one at a time, the latest last.
    fn change_settings(
        &mut self,
        change: impl FnOnce(&mut Settings),
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        change(&mut self.settings);
        cx.notify();
        self.settings_changes += 1;
        let (settings, paths) = (self.settings.clone(), self.paths.clone());
        let (this_change, saved) = (self.settings_changes, self.settings_saved.clone());
        cx.background_spawn(async move {
            let mut latest = saved.lock().unwrap_or_else(PoisonError::into_inner);
            if *latest < this_change {
                settings.save(&paths)?;
                *latest = this_change;
            }
            Ok(())
        })
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
            return Task::ready(Err(anyhow!("the wish history is not open yet")));
        };
        cx.background_spawn(async move {
            // A paimon.moe save is a few MB; a far larger file is no export.
            let size = std::fs::metadata(&path)
                .with_context(|| format!("reading {}", path.display()))?
                .len();
            if size > MAX_IMPORT_BYTES {
                return Err(anyhow!(
                    "{} is {} MB, too large for a wish history export",
                    path.display(),
                    size / 1_000_000
                ));
            }
            let bytes =
                std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
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
        let remembered = self.settings.game_folder.clone();
        cx.background_spawn(async move {
            match folder {
                Some(folder) => game_cache::find_link_in_game_folder(&folder),
                None => remembered
                    .and_then(|folder| game_cache::find_link_in_game_folder(&folder))
                    .or_else(game_cache::find_link),
            }
        })
    }

    /// Turns the controls to count wishes by hand on or off.
    pub fn set_manual_counting(&mut self, on: bool, cx: &mut Context<Self>) -> Task<Result<()>> {
        self.change_settings(|settings| settings.manual_counting = on, cx)
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

    /// Remembers the game folder the user pointed to.
    pub fn remember_game_folder(
        &mut self,
        folder: PathBuf,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.change_settings(|settings| settings.game_folder = Some(folder), cx)
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
            return Task::ready(Err(anyhow!("the wish history is not open yet")));
        };
        cx.background_spawn(async move {
            let known = lock(&store).known_api_ids()?;
            let is_known = |uid: &str, id: &str| known.get(uid).is_some_and(|ids| ids.contains(id));
            let mut report = |progress: FetchProgress| {
                if let Ok(mut slot) = control.progress.lock() {
                    *slot = Some(progress);
                }
            };
            // The store is not locked during the network requests.
            let fetched = api::fetch(
                &link,
                &HttpTransport::new(),
                Pacing::default(),
                &is_known,
                &mut report,
                &control.cancelled,
            )?;
            let store = lock(&store);
            Ok(api_plan::plan(fetched, &store, &data)?)
        })
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
            return Task::ready(Err(anyhow!("the wish history is not open yet")));
        };
        cx.background_spawn(async move {
            let store = lock(&store);
            match kind {
                ExportKind::Snapshot => {
                    let snapshot = snapshot::Snapshot::from_store(&store)?;
                    snapshot.write_to(&path)?;
                    let wishes: usize = snapshot.accounts.iter().map(|a| a.wishes.len()).sum();
                    Ok(format!(
                        "Exported {wishes} wishes of {} account(s).",
                        snapshot.accounts.len()
                    ))
                }
                ExportKind::Uigf => {
                    let export = wish_core::uigf::export(&store, &data)?;
                    wish_core::paths::write_atomic(&path, &export.json)?;
                    let mut summary = format!(
                        "Exported {} wishes of {} account(s).",
                        export.wishes, export.accounts
                    );
                    if !export.without_uid.is_empty() {
                        summary.push_str(&format!(
                            " Left out, without a UID: {}.",
                            export.without_uid.join(", ")
                        ));
                    }
                    Ok(summary)
                }
                ExportKind::Spreadsheet => {
                    let id = account.ok_or_else(|| anyhow!("choose an account first"))?;
                    let account = store
                        .accounts()?
                        .into_iter()
                        .find(|a| a.id == id)
                        .ok_or_else(|| anyhow!("that account no longer exists"))?;
                    let analysis = engine::analyze(
                        store.wishes(account.id)?,
                        account.server,
                        &store.manual_counters(account.id)?,
                        &data,
                    );
                    let xlsx = wish_core::excel::export(&account, &analysis, &data)?;
                    wish_core::paths::write_atomic(&path, &xlsx)?;
                    Ok(format!("Exported the wishes of {}.", account.label))
                }
            }
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

    /// Saves the mirror folder setting and, for a folder, writes the
    /// snapshot right away so the folder is never stale.
    pub fn set_mirror_folder(
        &mut self,
        folder: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let saved = self.change_settings(|settings| settings.mirror_folder = folder.clone(), cx);
        let mirrored = self.write(
            cx,
            false,
            move |store, _| Ok(folder.map(|folder| write_mirror(store, &folder))),
            |this, mirror| this.mirror = mirror,
        );
        cx.background_spawn(async move {
            saved.await?;
            mirrored.await
        })
    }

    fn set_loaded(&mut self, loaded: Loaded) {
        self.accounts = loaded.accounts;
        self.imports = loaded.imports;
        self.plan = loaded.plan;
        self.inventory = loaded.inventory;
        self.backups = loaded.backups;
    }

    /// Runs `work` on the background executor with the store locked, then
    /// refreshes the mirror folder (when `changes_data`) and reloads. Back on
    /// the app thread it installs the reloaded state and calls `finish`.
    ///
    /// Writes queue behind each other on the store lock, so `+1` clicked
    /// twice in a row counts twice.
    fn write<T, R>(
        &mut self,
        cx: &mut Context<Self>,
        changes_data: bool,
        work: impl FnOnce(&mut Store, &GameData) -> Result<T> + Send + 'static,
        finish: impl FnOnce(&mut Self, T) -> R + 'static,
    ) -> Task<Result<R>>
    where
        T: Send + 'static,
        R: 'static,
    {
        let (Some(store), Some(data)) = (self.store.clone(), self.data.clone()) else {
            return Task::ready(Err(anyhow!("the wish history is not open yet")));
        };
        self.writes += 1;
        cx.notify();
        let mirror_folder = self.settings.mirror_folder.clone().filter(|_| changes_data);
        let generations = self.generations.clone();
        let unfinished = self.unfinished.clone();
        unfinished.fetch_add(1, Ordering::SeqCst);

        // The write runs detached, so it finishes (and is counted out of
        // `writes`) even when the caller drops the returned task, say by
        // starting another flow.
        let (done, result) = oneshot::channel();
        cx.spawn(async move |this, cx| {
            let outcome = cx
                .background_spawn(async move {
                    // A panic becomes an error: the app must not stay busy.
                    // The write's transaction was rolled back as it unwound.
                    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                        let mut store = lock(&store);
                        // Taken under the lock, so it orders the reloads.
                        let generation = generations.fetch_add(1, Ordering::Relaxed) + 1;
                        let value = work(&mut store, &data);
                        let mirror = (mirror_folder.filter(|_| value.is_ok()))
                            .map(|folder| write_mirror(&store, &folder));
                        // After a failed write too: it undoes what the page
                        // showed at once.
                        let loaded = load(&store, &data);
                        (value, mirror, loaded, generation)
                    }))
                    .map_err(|panic| {
                        anyhow!("an internal error stopped it: {}", panic_text(&*panic))
                    });
                    unfinished.fetch_sub(1, Ordering::SeqCst);
                    outcome
                })
                .await;
            let result = this.update(cx, |this, cx| {
                this.writes -= 1;
                let result = outcome.and_then(|(value, mirror, loaded, generation)| {
                    // A later write may have finished first; keep its state.
                    match loaded {
                        Ok(loaded) if generation > this.loaded_generation => {
                            this.loaded_generation = generation;
                            this.set_loaded(loaded);
                            if let Some(mirror) = mirror {
                                this.mirror = Some(mirror);
                            }
                        }
                        Ok(_) => {}
                        Err(err) if value.is_ok() => return Err(err),
                        Err(_) => {}
                    }
                    value.map(|value| finish(this, value))
                });
                cx.notify();
                result
            });
            // Nobody waiting for the result is fine.
            done.send(result.and_then(|result| result)).ok();
        })
        .detach();
        cx.spawn(async move |_, _| {
            result
                .await
                .unwrap_or_else(|_| Err(anyhow!("the window closed before the change was saved")))
        })
    }
}

/// The store, for one write or read at a time. A write that panicked
/// leaves it as it was (each write is one transaction, rolled back as the
/// panic unwound), so the lock it poisoned is taken back.
fn lock(store: &Mutex<Store>) -> MutexGuard<'_, Store> {
    store.lock().unwrap_or_else(|poisoned| {
        store.clear_poison();
        poisoned.into_inner()
    })
}

/// What a panic said, for an error message.
fn panic_text(panic: &(dyn std::any::Any + Send)) -> String {
    (panic.downcast_ref::<&str>().map(|s| s.to_string()))
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic".to_owned())
}

/// The largest file an import reads.
const MAX_IMPORT_BYTES: u64 = 64 * 1024 * 1024;

/// How long quitting waits for a write under way.
const QUIT_WAIT: Duration = Duration::from_secs(5);

struct Opened {
    store: Store,
    data: GameData,
    planner: PlannerData,
    archive: ArchiveData,
    game_data_warning: Option<String>,
    settings_warning: Option<String>,
    images: Images,
    image_catalog: ImageCatalog,
    settings: Settings,
    loaded: Loaded,
}

/// Everything recomputed after a change.
struct Loaded {
    accounts: Vec<AccountSummary>,
    imports: Vec<ImportRecord>,
    plan: Vec<PlanEntry>,
    inventory: Inventory,
    backups: BackupStatus,
}

fn open(paths: AppPaths, image_requests: Option<UnboundedSender<Job>>) -> Result<Opened> {
    // Nothing in the game-data folder may keep the wishes from opening: a
    // damaged (or, for the wish data, older) copy gives way to the built-in
    // data, and says so.
    let (data, mut warnings) = GameData::load(&paths.game_data());
    // Preferences only hold folder paths; a damaged file must not lock the
    // user out of their wishes, nor be overwritten unseen.
    let (settings, settings_warning) = Settings::load_or_set_aside(&paths);
    let images = Images::scan(paths.image_cache(), image_requests);
    let planner = PlannerData::load(&paths.game_data()).unwrap_or_else(|e| {
        warnings.push(format!("{}: {e}", wish_core::planner::PLANNER_FILE));
        PlannerData::embedded().clone()
    });
    let archive = ArchiveData::load(&paths.game_data()).unwrap_or_else(|e| {
        warnings.push(format!("{}: {e}", wish_core::archive::ARCHIVE_FILE));
        ArchiveData::embedded().clone()
    });
    let image_catalog = ImageCatalog {
        icons: images::catalog(&data, &planner, &archive, Collection::Icons),
        everything: images::catalog(&data, &planner, &archive, Collection::Everything),
    };
    let store = Store::open(paths).context("opening the wish database")?;
    let loaded = load(&store, &data)?;
    Ok(Opened {
        store,
        data,
        planner,
        archive,
        game_data_warning: (!warnings.is_empty()).then(|| warnings.join("; ")),
        settings_warning,
        images,
        image_catalog,
        settings,
        loaded,
    })
}

fn load(store: &Store, data: &GameData) -> Result<Loaded> {
    let mut accounts = Vec::new();
    for account in store.accounts()? {
        let analysis = engine::analyze(
            store.wishes(account.id)?,
            account.server,
            &store.manual_counters(account.id)?,
            data,
        );
        // An account left empty by undoing its imports is not shown.
        if analysis.pools.is_empty() {
            continue;
        }
        accounts.push(AccountSummary {
            account,
            analysis: Arc::new(analysis),
        });
    }
    let backups = store.backups()?;
    Ok(Loaded {
        accounts,
        imports: store.recent_imports(RECENT_IMPORTS)?,
        plan: store.plan()?,
        inventory: store.inventory()?,
        backups: BackupStatus {
            count: backups.len(),
            newest: backups.first().map(|b| b.taken_at),
            files: backups,
        },
    })
}

fn write_mirror(store: &Store, folder: &Path) -> MirrorStatus {
    match snapshot::write_mirror(store, folder) {
        Ok(_) => MirrorStatus::Written(Local::now().naive_local()),
        Err(err) => MirrorStatus::Failed(err.to_string().into()),
    }
}

pub fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Whether an operation failed because the user cancelled it.
pub fn is_cancelled(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<wish_core::Error>(),
        Some(wish_core::Error::Cancelled)
    )
}
