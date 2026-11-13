//! Import: every way to bring data in, on one page. The wish history (from
//! the game, from a file) and the characters with their weapons (from
//! HoYoLAB), each source a card saying what it brings, then its buttons.
//! The buttons send the app's actions (`ImportFromGame`, `ImportFile`,
//! `SyncFromHoyolab`), as they do on the other pages that offer them.

pub(crate) mod hoyolab;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::{
    App, Div, IntoElement, ParentElement as _, RenderOnce, Styled as _, Window, div, rems,
};
use wish_ui::{PageFrame, button, card, card_title};

use crate::features::wishes::import::{import_file_button, import_game_button};
use crate::shell::SyncFromHoyolab;

/// "Sync from HoYoLAB…": sends the [`SyncFromHoyolab`] action.
pub fn hoyolab_sync_button() -> Button {
    button("sync-hoyolab")
        .label("Sync from HoYoLAB…")
        .icon(gpui_kit::assets::IconName::CloudDownload)
        .on_click(|_, window, cx| window.dispatch_action(Box::new(SyncFromHoyolab), cx))
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
        let text = |text: &'static str| div().text_sm().child(text);
        let source = |title: &'static str, body: Div, buttons: Div, cx: &App| {
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
            .max_w(rems(48.))
            .child(source(
                "Wish history",
                div()
                    .v_flex()
                    .gap_2()
                    .child(text(
                        "From the game: the app finds the wish history link in the game’s cache \
                         (Windows, or a Wine or Proton prefix on Linux), or you paste it from \
                         any device, then asks HoYoverse’s server for your wishes.",
                    ))
                    .child(text(
                        "From a file: a paimon.moe save or spreadsheet, a UIGF v4 file from \
                         another tracker, or this app’s snapshot.",
                    )),
                div()
                    .child(import_game_button().primary())
                    .child(import_file_button(self.busy)),
                cx,
            ))
            .child(source(
                "Characters and weapons",
                div()
                    .v_flex()
                    .gap_2()
                    .child(text(
                        "From HoYoLAB: your characters as they are in the game (level, \
                         constellations, talents) and the weapon each holds, into your \
                         collection. Your HoYoLAB cookie is used for that sync only, and never \
                         saved.",
                    ))
                    .child(div().text_sm().text_color(muted).child(
                        "It only adds or raises what you entered, never lowers it, and shows \
                         what would change before writing anything.",
                    )),
                div().child(hoyolab_sync_button()),
                cx,
            ))
            .child(div().text_sm().text_color(muted).child(
                "Every import can be undone, and a backup is taken before each: see Settings.",
            ));
        PageFrame::new("Import", body)
    }
}
