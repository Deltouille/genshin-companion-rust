//! The game's images, as the pages ask for them, and their downloads: the
//! [`Images`] the pages draw from, the downloader that fills the cache, and
//! "download every image" for offline use (Settings → Images).
//!
//! An entity of its own, apart from the [`Library`](super::Library): an
//! image arriving redraws the pages that read this entity (they draw images),
//! and wakes none of the library's observers, which rebuild the history's
//! table, the planner's sliders or the account picker on every change.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow};
use futures_channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures_util::StreamExt as _;
use gpui_kit::{AppContext as _, Context, Task};
use wish_core::images::{self, Collection, Remote};

use super::images::{Fetched, Images, Job};
use super::services::ImageDownloads;

/// How many images download at once.
const IMAGE_DOWNLOADS: usize = 6;
/// A download of every image gives up after this many failures without a
/// single image: the sites are out of reach.
const GIVE_UP_AFTER: usize = 20;

/// Every image the app can show, by collection.
#[derive(Debug, Default)]
pub(crate) struct ImageCatalog {
    pub icons: Vec<Remote>,
    pub everything: Vec<Remote>,
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

pub struct ImageService {
    cache: PathBuf,
    /// Where the pages' requests go, when downloads are on.
    requests: Option<UnboundedSender<Job>>,
    images: Arc<Images>,
    catalog: Arc<ImageCatalog>,
    /// The download of every image running, or the last one.
    download: Option<ImageDownload>,
}

impl ImageService {
    /// With downloads on, starts the downloader the pages' requests go to.
    /// Until the library has scanned the cache ([`ImageService::set_scanned`]),
    /// there are no images.
    pub(crate) fn new(cache: PathBuf, downloads: ImageDownloads, cx: &mut Context<Self>) -> Self {
        let requests = match downloads {
            ImageDownloads::On => {
                let (requests, queue) = mpsc::unbounded();
                Self::run_downloads(queue, cache.clone(), cx);
                Some(requests)
            }
            ImageDownloads::Off => None,
        };
        Self {
            cache,
            requests,
            images: Arc::new(Images::default()),
            catalog: Arc::default(),
            download: None,
        }
    }

    /// Where the [`Images`] the library scans send what they lack.
    pub(crate) fn requests(&self) -> Option<UnboundedSender<Job>> {
        self.requests.clone()
    }

    /// The cache as the library scanned it when it opened, and every image
    /// its game data names.
    pub(crate) fn set_scanned(
        &mut self,
        images: Images,
        catalog: ImageCatalog,
        cx: &mut Context<Self>,
    ) {
        self.images = Arc::new(images);
        self.catalog = Arc::new(catalog);
        cx.notify();
    }

    pub fn images(&self) -> &Arc<Images> {
        &self.images
    }

    /// Downloads the images the pages ask for, a few at a time, and redraws
    /// as they arrive. A failed download leaves its place empty until the
    /// next launch.
    fn run_downloads(queue: UnboundedReceiver<Job>, cache: PathBuf, cx: &mut Context<Self>) {
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
                let open = this.update(cx, |service, cx| {
                    service.images.finished(job.file(), fetched);
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
    pub fn missing(&self, collection: Collection) -> (usize, u64) {
        (self.catalog_of(collection).iter())
            .filter(|remote| self.images.missing(remote))
            .fold((0, 0), |(count, bytes), remote| {
                (count + 1, bytes + remote.typical_bytes())
            })
    }

    fn catalog_of(&self, collection: Collection) -> &[Remote] {
        match collection {
            Collection::Icons => &self.catalog.icons,
            Collection::Everything => &self.catalog.everything,
        }
    }

    /// The download of every image running, or the last one.
    pub fn download(&self) -> Option<&ImageDownload> {
        self.download.as_ref()
    }

    /// Downloads every image of `collection` missing from the cache, for
    /// offline use: a few at a time, like a page's. Gives up when nothing
    /// can be reached.
    pub fn download_all(&mut self, collection: Collection, cx: &mut Context<Self>) {
        let running = self
            .download
            .as_ref()
            .is_some_and(ImageDownload::is_running);
        if running || !self.images.downloads() {
            return;
        }
        let missing: Vec<Remote> = (self.catalog_of(collection).iter())
            .filter(|remote| self.images.missing(remote))
            .cloned()
            .collect();
        let cancel = Arc::new(AtomicBool::new(false));
        self.download = Some(ImageDownload {
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
        let cache = self.cache.clone();
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
                let open = this.update(cx, |service, cx| {
                    if let Some(download) = &mut service.download {
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
            this.update(cx, |service, cx| {
                if let Some(download) = &mut service.download {
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
    pub fn stop_download(&mut self, cx: &mut Context<Self>) {
        if let Some(download) = &mut self.download
            && download.state == DownloadState::Running
        {
            download.cancel.store(true, Ordering::Relaxed);
            download.state = DownloadState::Stopping;
            cx.notify();
        }
    }

    /// Deletes the downloaded images; pages download them again as they show
    /// them. Nothing in the cache is the player's.
    pub fn clear_cache(&mut self, cx: &mut Context<Self>) -> Task<Result<()>> {
        if self
            .download
            .as_ref()
            .is_some_and(ImageDownload::is_running)
        {
            return Task::ready(Err(anyhow!("images are downloading: stop that first")));
        }
        let cache = self.cache.clone();
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
            this.update(cx, |service, cx| {
                service.images.forget();
                service.download = None;
                cx.notify();
            })?;
            Ok(())
        })
    }
}
