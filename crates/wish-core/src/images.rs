//! The game's images: icons, banner art, wish art. They are HoYoverse's, not
//! ours to redistribute, so the app does not ship them. Each is downloaded on
//! first use from the fan sites that already serve them, then kept in the
//! image cache ([`AppPaths::image_cache`]): Project Amber for every image but
//! the banners' art, which only paimon.moe has.
//!
//! [`AppPaths::image_cache`]: crate::AppPaths::image_cache

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

use url::Url;

use crate::archive::ArchiveData;
use crate::data::{Banner, GameData};
use crate::error::{Error, Result};
use crate::model::{ItemType, Pool};
use crate::paths::write_atomic;
use crate::planner::PlannerData;

/// paimon.moe's images, as its site serves its `static/images` folder: here,
/// the banners' art.
const PAIMON: &str = "https://paimon.moe/images";
/// Project Amber's copy of the game's UI images, by the game's file names.
const AMBER: &str = "https://gi.yatta.moe/assets/UI";
/// No image of the game comes near; a bigger answer is not an image.
const MAX_BYTES: u64 = 16 * 1024 * 1024;
/// Beside an image none of its sites has: `<file>.missing`, empty.
const MISSING: &str = ".missing";
/// How long an image stays known to be missing: sites add the icons of new
/// characters some days after their release.
pub const MISSING_FOR: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// What earlier versions downloaded from paimon.moe and Project Amber has too
/// (the wished items' icons, the primogem): the same pictures twice. Removed
/// from the cache at launch ([`remove_obsolete`]).
const OBSOLETE: [&str; 3] = ["paimon/characters", "paimon/weapons", "paimon/primogem.png"];

/// The image a file of the cache marks as missing, if it is such a marker.
pub fn missing_marker(file: &str) -> Option<&str> {
    file.strip_suffix(MISSING)
}

/// An image the app shows: where the cache keeps it, and where it comes from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Remote {
    /// Its path in the image cache, `/`-separated: `paimon/banners/…` or
    /// `amber/…`.
    pub file: String,
    /// Where to download it, the first that has it.
    pub urls: Vec<String>,
}

impl Remote {
    /// A banner's art. paimon.moe names it after the banner; the cache, after
    /// its slug, since a `:` in a file name makes GPUI take it for a URL.
    pub fn banner(banner: &Banner) -> Remote {
        Remote {
            file: format!("paimon/banners/{}.png", banner.slug()),
            urls: vec![paimon_url(&["banners", &format!("{}.png", banner.id())])],
        }
    }

    /// The primogem's icon.
    pub fn primogem() -> Remote {
        Remote::amber("UI_ItemIcon_201")
    }

    /// Original Resin's icon, for the planner's estimate.
    pub fn resin() -> Remote {
        Remote::amber("UI_ItemIcon_106")
    }

    /// One of the game's UI images by its file name (`UI_ItemIcon_104315`),
    /// from Project Amber.
    pub fn amber(file: &str) -> Remote {
        Remote {
            file: format!("amber/{file}.png"),
            urls: vec![format!("{AMBER}/{file}.png")],
        }
    }

    /// A talent's or a constellation's icon. The newest have no `_HD` icon
    /// yet: the plain one stands in.
    pub fn skill(file: &str) -> Remote {
        let mut remote = Remote::amber(file);
        if let Some(plain) = file.strip_suffix("_HD") {
            remote.urls.push(format!("{AMBER}/{plain}.png"));
        }
        remote
    }

    /// An element's icon, by its name in the data ("Cryo").
    pub fn element(element: &str) -> Option<Remote> {
        let game = match element.to_ascii_lowercase().as_str() {
            "anemo" => "Wind",
            "geo" => "Rock",
            "electro" => "Electric",
            "dendro" => "Grass",
            "hydro" => "Water",
            "pyro" => "Fire",
            "cryo" => "Ice",
            _ => return None,
        };
        Some(Remote::amber(&format!("UI_Buff_Element_{game}")))
    }
}

/// The icon of each item a wish can give, by its key (`amos_bow`): the
/// planner's icon of the same character or weapon, found through the game's
/// id, so that an item is downloaded once for every page.
#[derive(Clone, Debug, Default)]
pub struct ItemIcons(HashMap<String, String>);

impl ItemIcons {
    pub fn new(game: &GameData, planner: &PlannerData) -> Self {
        fn by_id<'a>(
            entries: impl Iterator<Item = (u32, &'a Option<String>)>,
        ) -> HashMap<u32, &'a str> {
            entries
                .filter_map(|(id, icon)| Some((id, icon.as_deref()?)))
                .collect()
        }
        let characters = by_id(planner.characters().iter().map(|c| (c.id, &c.icon)));
        let weapons = by_id(planner.weapons().iter().map(|w| (w.id, &w.icon)));
        let by_key = game.items.iter().filter_map(|item| {
            let ids = match item.item_type {
                ItemType::Character => &characters,
                ItemType::Weapon => &weapons,
                ItemType::Unknown => return None,
            };
            let icon = ids.get(&item.item_id?)?;
            Some((item.key.clone(), (*icon).to_owned()))
        });
        ItemIcons(by_key.collect())
    }

    /// The icon of the wished item `key`, when the planner's data has it.
    pub fn get(&self, key: &str) -> Option<Remote> {
        self.0.get(key).map(|file| Remote::amber(file))
    }
}

impl Remote {
    /// About how big it is, for estimates: the average of its kind, as
    /// measured on the sites (sizes vary a lot within a kind).
    pub fn typical_bytes(&self) -> u64 {
        let file = self.file.as_str();
        if file.starts_with("paimon/banners/") {
            390_000
        } else if file.starts_with("amber/UI_Gacha_AvatarImg") {
            1_900_000
        } else {
            9_000
        }
    }
}

/// What a download of every image covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Collection {
    /// Every icon: items, characters, weapons, materials, elements, talents
    /// and constellations.
    Icons,
    /// The icons, the banner art and the characters' wish art.
    Everything,
}

/// Every image the app can show, each once.
pub fn catalog(
    game: &GameData,
    planner: &PlannerData,
    archive: &ArchiveData,
    collection: Collection,
) -> Vec<Remote> {
    // The wished items' icons are the planner's ([`ItemIcons`]).
    let mut remotes = vec![Remote::primogem()];
    remotes.push(Remote::resin());
    let icons = (planner.characters().iter().map(|c| &c.icon))
        .chain(planner.weapons().iter().map(|w| &w.icon))
        .chain(planner.materials().iter().map(|m| &m.icon))
        .flatten();
    remotes.extend(icons.map(|file| Remote::amber(file)));
    let elements = ["Anemo", "Geo", "Electro", "Dendro", "Hydro", "Pyro", "Cryo"];
    remotes.extend(elements.iter().filter_map(|e| Remote::element(e)));
    for page in archive.characters() {
        let skills = (page.talents.iter().map(|t| &t.icon))
            .chain(page.constellations.iter().map(|c| &c.icon))
            .flatten();
        remotes.extend(skills.map(|file| Remote::skill(file)));
    }
    if collection == Collection::Everything {
        for pool in Pool::ALL {
            remotes.extend(game.banners.for_pool(*pool).iter().map(Remote::banner));
        }
        let splash = archive
            .characters()
            .iter()
            .filter_map(|p| p.splash.as_deref());
        remotes.extend(splash.map(Remote::amber));
    }
    let mut seen = HashSet::new();
    remotes.retain(|remote| seen.insert(remote.file.clone()));
    remotes
}

/// A URL under paimon.moe's images, its segments encoded (banner names have
/// spaces).
fn paimon_url(segments: &[&str]) -> String {
    let mut url = Url::parse(PAIMON).expect("a valid base URL");
    url.path_segments_mut()
        .expect("an http URL has segments")
        .extend(segments);
    url.into()
}

static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .user_agent(concat!("genshin-companion/", env!("CARGO_PKG_VERSION")))
        // Both sites serve their files directly: a redirect would take the
        // request (and the user's address) to a host the README doesn't name,
        // and plain http would let anyone on the way swap the image.
        .https_only(true)
        .max_redirects(0)
        .build();
    ureq::Agent::new_with_config(config)
});

/// Downloads `remote` into the image cache at `cache`: its size once it is
/// there, `None` when none of its sources has it. Blocking; run it on a
/// background thread.
pub fn download(remote: &Remote, cache: &Path) -> Result<Option<u64>> {
    let file = in_cache(cache, &remote.file)?;
    for url in &remote.urls {
        let failed = |problem: String| Error::ImageDownload {
            url: url.clone(),
            problem,
        };
        let mut response = match AGENT.get(url).call() {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(404)) => continue,
            Err(e) => return Err(failed(e.to_string())),
        };
        if response.status().is_redirection() {
            return Err(failed(format!(
                "redirected ({}), which is not followed",
                response.status()
            )));
        }
        // paimon.moe answers a file it lacks with its web page, not a 404.
        let html = (response.headers().get("content-type"))
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/html"));
        if html {
            continue;
        }
        let bytes = response
            .body_mut()
            .with_config()
            .limit(MAX_BYTES)
            .read_to_vec()
            .map_err(|e| failed(e.to_string()))?;
        if !is_image(&bytes) {
            return Err(failed("the answer is not an image".to_owned()));
        }
        write_atomic(&file, &bytes)?;
        let _ = std::fs::remove_file(cache.join(format!("{}{MISSING}", remote.file)));
        return Ok(Some(bytes.len() as u64));
    }
    // Remembered, so the next launches don't ask again (see `MISSING_FOR`).
    // Only a convenience: no harm done if it can't be written.
    let marker = cache.join(format!("{}{MISSING}", remote.file));
    if let Some(dir) = marker.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&marker, b"");
    Ok(None)
}

/// Removes from the image cache at `cache` what earlier versions downloaded
/// and this one no longer shows (see `OBSOLETE`). Blocking; nothing is lost
/// if it fails.
pub fn remove_obsolete(cache: &Path) {
    for file in OBSOLETE {
        let Ok(path) = in_cache(cache, file) else {
            continue;
        };
        let _ = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
    }
}

/// Where `file` (a name in the cache, maybe with folders) is in `cache`.
/// The names come from the game data, which a local copy may override:
/// never let one climb out of the cache.
pub fn in_cache(cache: &Path, file: &str) -> Result<PathBuf> {
    let path = Path::new(file);
    let inside = path.components().all(|c| matches!(c, Component::Normal(_)));
    if !inside || file.is_empty() {
        return Err(Error::ImageDownload {
            url: file.to_owned(),
            problem: "not a file name".to_owned(),
        });
    }
    Ok(cache.join(path))
}

/// PNG, JPEG or WebP, by their first bytes.
fn is_image(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(&[0xFF, 0xD8, 0xFF])
        || (bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::model::Pool;

    #[test]
    fn urls_follow_each_site() {
        let primogem = Remote::primogem();
        assert_eq!(primogem.file, "amber/UI_ItemIcon_201.png");
        assert_eq!(
            primogem.urls,
            ["https://gi.yatta.moe/assets/UI/UI_ItemIcon_201.png"]
        );

        let burst = Remote::skill("Skill_E_Ganyu_01_HD");
        assert_eq!(
            burst.urls,
            [
                "https://gi.yatta.moe/assets/UI/Skill_E_Ganyu_01_HD.png",
                "https://gi.yatta.moe/assets/UI/Skill_E_Ganyu_01.png"
            ]
        );
        assert_eq!(
            Remote::element("Cryo").unwrap().file,
            "amber/UI_Buff_Element_Ice.png"
        );
    }

    #[test]
    fn every_wished_item_has_the_planners_icon() {
        let (game, planner) = (GameData::embedded(), PlannerData::embedded());
        let icons = ItemIcons::new(game, planner);
        let bow = icons.get("amos_bow").unwrap();
        assert_eq!(bow.file, "amber/UI_EquipIcon_Bow_Amos.png");
        assert_eq!(
            icons.get("raiden_shogun").unwrap().file,
            "amber/UI_AvatarIcon_Shougun.png"
        );
        // No wish gives the Traveler, which paimon.moe lists as a character.
        let without: Vec<&str> = (game.items.iter())
            .filter(|item| !item.key.starts_with("traveler") && icons.get(&item.key).is_none())
            .map(|item| item.key.as_str())
            .collect();
        assert!(without.is_empty(), "no icon for {without:?}");
    }

    #[test]
    fn what_earlier_versions_downloaded_twice_goes() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path();
        for file in [
            "paimon/characters/aino.png",
            "paimon/weapons/amos_bow.png",
            "paimon/weapons/new.png.missing",
            "paimon/primogem.png",
            "paimon/banners/Adrift in the Harbor 1.png",
            "amber/UI_ItemIcon_201.png",
        ] {
            let path = cache.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"png").unwrap();
        }
        remove_obsolete(cache);
        remove_obsolete(cache);
        assert!(!cache.join("paimon/characters").exists());
        assert!(!cache.join("paimon/weapons").exists());
        assert!(!cache.join("paimon/primogem.png").exists());
        assert!(
            cache
                .join("paimon/banners/Adrift in the Harbor 1.png")
                .exists()
        );
        assert!(cache.join("amber/UI_ItemIcon_201.png").exists());
    }

    #[test]
    fn banner_names_are_encoded_and_files_are_slugs() {
        let data = GameData::embedded();
        for pool in Pool::ALL {
            for banner in data.banners.for_pool(*pool) {
                let remote = Remote::banner(banner);
                assert!(!remote.urls[0].contains(' '), "{}", remote.urls[0]);
                assert!(!remote.file.contains(':'), "{}", remote.file);
                assert!(Url::parse(&remote.urls[0]).is_ok());
            }
        }
    }

    #[test]
    fn the_catalog_lists_every_image_once() {
        let (game, planner, archive) = (
            GameData::embedded(),
            PlannerData::embedded(),
            ArchiveData::embedded(),
        );
        let icons = catalog(game, planner, archive, Collection::Icons);
        let everything = catalog(game, planner, archive, Collection::Everything);
        assert!(icons.len() > 2_000, "{} icons", icons.len());
        assert!(everything.len() > icons.len() + 300);
        let files: HashSet<&str> = everything.iter().map(|r| r.file.as_str()).collect();
        assert_eq!(files.len(), everything.len());
        assert!(icons.iter().all(|r| files.contains(r.file.as_str())));
        // The banner and wish art are what makes "everything" big.
        let size = |remotes: &[Remote]| remotes.iter().map(Remote::typical_bytes).sum::<u64>();
        assert!(size(&everything) > 3 * size(&icons));
    }

    #[test]
    fn only_images_are_kept() {
        assert!(is_image(b"\x89PNG\r\n\x1a\nrest"));
        assert!(is_image(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(is_image(b"RIFF\0\0\0\0WEBPVP8 "));
        assert!(!is_image(b"<!DOCTYPE html>"));

        let dir = tempfile::tempdir().unwrap();
        let escaping = Remote {
            file: "../outside.png".to_owned(),
            urls: Vec::new(),
        };
        assert!(download(&escaping, dir.path()).is_err());
    }
}
