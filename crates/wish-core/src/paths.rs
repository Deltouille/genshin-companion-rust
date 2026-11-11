use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Where the app keeps its files. Everything lives under one root folder:
///
/// ```text
/// <root>/wishes.db      the database (source of truth)
/// <root>/backups/       automatic database backups
/// <root>/imports/       a copy of every imported file, as received
/// <root>/game-data/     optional newer game data (items.json, banners.json…)
/// <root>/cache/images/  the game's images, downloaded on first use
/// <root>/settings.json  app preferences for this machine
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppPaths {
    root: PathBuf,
}

impl AppPaths {
    /// The OS data folder: `~/.local/share/genshin-companion` on Linux,
    /// `%APPDATA%\genshin-companion\data` on Windows and
    /// `~/Library/Application Support/genshin-companion` on macOS.
    ///
    /// The app was called Genshin Wishes before: when only the folder of that
    /// name exists, it is copied to the new one first (see
    /// [`copy_data_folder`]), and left as it was.
    pub fn from_os() -> Result<Self> {
        let folder = |name: &str| {
            directories::ProjectDirs::from("", "", name)
                .map(|dirs| dirs.data_dir().to_path_buf())
                .ok_or(Error::NoDataDir)
        };
        let root = folder(APP_FOLDER)?;
        if !root.exists() {
            let former = folder(FORMER_APP_FOLDER)?;
            if former.is_dir() {
                copy_data_folder(&former, &root)?;
            }
        }
        Ok(Self::at(root))
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn database(&self) -> PathBuf {
        self.root.join("wishes.db")
    }

    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }

    pub fn imports(&self) -> PathBuf {
        self.root.join("imports")
    }

    pub fn game_data(&self) -> PathBuf {
        self.root.join("game-data")
    }

    /// The game's images, downloaded on first use (see [`crate::images`]):
    /// nothing there is the player's, and all of it can be downloaded again.
    pub fn image_cache(&self) -> PathBuf {
        self.root.join("cache").join("images")
    }

    pub fn settings(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    /// Takes the data folder for this process: `None` when another one
    /// already has it (the app is open). It stays taken while the returned
    /// file is open, so until the process ends, even by a crash.
    pub fn lock(&self) -> Result<Option<File>> {
        fs::create_dir_all(&self.root).map_err(Error::io("creating", &self.root))?;
        let path = self.root.join(".lock");
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(Error::io("opening", &path))?;
        match file.try_lock() {
            Ok(()) => Ok(Some(file)),
            Err(fs::TryLockError::WouldBlock) => Ok(None),
            Err(fs::TryLockError::Error(e)) => Err(Error::io("locking", &path)(e)),
        }
    }
}

/// The data folder's name under the OS data folder.
const APP_FOLDER: &str = "genshin-companion";
/// Its name when the app was called Genshin Wishes.
const FORMER_APP_FOLDER: &str = "genshin-wishes";

/// Copies a data folder to `to`, which must not exist, leaving `from` as it
/// is: into a temporary folder beside `to`, every file checked against its
/// original's size, then renamed into place. `to` is thus either absent or
/// complete, even after a crash midway.
pub fn copy_data_folder(from: &Path, to: &Path) -> Result<()> {
    if to.exists() {
        return Err(Error::io("copying the data folder to", to)(
            std::io::ErrorKind::AlreadyExists.into(),
        ));
    }
    let parent = to
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(Error::io("creating", parent))?;
    let name = to
        .file_name()
        .map_or_else(Default::default, |n| n.to_string_lossy());
    let tmp = parent.join(format!(".{name}.copying"));
    if tmp.exists() {
        // Left by an interrupted copy: start over.
        fs::remove_dir_all(&tmp).map_err(Error::io("removing", &tmp))?;
    }
    copy_tree(from, &tmp)?;
    fs::rename(&tmp, to).map_err(Error::io("renaming", &tmp))?;
    Ok(())
}

/// Copies the files and folders under `from` to `to`, checking each file.
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).map_err(Error::io("creating", to))?;
    for entry in fs::read_dir(from).map_err(Error::io("reading", from))? {
        let entry = entry.map_err(Error::io("reading", from))?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        let kind = entry.file_type().map_err(Error::io("reading", &source))?;
        if kind.is_dir() {
            copy_tree(&source, &target)?;
        } else if kind.is_file() {
            let copied = fs::copy(&source, &target).map_err(Error::io("copying", &source))?;
            let original = fs::metadata(&source)
                .map_err(Error::io("reading", &source))?
                .len();
            if copied != original {
                return Err(Error::io("copying", &source)(std::io::Error::other(
                    format!("copied {copied} of {original} bytes"),
                )));
            }
        }
    }
    Ok(())
}

/// Writes `bytes` to `path` so that readers see either the old file or the
/// complete new one, even after a crash: temp file, fsync, rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(dir).map_err(Error::io("creating", dir))?;

    let file_name = path
        .file_name()
        .ok_or_else(|| Error::io("writing", path)(std::io::ErrorKind::InvalidInput.into()))?;
    let tmp = dir.join(format!(".{}.tmp", file_name.to_string_lossy()));

    let mut file = File::create(&tmp).map_err(Error::io("creating", &tmp))?;
    file.write_all(bytes).map_err(Error::io("writing", &tmp))?;
    file.sync_all().map_err(Error::io("flushing", &tmp))?;
    drop(file);

    fs::rename(&tmp, path).map_err(Error::io("replacing", path))?;
    sync_dir(dir)
}

/// Makes a rename in `dir` durable. Directories cannot be opened on Windows,
/// where the rename is already durable once it returns.
fn sync_dir(dir: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(Error::io("flushing", dir))?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_process_at_a_time_has_the_data_folder() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::at(dir.path().join("data"));
        let first = paths.lock().unwrap();
        assert!(first.is_some());
        assert!(paths.lock().unwrap().is_none(), "already taken");
        drop(first);
        assert!(paths.lock().unwrap().is_some(), "free again");
    }

    #[test]
    fn write_atomic_creates_and_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/out.json");

        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"second");
        let leftovers: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, ["out.json"]);
    }

    #[test]
    fn a_data_folder_is_copied_whole_and_left_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("genshin-wishes");
        fs::create_dir_all(from.join("backups")).unwrap();
        fs::write(from.join("wishes.db"), b"database").unwrap();
        fs::write(from.join("backups/wishes-1.db"), b"backup").unwrap();
        let to = dir.path().join("new/genshin-companion");

        copy_data_folder(&from, &to).unwrap();
        assert_eq!(fs::read(to.join("wishes.db")).unwrap(), b"database");
        assert_eq!(fs::read(to.join("backups/wishes-1.db")).unwrap(), b"backup");
        // The former folder is untouched, and no temporary folder is left.
        assert_eq!(fs::read(from.join("wishes.db")).unwrap(), b"database");
        let beside: Vec<_> = fs::read_dir(to.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(beside, ["genshin-companion"]);
        // Never over an existing folder.
        assert!(copy_data_folder(&from, &to).is_err());
    }
}
