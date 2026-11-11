//! Finding the wish history link on this computer, as paimon.moe's
//! PowerShell script does: the game writes its install folder into
//! `output_log.txt`, and caches the history page (with its link) under
//! `<data folder>/webCaches/<version>/Cache/Cache_Data/data_2`.
//!
//! On Linux the game runs in a Wine or Proton prefix, where the same files
//! live under `drive_c`, and the log's Windows paths are resolved through the
//! prefix's `dosdevices` drive links.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::api::GachaLink;

/// Folders under `AppData/LocalLow/miHoYo` holding `output_log.txt`.
const LOG_FOLDERS: [&str; 2] = ["Genshin Impact", "原神"];
/// The game's data folder, global and China releases.
const DATA_FOLDERS: [&str; 2] = ["GenshinImpact_Data", "YuanShen_Data"];

#[derive(Clone, Debug)]
pub struct FoundLink {
    pub link: GachaLink,
    /// The link as found, to show and to paste elsewhere.
    pub url: String,
    pub cache_file: PathBuf,
}

/// Looks for the most recent link in the usual places for this system.
pub fn find_link() -> Option<FoundLink> {
    latest_link(&candidate_data_folders())
}

/// Looks in a game folder chosen by the user: the install folder, the
/// launcher folder above it, or the data folder itself.
pub fn find_link_in_game_folder(folder: &Path) -> Option<FoundLink> {
    let mut data_folders = Vec::new();
    for base in [folder.to_path_buf(), folder.join("Genshin Impact game")] {
        if base
            .file_name()
            .is_some_and(|n| DATA_FOLDERS.iter().any(|d| n == *d))
        {
            data_folders.push(base.clone());
        }
        data_folders.extend(DATA_FOLDERS.iter().map(|d| base.join(d)));
    }
    latest_link(&data_folders)
}

#[cfg(windows)]
fn candidate_data_folders() -> Vec<PathBuf> {
    let Some(profile) = std::env::var_os("USERPROFILE") else {
        return Vec::new();
    };
    let local_low = PathBuf::from(profile).join("AppData/LocalLow/miHoYo");
    LOG_FOLDERS
        .iter()
        .filter_map(|name| fs::read(local_low.join(name).join("output_log.txt")).ok())
        .flat_map(|log| data_folders_in_log(&String::from_utf8_lossy(&log)))
        .map(PathBuf::from)
        .collect()
}

#[cfg(not(windows))]
fn candidate_data_folders() -> Vec<PathBuf> {
    match std::env::var_os("HOME") {
        Some(home) => wine_data_folders(Path::new(&home)),
        None => Vec::new(),
    }
}

/// Data folders named in the game logs of the Wine/Proton prefixes found
/// under `home`. Not on Windows, where the game runs natively.
#[cfg(not(windows))]
pub(crate) fn wine_data_folders(home: &Path) -> Vec<PathBuf> {
    let mut prefixes = vec![
        home.join(".wine"),
        home.join(".local/share/anime-game-launcher/prefix"),
        home.join(".var/app/moe.launcher.an-anime-game-launcher/data/anime-game-launcher/prefix"),
    ];
    for steam in [".local/share/Steam", ".steam/steam"] {
        if let Ok(entries) = fs::read_dir(home.join(steam).join("steamapps/compatdata")) {
            prefixes.extend(entries.flatten().map(|e| e.path().join("pfx")));
        }
    }

    let mut folders = Vec::new();
    for prefix in prefixes {
        let Ok(users) = fs::read_dir(prefix.join("drive_c/users")) else {
            continue;
        };
        for user in users.flatten() {
            for name in LOG_FOLDERS {
                let log = user
                    .path()
                    .join("AppData/LocalLow/miHoYo")
                    .join(name)
                    .join("output_log.txt");
                let Ok(log) = fs::read(log) else { continue };
                for windows_path in data_folders_in_log(&String::from_utf8_lossy(&log)) {
                    if let Some(path) = in_prefix(&prefix, &windows_path) {
                        folders.push(path);
                    }
                }
            }
        }
    }
    folders
}

/// `C:/Games/Genshin` inside `prefix`: through `dosdevices/c:` when the
/// prefix has it, else `drive_c`.
#[cfg(not(windows))]
fn in_prefix(prefix: &Path, windows_path: &str) -> Option<PathBuf> {
    let (drive, rest) = windows_path.split_once(":/")?;
    let drive = drive.to_ascii_lowercase();
    let device = prefix.join("dosdevices").join(format!("{drive}:"));
    let root = if device.exists() {
        device
    } else if drive == "c" {
        prefix.join("drive_c")
    } else {
        return None;
    };
    Some(root.join(rest))
}

/// Paths of the game's data folder mentioned in a log, such as
/// `C:/Program Files/Genshin Impact/Genshin Impact game/GenshinImpact_Data`.
fn data_folders_in_log(log: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for line in log.lines() {
        let line = line.replace('\\', "/");
        for name in DATA_FOLDERS {
            let Some(name_at) = line.find(name) else {
                continue;
            };
            let before = &line[..name_at];
            // The drive letter: the last "X:/" before the folder name. The log
            // is the game's, any text: read characters, not bytes, so a
            // multi-byte one before the colon ("é:/") can't split a slice.
            let Some(colon) = before.rfind(":/") else {
                continue;
            };
            let mut back = before[..colon].chars().rev();
            let Some(letter) = back.next().filter(char::is_ascii_alphabetic) else {
                continue;
            };
            if back.next().is_some_and(|c| c.is_ascii_alphanumeric()) {
                continue;
            }
            // `letter` is ASCII, one byte: `colon - 1` starts it.
            let path = line[colon - letter.len_utf8()..name_at + name.len()].to_owned();
            if !found.contains(&path) {
                found.push(path);
            }
        }
    }
    found
}

/// The newest link among the caches of the given data folders.
fn latest_link(data_folders: &[PathBuf]) -> Option<FoundLink> {
    let mut caches: Vec<(SystemTime, PathBuf)> = data_folders
        .iter()
        .flat_map(|folder| cache_files(folder))
        .filter_map(|file| Some((fs::metadata(&file).ok()?.modified().ok()?, file)))
        .collect();
    // Newest cache first; a cache without a link falls back to the next one.
    caches.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    caches.into_iter().find_map(|(_, file)| {
        let bytes = fs::read(&file).ok()?;
        let url = last_link_in(&bytes)?;
        Some(FoundLink {
            link: GachaLink::parse(&url).ok()?,
            url,
            cache_file: file,
        })
    })
}

/// `webCaches/<version>/Cache/Cache_Data/data_2`, and the older layout
/// without a version folder.
fn cache_files(data_folder: &Path) -> Vec<PathBuf> {
    let web_caches = data_folder.join("webCaches");
    let mut roots = vec![web_caches.clone()];
    if let Ok(entries) = fs::read_dir(&web_caches) {
        roots.extend(entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
    }
    roots
        .into_iter()
        .map(|root| root.join("Cache/Cache_Data/data_2"))
        .filter(|file| file.is_file())
        .collect()
}

/// The last wish history link in a cache file. Entries are appended, so the
/// last one is the most recent.
fn last_link_in(bytes: &[u8]) -> Option<String> {
    let needle = b"https://";
    let mut last = None;
    let mut from = 0;
    while let Some(at) = find(&bytes[from..], needle).map(|i| from + i) {
        let end = bytes[at..]
            .iter()
            .position(|b| *b <= b' ' || *b >= 0x7f)
            .map_or(bytes.len(), |len| at + len);
        let candidate = String::from_utf8_lossy(&bytes[at..end]);
        let is_history = candidate.contains("authkey=")
            && (candidate.contains("getGachaLog") || candidate.contains("gacha"));
        if is_history {
            last = Some(candidate.into_owned());
        }
        from = end.max(at + needle.len());
    }
    last
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "https://webstatic-sea.hoyoverse.com/genshin/event/e20190909gacha-v3/index.html?authkey_ver=1&authkey=old%2Bkey&game_biz=hk4e_global#/log";
    const NEW: &str = "https://gs.hoyoverse.com/genshin/event/e20190909gacha-v3/index.html?authkey_ver=1&authkey=new%2Bkey&game_biz=hk4e_global#/log";

    fn cache_bytes() -> Vec<u8> {
        let mut bytes = b"\x00\x01junk1/0/".to_vec();
        bytes.extend_from_slice(OLD.as_bytes());
        bytes.extend_from_slice(b"\x00\x02https://example.com/not-it\x001/0/");
        bytes.extend_from_slice(NEW.as_bytes());
        bytes.extend_from_slice(b"\x00\xff\xfe trailing");
        bytes
    }

    fn write_cache(data_folder: &Path) {
        let dir = data_folder.join("webCaches/2.38.0.0/Cache/Cache_Data");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("data_2"), cache_bytes()).unwrap();
    }

    #[test]
    fn the_last_link_in_a_cache_wins() {
        assert_eq!(last_link_in(&cache_bytes()).as_deref(), Some(NEW));
        assert_eq!(last_link_in(b"nothing here"), None);
    }

    #[test]
    fn data_folders_are_read_from_the_log() {
        let log = "[Subsystems] Discovering subsystems at path \
                   C:/Program Files/Genshin Impact/Genshin Impact game/GenshinImpact_Data/UnitySubsystems\n\
                   Warmup file D:\\Games\\YuanShen\\YuanShen_Data\\StreamingAssets\\x\n";
        assert_eq!(
            data_folders_in_log(log),
            [
                "C:/Program Files/Genshin Impact/Genshin Impact game/GenshinImpact_Data",
                "D:/Games/YuanShen/YuanShen_Data",
            ]
        );
    }

    #[test]
    fn any_text_before_a_drive_is_read_safely() {
        // A multi-byte character, or an invalid byte read as U+FFFD, right
        // before ":/" is no drive letter, and must not split a character.
        let log = String::from_utf8_lossy(
            b"\xff:/GenshinImpact_Data\n\xc3\xa9:/GenshinImpact_Data\n\xe5\x8e\x9f:/YuanShen_Data\n\
              \xc3\xa9 E:/Genshin/GenshinImpact_Data\n",
        );
        assert_eq!(data_folders_in_log(&log), ["E:/Genshin/GenshinImpact_Data"]);
    }

    #[test]
    fn a_chosen_game_folder_is_searched() {
        let dir = tempfile::tempdir().unwrap();
        let install = dir.path().join("Genshin Impact");
        write_cache(&install.join("Genshin Impact game/GenshinImpact_Data"));

        let found = find_link_in_game_folder(&install).unwrap();
        assert_eq!(found.url, NEW);
        assert!(find_link_in_game_folder(dir.path()).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn wine_prefixes_are_searched_through_their_drive_links() {
        let home = tempfile::tempdir().unwrap();
        let prefix = home.path().join(".local/share/anime-game-launcher/prefix");
        fs::create_dir_all(prefix.join("dosdevices")).unwrap();
        fs::create_dir_all(prefix.join("drive_c")).unwrap();
        std::os::unix::fs::symlink("../drive_c", prefix.join("dosdevices/c:")).unwrap();

        let logs = prefix.join("drive_c/users/steamuser/AppData/LocalLow/miHoYo/Genshin Impact");
        fs::create_dir_all(&logs).unwrap();
        fs::write(
            logs.join("output_log.txt"),
            "Something at C:/Games/Genshin Impact game/GenshinImpact_Data/Managed\n",
        )
        .unwrap();
        write_cache(&prefix.join("drive_c/Games/Genshin Impact game/GenshinImpact_Data"));

        let folders = wine_data_folders(home.path());
        assert_eq!(folders.len(), 1);
        let found = latest_link(&folders).unwrap();
        assert_eq!(found.url, NEW);
    }
}
