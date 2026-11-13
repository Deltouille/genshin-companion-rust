//! The main window: the sidebar beside the current page.
//!
//! `Shell` assembles the window and routes between pages; it holds no page
//! logic. See the crate docs for how data flows between the shell, the
//! [`Session`], the [`Library`] and the pages.

mod sidebar;

/// The sidebar's width, in rems.
pub(crate) const SIDEBAR_REMS: f32 = 16.;
/// A page's side padding (`px_6`, both sides), in rems: with the sidebar,
/// what a page's content does not get of the window's width.
pub(crate) const PAGE_PADDING_REMS: f32 = 3.;

/// When the messages (snackbars) paint among the window's deferred layers:
/// after GPUI Kit's dialogs (10 and up, one per dialog open), before its
/// popups (100: a select's list, a menu) and tooltips (200).
const NOTICES_PRIORITY: usize = 50;

/// The width a page's content has in `window`.
pub(crate) fn content_width(window: &gpui_kit::Window) -> gpui_kit::Pixels {
    window.viewport_size().width - window.rem_size() * (SIDEBAR_REMS + PAGE_PADDING_REMS)
}

use std::collections::HashSet;
use std::rc::Rc;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::WindowExt as _;

use gpui_kit::component::spinner::Spinner;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, Focusable as _, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, Window, actions, deferred, div,
};
use wish_core::model::{AccountId, Pool};
use wish_core::planner::MaterialId;

use crate::app::library::{Library, LoadState};
use crate::app::{Notices, Session};
use crate::features::archive::{CharactersView, WeaponsView};
use crate::features::credits::Credits;
use crate::features::imports::Imports;
use crate::features::imports::hoyolab::HoyolabSync;
use crate::features::overview::Overview;
use crate::features::planner::PlannerView;
use crate::features::settings::SettingsPage;
use crate::features::wishes::history::HistoryView;
use crate::features::wishes::import::ImportFlow;
use sidebar::account_option;
use wish_ui::button;
use wish_ui::{PickerEvent, PickerItem, PickerState};

actions!(
    genshin_companion,
    [ImportFile, ImportFromGame, SyncFromHoyolab, Quit]
);

/// A destination in the sidebar: a page of a feature, or one of the app's own
/// pages at the foot of the sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// The app's home: the selected account in figures, one tab each.
    Overview(OverviewTab),
    Wishes(WishPage),
    Archive(ArchivePage),
    /// The ascension planner.
    Planner,
    /// Every way to bring data in.
    Imports,
    Settings,
    Credits,
}

/// The overview's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverviewTab {
    /// Key figures, a card per banner type, wishes and pity over time.
    #[default]
    Wishes,
    /// The characters owned, against the game's.
    Characters,
    /// The weapons owned, against the game's.
    Weapons,
}

impl OverviewTab {
    pub const ALL: [OverviewTab; 3] = [Self::Wishes, Self::Characters, Self::Weapons];

    pub fn label(self) -> &'static str {
        match self {
            Self::Wishes => "Wishes",
            Self::Characters => "Characters",
            Self::Weapons => "Weapons",
        }
    }
}

/// The wish counter's pages: one per banner type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WishPage {
    History(Pool),
}

/// The archive's pages: the game's characters and weapons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchivePage {
    /// Every character, and one character's page when one is open.
    Characters,
    /// Every weapon, and one weapon's page when one is open.
    Weapons,
}

/// A feature: a group in the sidebar, holding its pages. A new one (an
/// ascension planner…) adds a variant here and one to [`Page`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Feature {
    Wishes,
    Archive,
    Planner,
}

impl Page {
    /// The feature the page belongs to; `None` for the app's own pages.
    pub fn feature(self) -> Option<Feature> {
        match self {
            Page::Wishes(_) => Some(Feature::Wishes),
            Page::Archive(_) => Some(Feature::Archive),
            Page::Planner => Some(Feature::Planner),
            Page::Overview(_) | Page::Imports | Page::Settings | Page::Credits => None,
        }
    }
}

impl Feature {
    /// The page the feature opens on. The wish counter opens on its first
    /// banner type; without wishes, the shell shows the overview instead.
    pub fn home(self) -> Page {
        match self {
            Feature::Wishes => Page::Wishes(WishPage::History(Pool::CharacterEvent)),
            Feature::Archive => Page::Archive(ArchivePage::Characters),
            Feature::Planner => Page::Planner,
        }
    }
}

/// The window: sidebar, routing, and the pages it switches between.
///
/// The pages with state of their own (history, settings) and the import
/// flow are entities created once here and kept, so their state survives
/// switching pages. The stateless ones (overview, credits) are built on
/// each render from the library and the session.
pub struct Shell {
    library: Entity<Library>,
    session: Entity<Session>,
    page: Page,
    /// The overview's tab last shown, which its sidebar entry opens again.
    overview_tab: OverviewTab,
    /// Feature groups the user folded in the sidebar.
    collapsed: HashSet<Feature>,
    account_select: Entity<PickerState<AccountId>>,
    history: Entity<HistoryView>,
    archive: Entity<CharactersView>,
    weapons: Entity<WeaponsView>,
    planner: Entity<PlannerView>,
    settings: Entity<SettingsPage>,
    import: Entity<ImportFlow>,
    /// Messages about work done in the background, drawn over everything.
    notices: Entity<Notices>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Shell {
    pub fn new(library: Entity<Library>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let session = cx.new(|cx| Session::new(library.clone(), cx));
        let account_select = cx.new(|cx| PickerState::new(Vec::new(), None, cx));
        let history = cx.new(|cx| HistoryView::new(session.clone(), window, cx));
        let archive = cx.new(|cx| CharactersView::new(session.clone(), window, cx));
        let weapons = cx.new(|cx| WeaponsView::new(session.clone(), window, cx));
        let planner = cx.new(|cx| PlannerView::new(session.clone(), window, cx));
        let settings = cx.new(|cx| SettingsPage::new(session.clone(), cx));
        let import = cx.new(|cx| ImportFlow::new(session.clone(), cx));
        let notices = Notices::new(cx);
        let subscriptions = vec![
            wish_ui::theme::follow_system(window),
            // New data: the picker lists the accounts again.
            cx.observe_in(&library, window, |this, _, window, cx| {
                this.sync_account_picker(window, cx)
            }),
            // Another account, or its data changed: the picker and the page
            // follow.
            cx.observe_in(&session, window, |this, _, window, cx| {
                this.sync_account_picker(window, cx)
            }),
            cx.subscribe_in(
                &account_select,
                window,
                |this, _, event: &PickerEvent<AccountId>, _, cx| {
                    let PickerEvent::Confirm(id) = event;
                    let id = *id;
                    this.session
                        .update(cx, |session, cx| session.select_account(id, cx));
                },
            ),
        ];
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        let mut shell = Self {
            library,
            session,
            page: Page::Overview(OverviewTab::Wishes),
            overview_tab: OverviewTab::Wishes,
            collapsed: HashSet::new(),
            account_select,
            history,
            archive,
            weapons,
            planner,
            settings,
            import,
            notices,
            focus_handle,
            _subscriptions: subscriptions,
        };
        shell.sync_account_picker(window, cx);
        shell
    }

    /// Lists the accounts in the picker, selects the session's account, and
    /// leaves a history page the account has no wishes for.
    fn sync_account_picker(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let options: Vec<PickerItem<AccountId>> = self
            .library
            .read(cx)
            .accounts()
            .iter()
            .map(account_option)
            .collect();
        let account = self.session.read(cx).account();
        self.account_select.update(cx, |select, cx| {
            select.set_items(options, cx);
            if let Some(id) = account {
                select.set_selected_value(&id, cx);
            }
        });
        if let Page::Wishes(WishPage::History(pool)) = self.page
            && !self.pools(cx).contains(&pool)
        {
            self.page = self.first_wish_page(cx);
        }
        cx.notify();
    }

    /// The page currently shown.
    pub fn page(&self) -> Page {
        self.page
    }

    /// Whether the user folded `feature`'s group in the sidebar.
    pub fn is_folded(&self, feature: Feature) -> bool {
        self.collapsed.contains(&feature)
    }

    /// The account whose wishes are shown.
    pub fn account(&self, cx: &App) -> Option<AccountId> {
        self.session.read(cx).account()
    }

    /// Opens `page`; for a banner type the selected account has no wishes
    /// in, its first one with wishes, or the overview.
    pub fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = match page {
            Page::Wishes(WishPage::History(pool)) if !self.pools(cx).contains(&pool) => {
                self.first_wish_page(cx)
            }
            page => page,
        };
        if let Page::Overview(tab) = self.page {
            self.overview_tab = tab;
        }
        // The group of the page shown stays open.
        if let Some(feature) = self.page.feature() {
            self.collapsed.remove(&feature);
        }
        if let Page::Wishes(WishPage::History(pool)) = self.page {
            self.history
                .update(cx, |history, cx| history.set_pool(pool, cx));
        }
        // The sidebar entry opens the list, even from a character's page.
        if self.page == Page::Archive(ArchivePage::Weapons) {
            self.weapons.update(cx, |weapons, cx| weapons.show_list(cx));
        }
        if self.page == Page::Archive(ArchivePage::Characters) {
            self.archive.update(cx, |archive, cx| archive.show_list(cx));
        }
        cx.notify();
    }

    /// Opens the overview on the tab it showed last (the sidebar's entry).
    pub fn open_overview(&mut self, cx: &mut Context<Self>) {
        self.navigate(Page::Overview(self.overview_tab), cx);
    }

    /// A feature group's header was clicked: away from the feature, it opens
    /// the feature; on one of its pages, it folds or unfolds the group.
    pub fn open_or_fold(&mut self, feature: Feature, cx: &mut Context<Self>) {
        if self.page.feature() == Some(feature) {
            if !self.collapsed.remove(&feature) {
                self.collapsed.insert(feature);
            }
            cx.notify();
        } else {
            self.navigate(feature.home(), cx);
        }
    }

    /// Opens the page of the character `key` (a genshin-db key).
    pub fn open_character(&mut self, key: &str, cx: &mut Context<Self>) {
        self.navigate(Page::Archive(ArchivePage::Characters), cx);
        self.archive
            .update(cx, |archive, cx| archive.show_character(key, cx));
    }

    /// Opens the character list's filter sheet (the screenshot tool uses it).
    pub fn open_character_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(Page::Archive(ArchivePage::Characters), cx);
        self.archive
            .update(cx, |archive, cx| archive.open_filters(window, cx));
    }

    /// Opens the planner's "custom entry" dialog.
    /// Opens the custom character dialog: for a new one, or plan entry `id`.
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

    /// The window's messages (snackbars), for the screenshot tool and tests.
    /// The selected account and the flow in progress.
    pub fn session(&self) -> &Entity<Session> {
        &self.session
    }

    pub fn notices(&self) -> &Entity<Notices> {
        &self.notices
    }

    /// Shows the open-source licences, over the Credits page.
    pub fn open_licenses(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(Page::Credits, cx);
        crate::features::credits::open_licenses(window, cx);
    }

    /// Opens weapon `key`'s page in the archive.
    pub fn open_weapon(&mut self, key: &str, cx: &mut Context<Self>) {
        self.navigate(Page::Archive(ArchivePage::Weapons), cx);
        self.weapons
            .update(cx, |weapons, cx| weapons.show_weapon(key, cx));
    }

    /// On a history page, shows the banner view (`true`) or the wish list.
    pub fn show_banners(&mut self, banners: bool, cx: &mut Context<Self>) {
        self.history
            .update(cx, |history, cx| history.show_banners(banners, cx));
    }

    /// Opens "Add a 4★ or 5★" for `pool` (counting by hand).
    pub fn open_add_item(&mut self, pool: Pool, window: &mut Window, cx: &mut Context<Self>) {
        self.history
            .update(cx, |history, cx| history.open_add_item(pool, window, cx));
    }

    fn pools(&self, cx: &App) -> Vec<Pool> {
        self.session.read(cx).pools(cx)
    }

    /// The wish counter's first banner type with wishes, or the overview's
    /// Wishes tab when there is none.
    fn first_wish_page(&self, cx: &App) -> Page {
        self.pools(cx)
            .first()
            .map_or(Page::Overview(OverviewTab::Wishes), |pool| {
                Page::Wishes(WishPage::History(*pool))
            })
    }

    fn import_file(&mut self, _: &ImportFile, window: &mut Window, cx: &mut Context<Self>) {
        self.import
            .update(cx, |import, cx| import.import_file(window, cx));
    }

    fn import_from_game(
        &mut self,
        _: &ImportFromGame,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.import
            .update(cx, |import, cx| import.import_from_game(window, cx));
    }

    fn sync_from_hoyolab(
        &mut self,
        _: &SyncFromHoyolab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_hoyolab_sync(window, cx);
    }

    /// Opens the HoYoLAB sync: the cookie, the fetch, then what it changes
    /// (the screenshot tool opens it this way).
    pub fn open_hoyolab_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.library.read(cx).state(), LoadState::Ready) {
            return;
        }
        let (library, session) = (self.library.clone(), self.session.clone());
        let view = cx.new(|cx| HoyolabSync::new(library, session, window, cx));
        let first = view.read(cx).first_field().clone();
        let focus_once = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if focus_once.replace(false) {
                first.focus_handle(cx).focus(window, cx);
            }
            dialog
                .title("Sync from HoYoLAB")
                .w(window.rem_size() * 44.)
                .child(view.clone())
        });
    }

    fn render_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        match self.library.read(cx).state() {
            LoadState::Loading => {
                return centered(
                    div()
                        .h_flex()
                        .gap_2()
                        .text_color(cx.theme().muted_foreground)
                        .child(Spinner::new())
                        .child("Opening your wish history…"),
                )
                .into_any_element();
            }
            LoadState::Failed(message) => return self.render_failure(message.clone(), cx),
            LoadState::Ready => {}
        }
        match self.page {
            Page::Settings => self.settings.clone().into_any_element(),
            Page::Credits => Credits.into_any_element(),
            Page::Imports => Imports::new(self.library.read(cx).is_busy()).into_any_element(),
            Page::Wishes(WishPage::History(_)) => self.history.clone().into_any_element(),
            Page::Archive(ArchivePage::Characters) => self.archive.clone().into_any_element(),
            Page::Archive(ArchivePage::Weapons) => self.weapons.clone().into_any_element(),
            Page::Planner => self.planner.clone().into_any_element(),
            Page::Overview(tab) => self.render_overview(tab, cx),
        }
    }

    /// The overview, built from the library and the session; its links come
    /// back here as navigation.
    fn render_overview(&self, tab: OverviewTab, cx: &mut Context<Self>) -> AnyElement {
        let shell = cx.entity().downgrade();
        let to_weapon = shell.clone();
        let session = self.session.read(cx);
        let (account, summary) = (session.account(), session.summary(cx).cloned());
        let library = self.library.read(cx);
        Overview {
            tab,
            summary,
            owned: library.owned(account).clone(),
            data: library.data().cloned(),
            planner: library.planner().clone(),
            images: library.images().clone(),
            manual_counting: library.settings().manual_counting,
            busy: library.is_busy(),
            on_navigate: Rc::new(move |page, _, cx| {
                shell.update(cx, |shell, cx| shell.navigate(page, cx)).ok();
            }),
            on_open_weapon: Rc::new(move |key, _, cx| {
                to_weapon
                    .update(cx, |shell, cx| shell.open_weapon(key, cx))
                    .ok();
            }),
        }
        .into_any_element()
    }

    fn render_failure(&self, message: SharedString, cx: &mut Context<Self>) -> AnyElement {
        let folder = self.library.read(cx).paths().root().to_path_buf();
        centered(
            div()
                .v_flex()
                .gap_3()
                .max_w_128()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Couldn’t open your wish history"),
                )
                .child(div().text_sm().child(message))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "Your data is untouched. It is stored in {}, with automatic backups in its backups folder.",
                            folder.display()
                        )),
                )
                .child(
                    div().child(
                        button("show-data-folder")
                            .label("Show data folder")
                            .on_click(move |_, _, cx| cx.reveal_path(&folder)),
                    ),
                ),
        )
        .into_any_element()
    }
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .h_flex()
            .items_stretch()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .track_focus(&self.focus_handle)
            .key_context("Shell")
            // Commands that work from any page and from the menu; the import
            // flow carries them out.
            .on_action(cx.listener(Self::import_file))
            .on_action(cx.listener(Self::import_from_game))
            .on_action(cx.listener(Self::sync_from_hoyolab))
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.render_page(cx)),
            )
            // Deferred, after the rest of the window and the dialogs, so a
            // message stays visible over a dialog.
            .child(deferred(self.notices.clone()).with_priority(NOTICES_PRIORITY))
    }
}

fn centered(content: impl IntoElement) -> impl IntoElement {
    div()
        .size_full()
        .h_flex()
        .justify_center()
        .p_6()
        .child(content)
}
