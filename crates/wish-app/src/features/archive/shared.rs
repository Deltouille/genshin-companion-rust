//! What the character and weapon pages share: the hero banner, the facts,
//! a level badge, a table of stats, a list of materials, a card's corner
//! buttons, a list's section titles, and versions as numbers.

use std::collections::BTreeMap;

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::Button;
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Div, FontWeight, ImageSource, InteractiveElement as _, IntoElement, ObjectFit,
    ParentElement as _, Pixels, SharedString, Styled as _, StyledImage as _, Window, div, img,
    linear_color_stop, linear_gradient, rems,
};
use wish_core::planner::{MaterialId, PlannerData};

use crate::app::images::Images;
use gpui_kit::StatefulInteractiveElement as _;
use wish_ui::ButtonExt as _;
use wish_ui::{MaterialTile, badge, format};

/// The hero's height, in rems.
const HERO_REMS: f32 = 22.;

/// A page's banner: `info` on the left; on the right `art` (with its width
/// as a share of its height, and how many times the banner's height it is
/// drawn, cropped by the banner), centred three quarters of the way across;
/// under both `backdrop`, a blurred copy of the art faded into the card under
/// the text. `width` is the page's.
pub(super) fn hero_banner(
    info: Div,
    art: Option<(ImageSource, f32, f32)>,
    backdrop: Option<ImageSource>,
    width: Pixels,
    window: &Window,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let height = window.rem_size() * HERO_REMS;
    // The art beside the text when there is room for both.
    let beside = width > window.rem_size() * 52.;
    let surface = theme.group_box;
    let layer = || div().absolute().top_0().left_0().w(width).h(height);
    // Clipping is rectangular: each layer rounds its own corners.
    let backdrop = backdrop.map(|image| {
        layer()
            .rounded(theme.radius_lg)
            .overflow_hidden()
            .opacity(0.85)
            .child(
                img(image)
                    .w(width)
                    .h(height)
                    .aspect_ratio(width / height)
                    .object_fit(ObjectFit::Cover),
            )
    });
    // Solid under the text, clear under the art.
    let fade = layer().rounded(theme.radius_lg).bg(linear_gradient(
        90.,
        linear_color_stop(surface, 0.2),
        linear_color_stop(surface.opacity(0.), if beside { 0.75 } else { 1.4 }),
    ));
    let art = art.filter(|_| beside).map(|(art, ratio, scale)| {
        let art_height = (height * scale).min(width * 0.9 / ratio);
        let art_width = art_height * ratio;
        div()
            .absolute()
            .left(width * 0.76 - art_width / 2.)
            .top((height - art_height) / 2.)
            .child(
                img(art)
                    .w(art_width)
                    .h(art_height)
                    .aspect_ratio(ratio)
                    .object_fit(ObjectFit::Contain),
            )
    });
    div()
        .relative()
        .w(width)
        .h(height)
        .rounded(theme.radius_lg)
        .overflow_hidden()
        .bg(surface)
        .when(!theme.is_dark(), |this| {
            this.border_1().border_color(theme.border)
        })
        .children(backdrop)
        .child(fade)
        .children(art)
        .child(
            info.relative()
                .v_flex()
                .justify_center()
                .gap_4()
                .h(height)
                .w(if beside { width * 0.55 } else { width })
                .p_6(),
        )
}

/// Facts in two columns of label and value; those without a value left out.
pub(super) fn facts_grid(facts: &[(String, Option<String>)], cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .grid()
        .grid_cols(2)
        .gap_x_6()
        .gap_y_1()
        .text_sm()
        .children(facts.iter().filter_map(|(label, value)| {
            let value = value.as_ref()?;
            Some(
                div()
                    .h_flex()
                    .gap_2()
                    .child(
                        div()
                            .w(rems(6.5))
                            .flex_shrink_0()
                            .text_color(theme.muted_foreground)
                            .child(label.clone()),
                    )
                    .child(div().truncate().child(value.clone())),
            )
        }))
}

/// A small primary badge for the level a slider is at.
pub(super) fn level_badge(text: &str, cx: &App) -> Div {
    let theme = cx.theme();
    badge(
        text.to_owned(),
        (theme.primary, theme.primary.opacity(0.16)),
        cx,
    )
    .text_sm()
}

/// Two columns: a muted label, the value on the right.
pub(super) fn table<L: Into<SharedString>>(
    rows: impl IntoIterator<Item = (L, String)>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    div()
        .v_flex()
        .children(rows.into_iter().map(|(label, value)| {
            div()
                .h_flex()
                .justify_between()
                .gap_4()
                .py_1p5()
                .border_t_1()
                .border_color(theme.border)
                .child(div().text_color(theme.muted_foreground).child(label.into()))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(value))
        }))
}

/// The materials of a cost as the game shows them, in its inventory order.
pub(super) fn materials(
    need: &BTreeMap<MaterialId, u64>,
    planner: &PlannerData,
    images: &Images,
    cx: &App,
) -> AnyElement {
    let mut items: Vec<_> = need
        .iter()
        .filter_map(|(&id, &count)| Some((planner.material(id)?, count)))
        .collect();
    if items.is_empty() {
        return div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(t!("archive-nothing-to-spend"))
            .into_any_element();
    }
    items.sort_by_key(|(material, _)| (material.sort, material.id));
    div()
        .h_flex()
        .flex_wrap()
        .gap_2()
        .children(items.into_iter().map(|(material, count)| {
            div()
                .id(SharedString::from(format!("material-{}", material.id)))
                .child(MaterialTile::new(
                    images.material(material),
                    material.rarity,
                    format::compact(count),
                ))
                .tooltip({
                    let text = SharedString::from(t!(
                        "archive-material",
                        count = count,
                        name = material.name.as_str()
                    ));
                    move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(text.clone()).build(window, cx)
                    }
                })
        }))
        .into_any_element()
}

/// A list card's width, and its picture's height, in rems: the same on every
/// archive page (the user's ask on 2026-11-17).
pub(super) const CARD_REMS: f32 = 8.25;

/// What a list card shows: the picture on its rarity tile, with an icon top
/// left, buttons top right, what the account has bottom left and "New"
/// bottom right; the name under it, on a parchment strip.
pub(super) struct ItemCard {
    /// The button's id.
    pub id: SharedString,
    pub icon: Option<ImageSource>,
    pub rarity: u8,
    /// Not owned: the tile and picture faded, the strip slate, the name
    /// muted.
    pub owned: bool,
    pub name: String,
    /// What a screen reader says of it.
    pub label: String,
    pub top_left: Option<AnyElement>,
    pub corner: Option<AnyElement>,
    /// What the account has of it: "Lv. 90 · C2", "R3 · ×2", a count.
    pub summary: Option<String>,
    pub new: bool,
}

/// A list card, the same on every archive page: the caller adds what a
/// click does.
pub(super) fn item_card(card: ItemCard, cx: &App) -> Button {
    let theme = cx.theme();
    let (ink, parchment) = wish_ui::theme::tile_label(cx);
    let owned = card.owned;
    let tile = wish_ui::theme::stars_tile(card.rarity, cx);
    let picture = div()
        .relative()
        .w_full()
        .h(rems(CARD_REMS))
        .rounded_t(theme.radius_lg)
        .rounded_br(rems(2.))
        // Faded with its picture: the card reads as one not owned.
        .bg(if owned { tile } else { tile.opacity(0.4) })
        .children(card.icon.map(|icon| {
            img(icon)
                .size_full()
                .object_fit(ObjectFit::Contain)
                .when(!owned, |this| this.opacity(0.5))
        }))
        .children(
            (card.top_left).map(|icon| div().absolute().top_1().left_1().size_6().child(icon)),
        )
        .children((card.corner).map(|corner| div().absolute().top_1().right_1().child(corner)))
        .child(
            div()
                .absolute()
                .bottom_1()
                .left_1()
                .right_1()
                .h_flex()
                .gap_1()
                .children(card.summary.map(|text| {
                    badge(text, (theme.foreground, theme.background.opacity(0.85)), cx)
                }))
                .child(div().flex_1())
                .when(card.new, |this| {
                    this.child(badge(
                        t!("archive-new"),
                        (theme.primary_foreground, theme.primary),
                        cx,
                    ))
                }),
        );
    Button::new(card.id)
        .quiet(cx)
        .p_0()
        .w(rems(CARD_REMS))
        .h_auto()
        .rounded(theme.radius_lg)
        .cursor_pointer()
        .accessibility_label(card.label)
        .child(
            div()
                .v_flex()
                .w_full()
                .rounded(theme.radius_lg)
                .bg(if owned { parchment } else { theme.muted })
                .child(picture)
                .child(
                    div()
                        .w_full()
                        .px_2()
                        .py_1p5()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_center()
                        .truncate()
                        .text_color(if owned { ink } else { theme.muted_foreground })
                        .child(card.name),
                ),
        )
}

/// A small round button in a card's corner, on a veil of the page's colour
/// so it reads over any picture.
pub(super) fn corner_button(
    id: impl Into<SharedString>,
    icon: IconName,
    label: String,
    cx: &App,
) -> Button {
    let theme = cx.theme();
    Button::new(gpui_kit::ElementId::Name(id.into()))
        .quiet(cx)
        .small()
        .size_7()
        .rounded_full()
        .bg(theme.background.opacity(0.8))
        .icon(icon)
        .tooltip(label.clone())
        .accessibility_label(label)
        .cursor_pointer()
}

/// A part of the list: its name and how many it holds, as a chip.
pub(super) fn section_title(title: String, count: usize, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .h_flex()
        .gap_2()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(badge(
            count.to_string(),
            (theme.secondary_foreground, theme.secondary),
            cx,
        ))
}

/// "4.10" → (4, 10), so versions sort as numbers.
pub(super) fn release(version: &str) -> (u32, u32) {
    let mut parts = version.split('.').map(|p| p.parse().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}
