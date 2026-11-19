//! The player's settings on this computer (`settings.json`): the mirror
//! folder, the game folder, counting by hand, the resin options. Changed at
//! once in memory, saved in the background in order: an older save never
//! overwrites a newer one.
//!
//! An entity apart from the [`Library`](super::Library), which reads the
//! mirror folder for its writes. A change does wake the library's observers
//! (the library passes it on): a setting can change what the pages show,
//! counting by hand adds every banner type to the history.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use gpui_kit::{AppContext as _, Context, SharedString, Task};
use std::path::PathBuf;
use wish_core::AppPaths;
use wish_core::i18n::Language;
use wish_core::planner::ResinOptions;
use wish_core::settings::Settings;

/// The settings, changed through its methods only: at once in memory, saved in
/// the background in the order they were made.
pub struct Preferences {
    paths: AppPaths,
    settings: Settings,
    /// Why `settings.json` was set aside when the app opened, if it was.
    warning: Option<SharedString>,
    /// Changes made so far, in order.
    changes: u64,
    /// The latest change on disk.
    saved: Arc<Mutex<u64>>,
}

impl Preferences {
    /// The defaults, until the library has read the file
    /// ([`Preferences::set_read`]).
    pub(crate) fn new(paths: AppPaths) -> Self {
        Self {
            paths,
            settings: Settings::default(),
            warning: None,
            changes: 0,
            saved: Arc::default(),
        }
    }

    /// The settings as the library read them when it opened, and why the
    /// file was set aside if it was damaged.
    pub(crate) fn set_read(
        &mut self,
        settings: Settings,
        warning: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.settings = settings;
        self.warning = warning.map(Into::into);
        cx.notify();
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Why `settings.json` was set aside at startup, if it was.
    pub fn warning(&self) -> Option<&SharedString> {
        self.warning.as_ref()
    }

    /// Changes the settings at once, then saves them in the background. Each
    /// change builds on the ones before, saved or not, and the saves are
    /// written one at a time, the latest last.
    pub fn change(
        &mut self,
        change: impl FnOnce(&mut Settings),
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        change(&mut self.settings);
        cx.notify();
        self.changes += 1;
        let (settings, paths) = (self.settings.clone(), self.paths.clone());
        let (this_change, saved) = (self.changes, self.saved.clone());
        cx.background_spawn(async move {
            let mut latest = wish_core::service::lock(&saved);
            if *latest < this_change {
                settings.save(&paths)?;
                *latest = this_change;
            }
            Ok(())
        })
    }

    /// Turns the controls to count wishes by hand on or off.
    pub fn set_manual_counting(&mut self, on: bool, cx: &mut Context<Self>) -> Task<Result<()>> {
        self.change(|settings| settings.manual_counting = on, cx)
    }

    /// Changes how the planner estimates the resin farming takes.
    pub fn set_resin_options(
        &mut self,
        options: ResinOptions,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.change(|settings| settings.resin = options, cx)
    }

    /// Puts the interface in `language`, or the system's (`None`), at once.
    pub fn set_language(
        &mut self,
        language: Option<Language>,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let task = self.change(|settings| settings.language = language, cx);
        crate::apply_language(self.settings.language(), cx);
        task
    }

    /// Remembers the game folder the player pointed to.
    pub fn remember_game_folder(
        &mut self,
        folder: PathBuf,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.change(|settings| settings.game_folder = Some(folder), cx)
    }
}
