//! A material on its rarity tile with a count: the planner's costs and the
//! inventory.

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{ActiveTheme as _, Icon};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, FontWeight, ImageSource, IntoElement, ParentElement as _, RenderOnce, SharedString,
    Styled as _, Window, div, img, px, rems,
};

use crate::{format, theme};

/// A material and a count, as the game shows costs: the icon on its rarity
/// tile, the count under it. 56 px wide. Without an icon (a material the game
/// data does not have), a question mark; without a count, the tile alone.
/// How many are crafted from the tiers below shows in a badge over the
/// picture, an alchemy flask and the count, as genshin-center does.
#[derive(IntoElement)]
#[must_use = "a component draws only once it is a child of an element"]
pub struct MaterialTile {
    icon: Option<ImageSource>,
    stars: u8,
    label: Option<SharedString>,
    done: bool,
    crafted: u64,
}

impl MaterialTile {
    pub fn new(icon: Option<ImageSource>, stars: u8, label: impl Into<SharedString>) -> Self {
        Self {
            icon,
            stars,
            label: Some(label.into()),
            done: false,
            crafted: 0,
        }
    }

    /// The icon on its rarity tile, without a count.
    pub fn bare(icon: Option<ImageSource>, stars: u8) -> Self {
        Self {
            icon,
            stars,
            label: None,
            done: false,
            crafted: 0,
        }
    }

    /// Nothing left to get: the tile greyed out (its crafted badge stays
    /// clear).
    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }

    /// How many are crafted from the tiers below; none shows no badge.
    pub fn crafted(mut self, count: u64) -> Self {
        self.crafted = count;
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
        // The picture on its rarity tile, over the raised strip with the
        // count.
        let (strip_text, strip) = theme::tile_label(cx);
        let tile = div()
            .v_flex()
            .w(rems(3.5))
            .rounded(colors.radius)
            .overflow_hidden()
            .when(self.label.is_some(), |this| this.bg(strip))
            .child(
                div()
                    .size(rems(3.5))
                    // Clipping is rectangular: the tile rounds its own corners.
                    .rounded_t(colors.radius)
                    .when(self.label.is_none(), |this| this.rounded_b(colors.radius))
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
                        .text_color(strip_text)
                        .child(label),
                )
            })
            .when(self.done, |this| this.opacity(0.35));
        // The crafted count, coloured like the strip, over the picture's top
        // left corner.
        let badge = (self.crafted > 0).then(|| {
            div()
                .absolute()
                .top(px(2.))
                .left(px(2.))
                .h_flex()
                .gap(px(2.))
                .px(px(4.))
                .rounded(colors.radius)
                .bg(strip)
                .text_color(strip_text)
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .shadow_sm()
                .child(Icon::new(IconName::FlaskConical).size(px(11.)))
                .child(format::compact(self.crafted))
        });
        div().relative().flex_shrink_0().child(tile).children(badge)
    }
}
