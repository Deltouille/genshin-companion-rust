//! The dialogs other parts of the app open: the menu's commands (imports,
//! the HoYoLAB sync, the GOOD import), a page asking another (Resources opening
//! the planner's inventory dialog), and the screenshot tool, which opens each
//! dialog it shoots this way. Each one first shows the page the dialog belongs
//! to, then asks that page's entity to open it.

use gpui_kit::component::WindowExt as _;
use gpui_kit::{AppContext as _, Context, Entity, Focusable as _, ParentElement as _, Window};
use wish_core::model::{AccountId, Pool};
use wish_core::planner::MaterialId;
use wish_ui::form_dialog;

use super::{ArchivePage, ImportFile, ImportFromGame, ImportGood, Page, Shell, SyncFromHoyolab};
use crate::app::library::LoadState;
use crate::features::imports::good::GoodImport;
use crate::features::imports::hoyolab::HoyolabSync;

impl Shell {
    /// Opens the character list's filter sheet (the screenshot tool uses it).
    pub fn open_character_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(Page::Archive(ArchivePage::Characters), cx);
        self.archive
            .update(cx, |archive, cx| archive.open_filters(window, cx));
    }

    /// Opens the planner's custom character or weapon dialog: for a new one,
    /// or plan entry `id`'s.
    pub fn open_custom_entry(
        &mut self,
        id: Option<i64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigate(Page::Planner, cx);
        self.planner
            .update(cx, |planner, cx| planner.open_custom_entry(id, window, cx));
    }

    /// Opens a new custom character's dialog on the choice of its `role`'s
    /// material (the screenshot tool uses it).
    pub fn open_custom_picker(
        &mut self,
        role: wish_core::planner::Role,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigate(Page::Planner, cx);
        self.planner.update(cx, |planner, cx| {
            planner.open_custom_picker(role, window, cx)
        });
    }

    /// Opens the planner's "Add a character" (or, with `weapons`, "Add a
    /// weapon") dialog (the screenshot tool uses it).
    pub fn open_planner_add(&mut self, weapons: bool, window: &mut Window, cx: &mut Context<Self>) {
        use crate::features::planner::AddKind;
        self.navigate(Page::Planner, cx);
        let kind = if weapons {
            AddKind::Weapon
        } else {
            AddKind::Character
        };
        self.planner
            .update(cx, |planner, cx| planner.open_add(kind, window, cx));
    }

    /// Asks how many of material `id` the player has.
    pub fn open_inventory(&mut self, id: MaterialId, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(Page::Planner, cx);
        self.planner
            .update(cx, |planner, cx| planner.open_inventory_of(id, window, cx));
    }

    /// Opens the options of the planner's resin estimate (the screenshot tool
    /// uses it).
    pub fn open_resin_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(Page::Planner, cx);
        self.planner
            .update(cx, |planner, cx| planner.open_resin_options(window, cx));
    }

    /// Opens or closes the move menu of the plan's entry `at` (from 0), as
    /// its grip does (the screenshot tool uses it).
    pub fn show_plan_moves(
        &mut self,
        at: usize,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigate(Page::Planner, cx);
        let account = self.session.read(cx).account();
        let entry = (self.library.read(cx).plan(account).get(at)).map(|entry| entry.id);
        if let Some(id) = entry {
            self.planner
                .update(cx, |planner, cx| planner.show_moves(id, open, window, cx));
        }
    }

    /// Shows the open-source licences, over the Credits page.
    pub fn open_licenses(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(Page::Credits, cx);
        crate::features::credits::open_licenses(window, cx);
    }

    /// Opens Settings and the account form: for a new account, or to edit
    /// `account` (the screenshot tool uses it).
    pub fn open_account(
        &mut self,
        account: Option<AccountId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigate(Page::Settings, cx);
        let account = account
            .and_then(|id| self.library.read(cx).account(id))
            .map(|summary| summary.account.clone());
        self.settings.update(cx, |settings, cx| {
            settings.open_account(account, window, cx)
        });
    }

    /// Opens "Add a 4★ or 5★" for `pool` (counting by hand).
    pub fn open_add_item(&mut self, pool: Pool, window: &mut Window, cx: &mut Context<Self>) {
        self.history
            .update(cx, |history, cx| history.open_add_item(pool, window, cx));
    }

    pub(super) fn import_file(
        &mut self,
        _: &ImportFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.import
            .update(cx, |import, cx| import.import_file(window, cx));
    }

    pub(super) fn import_from_game(
        &mut self,
        _: &ImportFromGame,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.import
            .update(cx, |import, cx| import.import_from_game(window, cx));
    }

    pub(super) fn sync_from_hoyolab(
        &mut self,
        _: &SyncFromHoyolab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_hoyolab_sync(window, cx);
    }

    pub(super) fn import_good(
        &mut self,
        _: &ImportGood,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_good_import(window, cx);
    }

    /// Opens the GOOD import: the file (or the clipboard), then what it
    /// changes.
    pub fn open_good_import(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<GoodImport>> {
        if !matches!(self.library.read(cx).state(), LoadState::Ready) {
            return None;
        }
        let (library, session) = (self.library.clone(), self.session.clone());
        let view = cx.new(|_| GoodImport::new(library, session));
        let shown = view.clone();
        window.open_dialog(cx, move |dialog, window, _| {
            dialog
                .title(t!("app-import-good-title"))
                .w(window.rem_size() * 44.)
                .child(shown.clone())
        });
        Some(view)
    }

    /// Opens the GOOD import on `text` at once, for the screenshot tool's
    /// preview (it gives a sample file).
    pub fn open_good_preview(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = self.open_good_import(window, cx) {
            view.update(cx, |view, cx| {
                view.read(crate::app::library::GoodSource::Text(text), window, cx);
            });
        }
    }

    /// Opens the HoYoLAB sync: the cookie, the fetch, then what it changes
    /// (the screenshot tool opens it this way).
    pub fn open_hoyolab_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_hoyolab_dialog(window, cx);
    }

    /// Opens the HoYoLAB dialog and fetches with `cookie` at once, for the
    /// screenshot tool's preview (it gives a fake HoYoLAB).
    pub fn open_hoyolab_preview(
        &mut self,
        cookie: wish_core::hoyolab::Cookie,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = self.open_hoyolab_dialog(window, cx) {
            view.update(cx, |view, cx| view.start_fetch(cookie, window, cx));
        }
    }

    fn open_hoyolab_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<HoyolabSync>> {
        if !matches!(self.library.read(cx).state(), LoadState::Ready) {
            return None;
        }
        let (library, session) = (self.library.clone(), self.session.clone());
        let view = cx.new(|cx| HoyolabSync::new(library, session, window, cx));
        let first = view.read(cx).first_field().clone();
        let focus_once = std::cell::Cell::new(true);
        let shown = view.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            if focus_once.replace(false) {
                first.focus_handle(cx).focus(window, cx);
            }
            form_dialog(dialog)
                .title(t!("app-sync-hoyolab-title"))
                .w(window.rem_size() * 44.)
                .child(shown.clone())
        });
        Some(view)
    }
}
