//! The user's wish history as the window sees it: the store, the game data,
//! every account's analysis, and the operations that change them.
//!
//! `Library` owns this state; views observe it. Database work runs on the
//! background executor, behind a mutex, one write at a time.

use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow};
use chrono::{DateTime, Local, NaiveDateTime};
use futures_channel::mpsc::UnboundedSender;
use futures_channel::oneshot;
use gpui_kit::{App, AppContext as _, Context, Entity, SharedString, Subscription, Task};
use wish_core::archive::ArchiveData;
use wish_core::data::GameData;
use wish_core::images::{self, Collection};

use super::image_service::ImageCatalog;
use super::images::Job;
use wish_core::collection::{
    Collection as Owned, GoodFile, GoodPlan, OwnedCharacter, OwnedWeapon, SyncPlan,
};
use wish_core::error::IoAction;
use wish_core::hoyolab;
use wish_core::planner::{CustomItem, Inventory, PlanEntry, PlanTarget, PlannerData};
use wish_core::service::export::ExportSummary;
use wish_core::service::{self, lock};

use super::images::Images;
use wish_core::engine::{self, Analysis};
use wish_core::import::api::{FetchProgress, GachaLink};
use wish_core::import::api_plan::ApiPlan;
use wish_core::import::game_cache::{self, FoundLink};
use wish_core::import::{
    self, ImportBatch, ImportRecord, ImportReport, PreparedImport, UndoReport,
};
use wish_core::model::{Account, AccountId, Server, WishId};
use wish_core::settings::Settings;
use wish_core::snapshot;

pub use super::image_service::{DownloadState, ImageDownload, ImageService};
pub use super::preferences::Preferences;
pub use super::services::{ImageDownloads, Services};
pub use wish_core::service::good::GoodPreview;
pub use wish_core::service::hoyolab::{HoyolabAccount, SyncChoice};
use wish_core::store::{BackupFile, BackupReason};
use wish_core::{AppPaths, Store};

/// Where a GOOD file comes from: a file the player picked, or the text they
/// copied (Irminsul exports to the clipboard too).
#[derive(Clone, Debug)]
pub enum GoodSource {
    File(PathBuf),
    Text(String),
}

/// How many imports the import log shows.
const RECENT_IMPORTS: u32 = 10;
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

/// What one account owns and plans, as the player entered it.
#[derive(Clone, Debug, Default)]
pub struct Holdings {
    /// The ascension plan, in order.
    pub plan: Vec<PlanEntry>,
    /// The materials it has.
    pub inventory: Inventory,
    /// The characters and weapons it has.
    pub owned: Owned,
}

impl Holdings {
    pub fn is_empty(&self) -> bool {
        self.plan.is_empty() && self.inventory == Inventory::default() && self.owned.is_empty()
    }
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
    /// The network: the game's API, HoYoLAB, the image sites.
    services: Services,
    state: LoadState,
    store: Option<Arc<Mutex<Store>>>,
    data: Option<Arc<GameData>>,
    /// The planner's costs and the character pages, from genshin-db.
    planner: Arc<PlannerData>,
    archive: Arc<ArchiveData>,
    /// Why a copy of them in the game-data folder was set aside, if one was.
    game_data_warning: Option<SharedString>,
    /// The game's images and their downloads, an entity of their own so
    /// that an image arriving wakes none of the library's observers.
    images: Entity<ImageService>,
    accounts: Vec<AccountSummary>,
    imports: Vec<ImportRecord>,
    /// What each account owns and plans.
    holdings: HashMap<AccountId, Holdings>,
    /// What no account holds: shown before any account exists.
    no_holdings: Holdings,
    /// The player's settings, an entity of their own; a change is passed on
    /// to the library's observers.
    preferences: Entity<Preferences>,
    backups: BackupStatus,
    mirror: Option<MirrorStatus>,
    /// A change the mirror doesn't hold yet (set under the store lock).
    mirror_due: Arc<AtomicBool>,
    /// Writes the mirror once the changes stop (`schedule_mirror`).
    mirror_timer: Option<Task<()>>,
    /// Database writes queued or running.
    writes: usize,
    /// Writes not finished yet, counted where they run: quitting waits for
    /// them, since the app thread that counts `writes` is then blocked.
    unfinished: Arc<AtomicUsize>,
    _preferences_changes: Subscription,
    _wait_on_quit: Subscription,
    /// Counts writes in the order they take the store lock.
    generations: Arc<AtomicU64>,
    /// Each account's analysis, kept from one reload to the next: a write
    /// works out again only those whose wishes, counters or server it
    /// changed (`Store::take_changed`).
    analyses: Arc<Mutex<Analyses>>,
    /// The write whose reload is installed.
    loaded_generation: u64,
    /// Done when the last write made so far has run: the next one waits for
    /// it, so writes run in the order they were made.
    last_write: Option<oneshot::Receiver<()>>,
}

impl Library {
    /// Starts opening the database in the background; observers are
    /// notified when it is ready or has failed. `services` is how it reaches
    /// the network: tests give fakes ([`Services::offline`]).
    pub fn open(paths: AppPaths, services: Services, cx: &mut Context<Self>) -> Self {
        let images = cx.new(|cx| ImageService::new(paths.image_cache(), services.images, cx));
        let preferences = cx.new(|_| Preferences::new(paths.clone()));
        let image_requests = images.read(cx).requests();
        let task_paths = paths.clone();
        let analyses: Arc<Mutex<Analyses>> = Arc::default();
        let cache = analyses.clone();
        let unfinished = Arc::new(AtomicUsize::new(0));
        cx.spawn(async move |this, cx| {
            let opened = cx
                .background_spawn(async move { open(task_paths, image_requests, &cache) })
                .await;
            this.update(cx, |this, cx| {
                match opened {
                    Ok(opened) => {
                        this.store = Some(Arc::new(Mutex::new(opened.store)));
                        this.data = Some(Arc::new(opened.data));
                        this.planner = Arc::new(opened.planner);
                        this.archive = Arc::new(opened.archive);
                        this.game_data_warning = opened.game_data_warning.map(Into::into);
                        this.images.update(cx, |images, cx| {
                            images.set_scanned(opened.images, opened.image_catalog, cx)
                        });
                        this.preferences.update(cx, |preferences, cx| {
                            preferences.set_read(opened.settings, opened.settings_warning, cx)
                        });
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
            services,
            state: LoadState::Loading,
            store: None,
            data: None,
            planner: Arc::new(PlannerData::embedded().clone()),
            archive: Arc::new(ArchiveData::embedded().clone()),
            game_data_warning: None,
            images,
            accounts: Vec::new(),
            imports: Vec::new(),
            holdings: HashMap::new(),
            no_holdings: Holdings::default(),
            preferences: preferences.clone(),
            backups: BackupStatus::default(),
            mirror: None,
            mirror_due: Arc::default(),
            mirror_timer: None,
            writes: 0,
            unfinished: unfinished.clone(),
            // A setting can change what the pages show.
            _preferences_changes: cx.observe(&preferences, |_, _, cx| cx.notify()),
            _wait_on_quit: cx.on_app_quit(move |this, cx| {
                // A write cut off by quitting is rolled back (one transaction
                // each), but the change is lost: give it a few seconds.
                let since = Instant::now();
                while unfinished.load(Ordering::SeqCst) > 0 && since.elapsed() < QUIT_WAIT {
                    std::thread::sleep(Duration::from_millis(10));
                }
                // Then the mirror the last changes still wait for.
                if let (Some(store), Some(folder)) =
                    (this.store.clone(), this.settings(cx).mirror_folder.clone())
                {
                    write_mirror_if_due(&store, &folder, &this.mirror_due);
                }
                async {}
            }),
            generations: Arc::default(),
            loaded_generation: 0,
            last_write: None,
            analyses,
        }
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

    /// What `account` owns and plans; nothing for no account.
    ///
    /// Every method below takes the account the page shows (the session's).
    /// A write with no account goes to the first one, or, when there is
    /// none yet, to a new account named "Main": a player can plan before
    /// importing any wishes.
    pub fn holdings(&self, account: Option<AccountId>) -> &Holdings {
        account
            .and_then(|account| self.holdings.get(&account))
            .unwrap_or(&self.no_holdings)
    }

    /// `account`'s ascension plan, in order.
    pub fn plan(&self, account: Option<AccountId>) -> &[PlanEntry] {
        &self.holdings(account).plan
    }

    /// The materials `account` has, as the player entered them.
    pub fn inventory(&self, account: Option<AccountId>) -> &Inventory {
        &self.holdings(account).inventory
    }

    /// The characters and weapons `account` has.
    pub fn owned(&self, account: Option<AccountId>) -> &Owned {
        &self.holdings(account).owned
    }

    /// Set when a copy of the planner or archive data in the game-data folder
    /// could not be read, and the built-in data is used instead.
    pub fn game_data_warning(&self) -> Option<&SharedString> {
        self.game_data_warning.as_ref()
    }

    /// The game's images, for a page to draw. Reading them through the
    /// image service redraws the page as they arrive.
    pub fn images<'a>(&self, cx: &'a App) -> &'a Arc<Images> {
        self.images.read(cx).images()
    }

    /// The images' downloads (Settings → Images).
    pub fn image_service(&self) -> &Entity<ImageService> {
        &self.images
    }

    pub fn settings<'a>(&self, cx: &'a App) -> &'a Settings {
        self.preferences.read(cx).settings()
    }

    /// The settings, to change them.
    pub fn preferences(&self) -> &Entity<Preferences> {
        &self.preferences
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
    pub fn settings_warning<'a>(&self, cx: &'a App) -> Option<&'a SharedString> {
        self.preferences.read(cx).warning()
    }

    fn set_loaded(&mut self, loaded: Loaded) {
        self.accounts = loaded.accounts;
        self.imports = loaded.imports;
        self.holdings = loaded.holdings;
        self.backups = loaded.backups;
    }
}

/// Why a change or a fetch can't start: the data folder isn't open (it is
/// opening, or failed to).
fn not_open() -> anyhow::Error {
    anyhow!(t!("app-not-open"))
}

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
    holdings: HashMap<AccountId, Holdings>,
    backups: BackupStatus,
}

fn open(
    paths: AppPaths,
    image_requests: Option<UnboundedSender<Job>>,
    analyses: &Mutex<Analyses>,
) -> Result<Opened> {
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
    let mut store = Store::open(paths).context(t!("app-opening-database"))?;
    let loaded = load(&mut store, &data, analyses)?;
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

/// Each account's analysis, by account.
type Analyses = HashMap<AccountId, Arc<Analysis>>;

/// Everything the pages show, read again after a change. An account's
/// analysis, the costly part, is kept from `analyses` unless the store says
/// a write changed what it reads; plans, inventories and collections are
/// read again whole (a few small queries).
fn load(store: &mut Store, data: &GameData, analyses: &Mutex<Analyses>) -> Result<Loaded> {
    let mut analyses = analyses.lock().unwrap_or_else(PoisonError::into_inner);
    let changed = store.take_changed();
    analyses.retain(|account, _| !changed.includes(*account));
    let mut accounts = Vec::new();
    let mut all_holdings = HashMap::new();
    for account in store.accounts()? {
        let analysis = match analyses.get(&account.id) {
            Some(analysis) => analysis.clone(),
            None => {
                let analysis = Arc::new(engine::analyze(
                    store.wishes(account.id)?,
                    account.server,
                    &store.manual_counters(account.id)?,
                    data,
                ));
                analyses.insert(account.id, analysis.clone());
                analysis
            }
        };
        let holdings = Holdings {
            plan: store.plan(account.id)?,
            inventory: store.inventory(account.id)?,
            owned: store.collection(account.id)?,
        };
        // An account an import made and its undo left empty is not shown;
        // one added by hand is, even before it holds anything.
        if analysis.pools.is_empty() && holdings.is_empty() && store.has_imports(account.id)? {
            continue;
        }
        all_holdings.insert(account.id, holdings);
        accounts.push(AccountSummary { account, analysis });
    }
    let backups = store.backups()?;
    Ok(Loaded {
        accounts,
        imports: store.recent_imports(RECENT_IMPORTS)?,
        holdings: all_holdings,
        backups: BackupStatus {
            count: backups.len(),
            newest: backups.first().map(|b| b.taken_at),
            files: backups,
        },
    })
}

/// The account a write of the plan or the collection goes to: the one the
/// page shows, else the first, else a new account named "Main" (a player who
/// plans before importing any wishes).
fn account_for_writing(store: &mut Store, account: Option<AccountId>) -> Result<AccountId> {
    if let Some(account) = account {
        return Ok(account);
    }
    match store.accounts()?.first() {
        Some(first) => Ok(first.id),
        None => Ok(store.add_account("Main", None, None)?.id),
    }
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

mod collection;
mod data;
mod mirror;
use mirror::write_mirror_if_due;
mod plan;
mod write;

#[cfg(test)]
mod tests;
