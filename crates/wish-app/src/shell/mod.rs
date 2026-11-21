//! The main window: the sidebar beside the current page.
//!
//! [`Shell`] assembles the window and routes between pages; it holds no page
//! logic. See the crate docs for how data flows between the shell, the
//! [`Session`], the [`Library`] and the pages. Its parts:
//!
//! - `page`: the destinations ([`Page`], [`Feature`] and each feature's page
//!   enum);
//! - `navigation`: moving between pages, and the account picker;
//! - `dialogs`: the dialogs opened from outside their page (the menu's
//!   commands, the screenshot tool, one page asking another);
//! - `sidebar`: the sidebar.

mod dialogs;
mod navigation;
mod page;
mod sidebar;

pub use page::{ArchivePage, Feature, OverviewTab, Page, WishPage};

use std::collections::HashSet;
use std::rc::Rc;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, Styled as _, Subscription, Window,
    actions, deferred, div,
};
use wish_core::model::AccountId;

use crate::app::library::{Library, LoadState};
use crate::app::{Notices, Session};
use crate::features::archive::{CharactersView, EditMaterial, ResourcesView, WeaponsView};
use crate::features::credits::Credits;
use crate::features::imports::Imports;
use crate::features::overview::Overview;
use crate::features::planner::PlannerView;
use crate::features::settings::SettingsPage;
use crate::features::wishes::history::HistoryView;
use crate::features::wishes::import::ImportFlow;
use wish_ui::button;
use wish_ui::{PickerEvent, PickerState};

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

actions!(
    genshin_companion,
    [
        ImportFile,
        ImportFromGame,
        SyncFromHoyolab,
        ImportGood,
        Quit
    ]
);

/// The window: sidebar, routing, and the pages it switches between.
///
/// The pages with state of their own (the wish history, the planner, the
/// archive's three lists, settings) and the import flow are entities created
/// once here and kept, so their state survives switching pages. The stateless
/// ones (the overview, the Import page, credits) are built on each render from
/// the library and the session.
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
    resources: Entity<ResourcesView>,
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
        // A material's count is set in the planner's inventory dialog.
        let shell = cx.weak_entity();
        let edit: EditMaterial = std::rc::Rc::new(move |id, window, cx| {
            shell
                .update(cx, |shell, cx| shell.open_inventory(id, window, cx))
                .ok();
        });
        let resources = cx.new(|cx| ResourcesView::new(session.clone(), edit, window, cx));
        let settings = cx.new(|cx| SettingsPage::new(session.clone(), cx));
        let import = cx.new(|cx| ImportFlow::new(session.clone(), cx));
        let notices = Notices::new(cx);
        let image_service = library.read(cx).image_service().clone();
        let subscriptions = vec![
            wish_ui::theme::follow_system(window),
            // An image arrived: the window draws it, wherever a page or a
            // dialog shows it. Only a redraw: the library's observers, which
            // rebuild tables and sliders, aren't woken.
            cx.observe(&image_service, |_, _, cx| cx.notify()),
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
            resources,
            settings,
            import,
            notices,
            focus_handle,
            _subscriptions: subscriptions,
        };
        shell.sync_account_picker(window, cx);
        shell
    }

    /// The page currently shown.
    pub fn page(&self) -> Page {
        self.page
    }

    /// Whether the user folded `feature`'s group in the sidebar.
    pub fn is_folded(&self, feature: Feature) -> bool {
        self.collapsed.contains(&feature)
    }

    /// The selected account and the flow in progress.
    pub fn session(&self) -> &Entity<Session> {
        &self.session
    }

    /// The window's messages (snackbars), for the screenshot tool and tests.
    pub fn notices(&self) -> &Entity<Notices> {
        &self.notices
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
                        .child(t!("app-opening")),
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
            Page::Archive(ArchivePage::Resources) => self.resources.clone().into_any_element(),
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
            archive: library.archive().clone(),
            resin: library.settings(cx).resin.clone(),
            images: library.images(cx).clone(),
            manual_counting: library.settings(cx).manual_counting,
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
                        .child(t!("app-open-failed")),
                )
                .child(div().text_sm().child(message))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(t!(
                            "app-open-failed-help",
                            folder = folder.display().to_string()
                        )),
                )
                .child(
                    div().child(
                        button("show-data-folder")
                            .label(t!("app-show-data-folder"))
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
            .on_action(cx.listener(Self::import_good))
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
