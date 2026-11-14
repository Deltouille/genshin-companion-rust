//! App preferences, kept in `<root>/settings.json` next to the database.
//!
//! They describe this machine (folder paths), not the wish data, so they are
//! not part of the database or its backups.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::paths::{AppPaths, write_atomic};
use crate::planner::ResinOptions;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Settings {
    /// Folder that receives a snapshot after every change, typically one
    /// synced by Syncthing, Nextcloud, Dropbox or similar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror_folder: Option<PathBuf>,
    /// Game folder chosen by the user to find the wish history link, when it
    /// is not found automatically (a Wine prefix in an unusual place).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_folder: Option<PathBuf>,
    /// Show the controls to count wishes by hand, like paimon.moe's manual mode.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub manual_counting: bool,
    /// How the planner estimates the resin farming takes.
    #[serde(default, skip_serializing_if = "ResinOptions::is_default")]
    pub resin: ResinOptions,
}

impl Settings {
    /// Reads the settings, or the defaults when there is no settings file yet.
    pub fn load(paths: &AppPaths) -> Result<Self> {
        let path = paths.settings();
        match fs::read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(Error::io("reading", &path)(e)),
        }
    }

    /// Reads the settings; when the file can't be read, the defaults, and
    /// why. A file that doesn't parse is first renamed
    /// `settings.json.<UTC time>.damaged`: the next save would otherwise
    /// replace it, and with it the mirror folder the user chose, without a
    /// word. The time keeps an earlier damaged file from being replaced.
    pub fn load_or_set_aside(paths: &AppPaths) -> (Self, Option<String>) {
        let err = match Self::load(paths) {
            Ok(settings) => return (settings, None),
            Err(err) => err,
        };
        let path = paths.settings();
        let warning = if matches!(err, Error::Json(_)) {
            let time = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
            let aside = path.with_extension(format!("json.{time}.damaged"));
            match fs::rename(&path, &aside) {
                Ok(()) => format!(
                    "settings.json could not be read ({err}); it was kept as {} and the \
                     defaults are used",
                    aside.display()
                ),
                Err(e) => format!("settings.json could not be read ({err}), nor set aside ({e})"),
            }
        } else {
            format!("settings.json could not be read ({err}); the defaults are used")
        };
        (Self::default(), Some(warning))
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        let mut json = serde_json::to_vec_pretty(self).expect("settings always serialize");
        json.push(b'\n');
        write_atomic(&paths.settings(), &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_default_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::at(dir.path());
        assert_eq!(Settings::load(&paths).unwrap(), Settings::default());

        let settings = Settings {
            mirror_folder: Some(dir.path().join("Sync")),
            game_folder: None,
            manual_counting: true,
            resin: ResinOptions {
                world_level: 9,
                ..ResinOptions::default()
            },
        };
        settings.save(&paths).unwrap();
        assert_eq!(Settings::load(&paths).unwrap(), settings);
    }

    #[test]
    fn a_damaged_file_is_set_aside_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::at(dir.path());
        fs::write(paths.settings(), "{ \"mirror_folder\": ").unwrap();
        let (settings, warning) = Settings::load_or_set_aside(&paths);
        assert_eq!(settings, Settings::default());
        assert!(warning.unwrap().contains(".damaged"));
        let aside: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.to_string_lossy().ends_with(".damaged"))
            .collect();
        assert_eq!(aside.len(), 1, "{aside:?}");
        let name = aside[0].file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("settings.json."), "{name}");
        assert_eq!(
            fs::read_to_string(&aside[0]).unwrap(),
            "{ \"mirror_folder\": "
        );
        assert!(!paths.settings().exists());
    }
}
