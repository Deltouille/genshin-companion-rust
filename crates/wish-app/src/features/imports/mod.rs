//! Import: every way to bring data in, on one page. The wish history (from
//! the game, from a file) and the characters with their weapons (from
//! HoYoLAB; with the materials, from a GOOD file), each source a card saying
//! what it brings, then its buttons. The buttons send the app's actions
//! (`ImportFromGame`, `ImportFile`, `SyncFromHoyolab`, `ImportGood`), as they
//! do on the other pages that offer them.

pub(crate) mod good;
pub(crate) mod hoyolab;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::{
    App, Div, IntoElement, ParentElement as _, RenderOnce, Styled as _, Window, div, rems,
};
use wish_ui::{PageFrame, button, card, card_title};

use crate::features::wishes::import::{import_file_button, import_game_button};
use crate::shell::{ImportGood, SyncFromHoyolab};

/// "Sync from HoYoLAB…": sends the [`SyncFromHoyolab`] action.
pub fn hoyolab_sync_button() -> Button {
    button("sync-hoyolab")
        .label(t!("imports-sync-button"))
        .icon(gpui_kit::assets::IconName::CloudDownload)
        .on_click(|_, window, cx| window.dispatch_action(Box::new(SyncFromHoyolab), cx))
}

/// "Import a GOOD file…": sends the [`ImportGood`] action.
pub fn good_import_button() -> Button {
    button("import-good")
        .label(t!("imports-good-button"))
        .icon(gpui_kit::assets::IconName::FolderOpen)
        .on_click(|_, window, cx| window.dispatch_action(Box::new(ImportGood), cx))
}

/// The Import page. It has no state: whether a write is running (to show
/// the file import busy) is all it is given.
#[derive(IntoElement)]
pub struct Imports {
    busy: bool,
}

impl Imports {
    pub fn new(busy: bool) -> Self {
        Self { busy }
    }
}

impl RenderOnce for Imports {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let text = |text: String| div().text_sm().child(text);
        let source = |title: String, body: Div, buttons: Div, cx: &App| {
            card(cx)
                .p_5()
                .gap_3()
                .child(card_title(title, cx))
                .child(body)
                .child(buttons.h_flex().flex_wrap().gap_2())
        };
        let body = div()
            .v_flex()
            .gap_4()
            .child(source(
                t!("imports-wish-history"),
                div()
                    .v_flex()
                    .gap_2()
                    .child(text(t!("imports-wish-history-game")))
                    .child(text(t!("imports-wish-history-file"))),
                div()
                    .child(import_game_button().primary())
                    .child(import_file_button(self.busy)),
                cx,
            ))
            .child(source(
                t!("imports-collection"),
                div()
                    .v_flex()
                    .gap_2()
                    .child(text(t!("imports-collection-help")))
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child(t!("imports-collection-safe")),
                    ),
                div().child(hoyolab_sync_button()),
                cx,
            ))
            .child(source(
                t!("imports-good"),
                div()
                    .v_flex()
                    .gap_2()
                    .child(text(t!("imports-good-help")))
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child(t!("imports-good-safe")),
                    ),
                div().child(good_import_button()),
                cx,
            ))
            .child(div().text_sm().text_color(muted).child(t!("imports-undo")));
        PageFrame::new(t!("imports-title"), body).max_width(rems(48.))
    }
}
