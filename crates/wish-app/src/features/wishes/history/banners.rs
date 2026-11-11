//! The banner view: the 5★ history, then one card per banner.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    AnyElement, App, FontWeight, InteractiveElement as _, IntoElement, ObjectFit,
    ParentElement as _, Pixels, SharedString, Styled as _, StyledImage as _, Window, div, img,
    prelude::FluentBuilder as _, relative,
};
use wish_core::data::Banner;
use wish_core::engine::{PoolAnalysis, Pull};
use wish_core::model::Rarity;

use crate::app::images::Images;
use wish_ui::outcome_badge;
use wish_ui::{card, card_title, item_tile, stat_label};
use wish_ui::{format, theme};

/// Every 5★ of the pool, newest first, with its pity as a bar out of the
/// hard pity: how lucky each one was, labelled, at a glance.
pub(super) fn five_star_history(
    pool: &PoolAnalysis,
    images: &Images,
    cx: &App,
) -> Option<AnyElement> {
    let fives: Vec<&Pull> = pool
        .pulls
        .iter()
        .rev()
        .filter(|p| p.wish.rarity == Rarity::Five)
        .collect();
    if fives.is_empty() {
        return None;
    }
    let theme = cx.theme();
    let gold = theme::rarity(Rarity::Five, cx);
    let cap = pool.pool.hard_pity_five();
    let rows = fives.into_iter().map(|pull| {
        let pity = pull.pity.unwrap_or(0);
        let fraction = (pity as f32 / cap as f32).clamp(0.0, 1.0);
        div()
            .id(SharedString::from(format!("five-{}", pull.wish.id)))
            .h_flex()
            .gap_3()
            .py_1()
            .child(item_tile(pull.wish.rarity, images.item(&pull.wish), cx))
            .child(
                div()
                    .w_48()
                    .flex_shrink_0()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(gold)
                    .child(pull.wish.name.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .h_2()
                    .rounded_full()
                    .bg(theme.muted)
                    .child(div().h_full().w(relative(fraction)).rounded_full().bg(gold)),
            )
            .child(
                div()
                    .w_8()
                    .flex_shrink_0()
                    .text_right()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(pity.to_string()),
            )
            .child(
                div()
                    .w_24()
                    .flex_shrink_0()
                    .h_flex()
                    .children(pull.rate_up.map(|r| outcome_badge(r.outcome(), cx))),
            )
            .child(
                div()
                    .w_24()
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(pull.wish.time.naive().format("%-d %b %Y").to_string()),
            )
    });
    Some(
        card(cx)
            .gap_2()
            .p_4()
            .child(card_title("5★ history", cx).child(stat_label(format!("pity out of {cap}"), cx)))
            .children(rows)
            .into_any_element(),
    )
}

/// A banner card is never narrower: room for its four numbers side by side.
pub(super) const BANNER_CARD_REMS: f32 = 22.;
/// At most this many banner cards a row.
pub(super) const BANNER_COLUMNS: u16 = 4;

/// One banner, as a card of the grid: its art, name and dates, its numbers,
/// then each 5★ pulled on it, one a line.
pub(super) fn banner_card(
    banner: &Banner,
    pulls: &[Pull],
    images: &Images,
    card_width: Pixels,
    window: &Window,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    // Twice as wide as high, across the card inside its padding. Set, not
    // left to the ratio: in a column the picture's own height would win.
    let art_width = card_width - window.rem_size() * 1.5;
    let art_height = art_width / 2.;
    let total = pulls.len() as u32;
    let count = |rarity| pulls.iter().filter(|p| p.wish.rarity == rarity).count() as u32;
    let dates = format!(
        "{} – {}",
        banner.start.naive().format("%-d %b %Y"),
        banner.end.naive().format("%-d %b %Y")
    );
    // The art across the card, inset: its own corners, since clipping is
    // rectangular.
    let art = match images.banner(banner) {
        Some(path) => img(path)
            .w(art_width)
            .h(art_height)
            // `img` would otherwise keep the picture's own ratio.
            .aspect_ratio(2.)
            .object_fit(ObjectFit::Cover)
            .rounded(theme.radius)
            .into_any_element(),
        None => div()
            .w(art_width)
            .h(art_height)
            .rounded(theme.radius)
            .bg(theme.muted)
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(theme.muted_foreground)
            .child(banner.short_name.clone())
            .into_any_element(),
    };
    let stat = |label: &'static str, value: String| {
        div()
            .v_flex()
            .gap_0p5()
            .min_w_0()
            .child(stat_label(label, cx))
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .truncate()
                    .child(value),
            )
    };
    let gold = theme::rarity(Rarity::Five, cx);
    let fives: Vec<_> = pulls
        .iter()
        .filter(|p| p.wish.rarity == Rarity::Five)
        .map(|pull| {
            div()
                .h_flex()
                .gap_2()
                .child(item_tile(pull.wish.rarity, images.item(&pull.wish), cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(gold)
                        .child(pull.wish.name.clone()),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(pull.pity.unwrap_or(0).to_string()),
                )
                .children(pull.rate_up.map(|r| outcome_badge(r.outcome(), cx)))
        })
        .collect();
    let none = fives.is_empty();

    card(cx)
        .id(SharedString::from(format!("banner-{}", banner.id())))
        .gap_3()
        .p_3()
        .child(art)
        .child(
            div()
                .v_flex()
                .gap_3()
                .px_1()
                .pb_1()
                .child(
                    div()
                        .v_flex()
                        .gap_0p5()
                        .child(
                            div()
                                .h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .truncate()
                                        .child(banner.name.clone()),
                                )
                                .when_some(banner.version.clone(), |this, version| {
                                    this.child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.muted_foreground)
                                            .child(version),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child(dates),
                        ),
                )
                .child(
                    div()
                        .grid()
                        .grid_cols(4)
                        .gap_2()
                        .child(stat("Wishes", format::count(total)))
                        .child(stat("Primogems", format::primogems(total)))
                        .child(stat("5★", count(Rarity::Five).to_string()))
                        .child(stat("4★", count(Rarity::Four).to_string())),
                )
                .child(div().v_flex().gap_1p5().children(fives).when(none, |this| {
                    this.child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("No 5★ on this banner."),
                    )
                })),
        )
        .into_any_element()
}
