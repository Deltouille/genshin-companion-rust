//! Item icons, banner art and the archive's pictures.
//!
//! None ships with the app: they are HoYoverse's (see
//! [`wish_core::images`]). Each is downloaded the first time a page shows it
//! and kept in the image cache. Until it is there, and when it cannot be
//! downloaded (offline), the page shows its place empty: everything works
//! without an image.
//!
//! Asking for an image never blocks: [`Images`] answers from the files it
//! knows are in the cache, and queues the others for the [`Library`]'s
//! downloader, which redraws the window as they arrive. The same queue makes
//! the blurred backdrops of wish art, which GPUI cannot blur itself.
//!
//! [`Library`]: super::Library

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures_channel::mpsc::UnboundedSender;
use gpui_kit::ImageSource;
use wish_core::data::Banner;
use wish_core::images::{self, Remote};
use wish_core::model::Wish;
use wish_core::planner::{Character, Material, Weapon};

#[derive(Clone, Debug, Default)]
pub struct Images {
    cache: PathBuf,
    state: Arc<Mutex<State>>,
    /// To the downloader; none before the library is open.
    requests: Option<UnboundedSender<Job>>,
}

/// Work for the downloader, each making one file of the cache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Job {
    Download(Remote),
    /// The blurred backdrop `to` of the image `from`, already in the cache.
    Blur {
        from: String,
        to: String,
    },
}

impl Job {
    /// The file it makes, as the cache lists it.
    pub(crate) fn file(&self) -> &str {
        match self {
            Job::Download(remote) => &remote.file,
            Job::Blur { to, .. } => to,
        }
    }

    /// Does the work, blocking: run it on a background thread.
    pub(crate) fn run(&self, cache: &Path) -> Fetched {
        match self {
            Job::Download(remote) => Fetched::of(images::download(remote, cache)),
            Job::Blur { from, to } => {
                let paths = images::in_cache(cache, from)
                    .and_then(|from| Ok((from, images::in_cache(cache, to)?)));
                match paths
                    .map_err(anyhow::Error::from)
                    .and_then(|(from, to)| blur(&from, &to))
                {
                    Ok(bytes) => Fetched::Stored(bytes),
                    Err(_) => Fetched::Failed,
                }
            }
        }
    }
}

/// A backdrop's width: the art shrunk this small, then blurred, reads as
/// colour once stretched across a page.
const BACKDROP_WIDTH: u32 = 160;
const BACKDROP_BLUR: f32 = 6.0;

/// Writes a blurred, shrunk copy of the image `from` at `to`; returns its
/// size.
pub(crate) fn blur(from: &Path, to: &Path) -> anyhow::Result<u64> {
    // A small file may claim huge dimensions: the wish art is 2048 px wide
    // at most, so anything past these limits is refused before decoding.
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    let mut reader = image::ImageReader::open(from)?.with_guessed_format()?;
    reader.limits(limits);
    let image = reader.decode()?;
    let soft = image
        .thumbnail(BACKDROP_WIDTH, BACKDROP_WIDTH)
        .blur(BACKDROP_BLUR);
    let mut bytes = Vec::new();
    soft.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )?;
    wish_core::paths::write_atomic(to, &bytes)?;
    Ok(bytes.len() as u64)
}

/// How a download went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fetched {
    /// In the cache now, this many bytes.
    Stored(u64),
    /// None of its sites has it.
    NotFound,
    /// Not downloaded: no connection, a site down…
    Failed,
}

impl Fetched {
    pub(crate) fn of(download: wish_core::error::Result<Option<u64>>) -> Self {
        match download {
            Ok(Some(bytes)) => Fetched::Stored(bytes),
            Ok(None) => Fetched::NotFound,
            Err(_) => Fetched::Failed,
        }
    }
}

#[derive(Debug, Default)]
struct State {
    /// Files in the cache, as `Remote::file`.
    present: HashSet<String>,
    /// Asked for this session and not downloaded yet.
    waiting: HashSet<String>,
    /// Not downloaded this session (no connection…): pages ask again at the
    /// next launch, a download of everything right away.
    failed: HashSet<String>,
    /// Not on its sites this session: asked again at the next launch.
    not_found: HashSet<String>,
    /// The size of the files in `present`.
    bytes: u64,
}

impl Images {
    /// Lists the cache once, so rendering never touches the disk to find out
    /// whether an image is there. Without `requests`, missing images are not
    /// downloaded.
    pub(crate) fn scan(cache: PathBuf, requests: Option<UnboundedSender<Job>>) -> Self {
        let mut state = State::default();
        list(&cache, "", &mut state);
        Self {
            cache,
            state: Arc::new(Mutex::new(state)),
            requests,
        }
    }

    /// Whether missing images are downloaded (not in tests).
    pub fn downloads(&self) -> bool {
        self.requests.is_some()
    }

    /// How many images the cache holds, and their size.
    pub fn usage(&self) -> (usize, u64) {
        self.state
            .lock()
            .map_or((0, 0), |state| (state.present.len(), state.bytes))
    }

    /// Whether `remote` is still to download: not in the cache, and not
    /// known to be missing from its sites.
    pub fn missing(&self, remote: &Remote) -> bool {
        self.state.lock().is_ok_and(|state| {
            !state.present.contains(&remote.file) && !state.not_found.contains(&remote.file)
        })
    }

    /// Takes `remote` for a download of its own: `false` when it is already
    /// there, downloading or known to be missing, so that no two downloads
    /// write one file. One that failed is tried again.
    pub(crate) fn claim(&self, remote: &Remote) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let file = &remote.file;
        if state.present.contains(file)
            || state.waiting.contains(file)
            || state.not_found.contains(file)
        {
            return false;
        }
        state.failed.remove(&remote.file);
        state.waiting.insert(remote.file.clone())
    }

    /// Forgets the cache's files, once they are deleted.
    pub(crate) fn forget(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.present.clear();
            state.failed.clear();
            state.not_found.clear();
            state.bytes = 0;
        }
    }

    pub fn cache(&self) -> &Path {
        &self.cache
    }

    /// Images asked for and still downloading.
    pub fn pending(&self) -> usize {
        self.state.lock().map_or(0, |state| state.waiting.len())
    }

    /// Records the end of the job making `file`.
    pub(crate) fn finished(&self, file: &str, fetched: Fetched) {
        if let Ok(mut state) = self.state.lock() {
            state.waiting.remove(file);
            let file = file.to_owned();
            match fetched {
                Fetched::Stored(bytes) => {
                    if state.present.insert(file) {
                        state.bytes += bytes;
                    }
                }
                Fetched::NotFound => {
                    state.not_found.insert(file);
                }
                Fetched::Failed => {
                    state.failed.insert(file);
                }
            }
        }
    }

    /// The image, when it is in the cache; otherwise it is queued for
    /// download (once) and the page draws without it.
    pub fn get(&self, remote: Remote) -> Option<ImageSource> {
        let mut state = self.state.lock().ok()?;
        if state.present.contains(&remote.file) {
            return Some(self.cache.join(&remote.file).into());
        }
        let asked = state.waiting.contains(&remote.file)
            || state.failed.contains(&remote.file)
            || state.not_found.contains(&remote.file);
        if !asked && let Some(requests) = &self.requests {
            state.waiting.insert(remote.file.clone());
            requests.unbounded_send(Job::Download(remote)).ok();
        }
        None
    }

    /// A blurred copy of a character's wish art, to set behind it: once the
    /// art is downloaded, it is made (once) and the page draws without it
    /// meanwhile.
    pub fn backdrop(&self, file: Option<&str>) -> Option<ImageSource> {
        let art = Remote::amber(file?);
        let blurred = format!("blur/{}", art.file);
        {
            let mut state = self.state.lock().ok()?;
            if state.present.contains(&blurred) {
                return Some(self.cache.join(&blurred).into());
            }
            if state.present.contains(&art.file) {
                let asked = state.waiting.contains(&blurred) || state.failed.contains(&blurred);
                if !asked && let Some(requests) = &self.requests {
                    state.waiting.insert(blurred.clone());
                    let job = Job::Blur {
                        from: art.file,
                        to: blurred,
                    };
                    requests.unbounded_send(job).ok();
                }
                return None;
            }
        }
        // The art first.
        self.get(art);
        None
    }

    /// The icon of a wished item, when there is one.
    pub fn item(&self, wish: &Wish) -> Option<ImageSource> {
        let key = wish.item_key.as_deref()?;
        self.get(Remote::wish_icon(wish.item_type, key)?)
    }

    /// The art of a banner.
    pub fn banner(&self, banner: &Banner) -> Option<ImageSource> {
        self.get(Remote::banner(banner))
    }

    /// paimon.moe's primogem icon.
    pub fn primogem(&self) -> Option<ImageSource> {
        self.get(Remote::primogem())
    }

    /// Original Resin's icon.
    pub fn resin(&self) -> Option<ImageSource> {
        self.get(Remote::resin())
    }

    /// A character's wish art, by the game's file name.
    pub fn splash(&self, file: Option<&str>) -> Option<ImageSource> {
        self.get(Remote::amber(file?))
    }

    /// A talent's or constellation's icon, by the game's file name.
    pub fn skill(&self, file: &str) -> Option<ImageSource> {
        self.get(Remote::skill(file))
    }

    /// An element's icon: "Cryo".
    pub fn element(&self, element: &str) -> Option<ImageSource> {
        self.get(Remote::element(element)?)
    }

    /// The planner's icon of a character.
    pub fn character(&self, character: &Character) -> Option<ImageSource> {
        self.get(Remote::amber(character.icon.as_deref()?))
    }

    /// The planner's icon of a weapon.
    pub fn weapon(&self, weapon: &Weapon) -> Option<ImageSource> {
        self.get(Remote::amber(weapon.icon.as_deref()?))
    }

    /// The icon of a material.
    pub fn material(&self, material: &Material) -> Option<ImageSource> {
        self.get(Remote::amber(material.icon.as_deref()?))
    }
}

/// Adds the files under `dir` to `state`, as `/`-separated paths after `prefix`.
fn list(dir: &Path, prefix: &str, state: &mut State) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => list(&entry.path(), &format!("{path}/"), state),
            // A download in progress or interrupted: not an image yet.
            Ok(_) if name.starts_with('.') => {}
            Ok(_) => match images::missing_marker(&path) {
                // Known to be missing from its sites, for a while.
                Some(image) => {
                    let age = (entry.metadata().and_then(|m| m.modified()).ok())
                        .and_then(|time| time.elapsed().ok());
                    if age.is_some_and(|age| age < images::MISSING_FOR) {
                        state.not_found.insert(image.to_owned());
                    }
                }
                None => {
                    state.bytes += entry.metadata().map_or(0, |m| m.len());
                    state.present.insert(path);
                }
            },
            Err(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_channel::mpsc;

    #[test]
    fn an_image_is_asked_for_once_then_served_from_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("paimon")).unwrap();
        fs::write(dir.path().join("paimon/primogem.png"), b"png").unwrap();
        // Missing from its site, as a past launch found out.
        fs::create_dir_all(dir.path().join("amber")).unwrap();
        fs::write(
            dir.path().join("amber/UI_Buff_Element_Wind.png.missing"),
            b"",
        )
        .unwrap();
        let (requests, mut queue) = mpsc::unbounded();
        let images = Images::scan(dir.path().to_path_buf(), Some(requests));

        // Already there: served, nothing asked; known missing: not asked.
        assert!(images.primogem().is_some());
        assert!(images.element("Anemo").is_none());
        assert!(!images.missing(&Remote::element("Anemo").unwrap()));
        assert_eq!(images.usage(), (1, 3));
        let ice = || images.element("Cryo");
        assert!(ice().is_none());
        assert!(ice().is_none());
        let Job::Download(asked) = queue.try_recv().unwrap() else {
            panic!("a download");
        };
        assert_eq!(asked.file, "amber/UI_Buff_Element_Ice.png");
        assert!(queue.try_recv().is_err(), "asked once");
        assert_eq!(images.pending(), 1);

        images.finished(&asked.file, Fetched::Stored(3));
        assert_eq!(images.pending(), 0);
        assert!(ice().is_some());

        // Not found: not asked again this session.
        assert!(images.element("Pyro").is_none());
        let Job::Download(fire) = queue.try_recv().unwrap() else {
            panic!("a download");
        };
        images.finished(&fire.file, Fetched::Failed);
        assert!(images.element("Pyro").is_none());
        assert!(queue.try_recv().is_err());
        assert_eq!(images.usage(), (2, 6));

        // A download of everything takes what is missing, the failed included,
        // and never what a page is already downloading or the sites lack.
        assert!(images.missing(&fire));
        assert!(images.claim(&fire));
        assert!(!images.claim(&fire));
        assert!(!images.claim(&asked));
        assert!(images.element("Dendro").is_none());
        let Job::Download(grass) = queue.try_recv().unwrap() else {
            panic!("a download");
        };
        images.finished(&grass.file, Fetched::NotFound);
        assert!(!images.missing(&grass));
        assert!(!images.claim(&grass));
        images.forget();
        assert_eq!(images.usage(), (0, 0));
        assert!(images.missing(&asked));
    }

    #[test]
    fn every_material_and_character_has_a_source() {
        let data = wish_core::planner::PlannerData::embedded();
        let without: Vec<&str> = (data.characters().iter().filter(|c| c.icon.is_none()))
            .map(|c| c.key.as_str())
            .chain((data.materials().iter().filter(|m| m.icon.is_none())).map(|m| m.key.as_str()))
            .collect();
        assert!(without.is_empty(), "without an icon name: {without:?}");
    }
}

#[cfg(test)]
mod backdrop_tests {
    use super::*;

    #[test]
    fn a_backdrop_is_a_small_blurred_copy() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("art.png"), dir.path().join("blur/art.png"));
        let art = image::RgbaImage::from_fn(1024, 512, |x, _| {
            image::Rgba([if x < 512 { 255 } else { 0 }, 0, 0, 255])
        });
        art.save(&from).unwrap();

        let bytes = blur(&from, &to).unwrap();
        assert_eq!(fs::metadata(&to).unwrap().len(), bytes);
        let soft = image::open(&to).unwrap().to_rgba8();
        assert_eq!((soft.width(), soft.height()), (160, 80));
        // The sharp edge in the middle is a gradient now.
        let middle = soft.get_pixel(80, 40)[0];
        assert!(middle > 40 && middle < 215, "{middle}");
    }
}
