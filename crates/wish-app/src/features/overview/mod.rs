//! Overview: the app's home, the selected account in figures, one tab each:
//! its wishes (`wishes/`), the characters it owns (`characters.rs`) and the
//! weapons its wishes brought (`weapons.rs`).

mod characters;
mod weapons;
mod wishes;

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Div, FontWeight, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _,
    Window, div,
};
use wish_core::collection::Collection;
use wish_core::data::GameData;
use wish_core::planner::PlannerData;
use wish_ui::{PageFrame, Segmented, card, card_title};

use crate::app::images::Images;
use crate::app::library::AccountSummary;
use crate::features::wishes::import::{import_file_button, import_game_button};
use crate::shell::{OverviewTab, Page};

/// Called with the page to open: a banner type's history, another tab.
pub(crate) type NavigateHandler = Rc<dyn Fn(Page, &mut Window, &mut App)>;

/// Called with the key of the weapon whose archive page to open.
pub(crate) type OpenWeaponHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// The overview of the selected account.
///
/// It has no state of its own, so it is a plain component (`RenderOnce`)
/// that the shell builds each time with what it shows: the tab (part of the
/// page, so the shell's), the account's summary (the session's selected
/// account, if any) and what it owns, the game data and the images (the
/// library), and whether a write is running (to show the import button
/// busy). Opening another page or tab is routing, the shell's job, so it
/// calls back through `on_navigate` and `on_open_weapon`.
#[derive(IntoElement)]
pub struct Overview {
    pub tab: OverviewTab,
    pub summary: Option<AccountSummary>,
    /// The characters and weapons the selected account owns.
    pub owned: Collection,
    pub data: Option<Arc<GameData>>,
    pub planner: Arc<PlannerData>,
    pub images: Arc<Images>,
    pub manual_counting: bool,
    pub busy: bool,
    pub on_navigate: NavigateHandler,
    pub on_open_weapon: OpenWeaponHandler,
}

impl RenderOnce for Overview {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // An account added by hand may have no wishes yet.
        let with_wishes = self
            .summary
            .as_ref()
            .filter(|summary| !summary.analysis.pools.is_empty());
        let body = match self.tab {
            OverviewTab::Wishes => match with_wishes {
                Some(summary) => wishes::wishes_tab(
                    summary,
                    self.data.as_deref(),
                    &self.images,
                    &self.on_navigate,
                    window,
                    cx,
                ),
                None => no_wishes(self.manual_counting, self.busy, cx),
            },
            OverviewTab::Characters => characters::characters_tab(
                &self.owned,
                &self.planner,
                &self.images,
                &self.on_navigate,
                window,
                cx,
            ),
            OverviewTab::Weapons => weapons::weapons_tab(
                with_wishes,
                &self.planner,
                &self.images,
                &self.on_open_weapon,
                self.busy,
                window,
                cx,
            ),
        };

        let on_navigate = self.on_navigate.clone();
        let tabs = OverviewTab::ALL
            .iter()
            .fold(Segmented::new("overview-tab"), |tabs, tab| {
                tabs.option(tab.label())
            })
            .selected(
                OverviewTab::ALL
                    .iter()
                    .position(|tab| *tab == self.tab)
                    .unwrap_or(0),
            )
            .on_select(move |ix, window, cx| {
                if let Some(tab) = OverviewTab::ALL.get(*ix) {
                    on_navigate(Page::Overview(*tab), window, cx);
                }
            });

        let account = self.summary.as_ref().map(|summary| &summary.account);
        let title: SharedString = match account {
            Some(account) => account.label.clone().into(),
            None => "Overview".into(),
        };
        let frame = PageFrame::new(title, body).toolbar(div().h_flex().child(tabs));
        let frame = match account.and_then(|account| account.uid.as_ref()) {
            Some(uid) => frame.subtitle(format!("UID {uid}")),
            None => frame,
        };
        // Without wishes, the tab's own buttons say how to import them.
        frame.when(
            self.tab == OverviewTab::Wishes && with_wishes.is_some(),
            |frame| {
                frame
                    .action(import_game_button())
                    .action(import_file_button(self.busy))
            },
        )
    }
}

/// The Wishes tab of an account without wishes: how to import them.
fn no_wishes(manual_counting: bool, busy: bool, cx: &App) -> Div {
    let muted = cx.theme().muted_foreground;
    empty_state(
        "No wishes yet",
        "Import your wish history from the game, or your paimon.moe data: in paimon.moe, open \
         Settings and choose Export, or download the save file from your Google Drive. Import \
         both to keep the history paimon.moe holds from before the game’s last six months.",
        div()
            .v_flex()
            .items_center()
            .gap_4()
            .when(manual_counting, |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_center()
                        .text_color(muted)
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
        cx,
    )
}

/// What a tab shows with nothing to count: a title, where its figures come
/// from, and what to do to get some.
fn empty_state(
    title: &'static str,
    text: impl Into<SharedString>,
    next: impl IntoElement,
    cx: &App,
) -> Div {
    div().w_full().h_flex().justify_center().py_16().child(
        div()
            .v_flex()
            .items_center()
            .gap_4()
            .max_w_96()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .text_sm()
                    .text_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(text.into()),
            )
            .child(next),
    )
}

/// A card with a title over `content`.
fn section(title: &'static str, content: impl IntoElement, cx: &App) -> Div {
    card(cx)
        .p_5()
        .gap_4()
        .child(card_title(title, cx))
        .child(content)
}

/// The width a card's content has on the page, in rems: the page's less the
/// card's padding.
fn card_room(window: &Window) -> f32 {
    crate::shell::content_width(window) / window.rem_size() - 2.5
}

/// A grid of `count` tiles at least `min_rems` wide in `room` rems, in rows
/// as even as can be (a short set, such as the elements).
fn tile_grid(count: usize, min_rems: f32, room: f32) -> Div {
    list_grid(min_rems, room).grid_cols(wishes::even_columns(count, fit(min_rems, room)) as u16)
}

/// A grid of entries at least `min_rems` wide in `room` rems, as many a row
/// as fit (a list of any length, whose last row may be short).
fn list_grid(min_rems: f32, room: f32) -> Div {
    div().grid().grid_cols(fit(min_rems, room) as u16).gap_3()
}

/// How many items at least `min_rems` wide fit a row of `room` rems.
fn fit(min_rems: f32, room: f32) -> usize {
    // gap_3 between them: 0.75 rem.
    ((room + 0.75) / (min_rems + 0.75)).floor().max(1.) as usize
}
