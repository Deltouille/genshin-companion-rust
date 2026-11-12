use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, FontWeight, ImageSource, IntoElement, ParentElement as _, RenderOnce, SharedString,
    Styled as _, Window, div, img, rems,
};

use crate::theme;

/// A material and a count, as the game shows costs: the icon on its rarity
/// tile, the count under it. 56 px wide. Without an icon (a material the game
/// data does not have), a question mark; without a count, the tile alone.
#[derive(IntoElement)]
pub struct MaterialTile {
    icon: Option<ImageSource>,
    stars: u8,
    label: Option<SharedString>,
    done: bool,
}

impl MaterialTile {
    pub fn new(icon: Option<ImageSource>, stars: u8, label: impl Into<SharedString>) -> Self {
        Self {
            icon,
            stars,
            label: Some(label.into()),
            done: false,
        }
    }

    /// The icon on its rarity tile, without a count.
    pub fn bare(icon: Option<ImageSource>, stars: u8) -> Self {
        Self {
            icon,
            stars,
            label: None,
            done: false,
        }
    }

    /// Nothing left to get: the tile greyed out.
    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }
}

impl RenderOnce for MaterialTile {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme();
        let picture = match self.icon {
            Some(icon) => img(icon).size_full().into_any_element(),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .text_color(colors.foreground.opacity(0.6))
                .child("?")
                .into_any_element(),
        };
        // The game's item card: the picture on its rarity tile, whose bottom
        // right corner rounds wide into the parchment strip under it, where
        // the count is written in ink.
        let (ink, parchment) = theme::tile_label(cx);
        div()
            .relative()
            .v_flex()
            .w(rems(3.5))
            .flex_shrink_0()
            .rounded(colors.radius)
            .overflow_hidden()
            .when(self.label.is_some(), |this| this.bg(parchment))
            .child(
                div()
                    .size(rems(3.5))
                    // Clipping is rectangular: the tile rounds its own corners.
                    .rounded_t(colors.radius)
                    .rounded_bl(match self.label {
                        Some(_) => rems(0.),
                        None => rems(0.5),
                    })
                    .rounded_br(rems(1.))
                    .bg(theme::stars_tile(self.stars, cx))
                    .child(picture),
            )
            .when_some(self.label, |this, label| {
                this.child(
                    div()
                        .w_full()
                        .py_0p5()
                        .text_center()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(ink)
                        .child(label),
                )
            })
            .when(self.done, |this| this.opacity(0.35))
    }
}
