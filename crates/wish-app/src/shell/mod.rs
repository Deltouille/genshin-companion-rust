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

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;

use gpui_kit::component::spinner::Spinner;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, Window, actions, deferred, div, prelude::FluentBuilder as _,
};
use wish_core::model::{AccountId, Pool};
use wish_core::planner::MaterialId;

use crate::app::library::{Library, LoadState};
use crate::app::{Notices, Session};
use crate::features::archive::{CharactersView, WeaponsView};
use crate::features::credits::Credits;
use crate::features::planner::PlannerView;
use crate::features::settings::SettingsPage;
use crate::features::wishes::history::HistoryView;
use crate::features::wishes::import::{ImportFlow, import_file_button, import_game_button};
use crate::features::wishes::overview::Overview;
use sidebar::account_option;
use wish_ui::button;
use wish_ui::{PickerEvent, PickerItem, PickerState};

actions!(genshin_companion, [ImportFile, ImportFromGame, Quit]);

/// A destination in the sidebar: a page of a feature, or one of the app's own
/// pages at the foot of the sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Wishes(WishPage),
    Archive(ArchivePage),
    /// The ascension planner.
    Planner,
    Settings,
    Credits,
}

/// The wish counter's pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WishPage {
    Overview,
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
            Page::Settings | Page::Credits => None,
        }
    }
}

impl Feature {
    /// The page the feature opens on.
    pub fn home(self) -> Page {
        match self {
            Feature::Wishes => Page::Wishes(WishPage::Overview),
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
            page: Feature::Wishes.home(),
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
            self.page = Feature::Wishes.home();
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

    /// Opens `page`, or the overview when the selected account has no
    /// wishes in that pool.
    pub fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = match page {
            Page::Wishes(WishPage::History(pool)) if !self.pools(cx).contains(&pool) => {
                Feature::Wishes.home()
            }
            page => page,
        };
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
            Page::Wishes(WishPage::History(_)) => self.history.clone().into_any_element(),
            Page::Archive(ArchivePage::Characters) => self.archive.clone().into_any_element(),
            Page::Archive(ArchivePage::Weapons) => self.weapons.clone().into_any_element(),
            Page::Planner => self.planner.clone().into_any_element(),
            // An account added by hand may have no wishes yet.
            Page::Wishes(WishPage::Overview) => match self.session.read(cx).summary(cx).cloned() {
                Some(summary) if !summary.analysis.pools.is_empty() => {
                    let library = self.library.read(cx);
                    let shell = cx.entity().downgrade();
                    Overview::new(
                        summary,
                        library.data().cloned(),
                        library.images().clone(),
                        library.is_busy(),
                        move |page, _, cx| {
                            shell.update(cx, |shell, cx| shell.navigate(page, cx)).ok();
                        },
                    )
                    .into_any_element()
                }
                _ => self.render_empty(cx),
            },
        }
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> AnyElement {
        let library = self.library.read(cx);
        let (manual_counting, busy) = (library.settings().manual_counting, library.is_busy());
        centered(
            div()
                .v_flex()
                .items_center()
                .gap_4()
                .max_w_96()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("No wishes yet"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_center()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            "Import your wish history from the game, or your paimon.moe data: \
                             in paimon.moe, open Settings and choose Export, or download the \
                             save file from your Google Drive. Import both to keep the history \
                             paimon.moe holds from before the game’s last six months.",
                        ),
                )
                .when(manual_counting, |this| {
                    this.child(
                        div()
                            .text_sm()
                            .text_center()
                            .text_color(cx.theme().muted_foreground)
                            .child("Or open a banner in the sidebar to count your wishes by hand."),
                    )
                })
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .child(import_game_button().primary())
                        .child(import_file_button(busy)),
                ),
        )
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
