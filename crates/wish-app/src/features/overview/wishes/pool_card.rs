//! One banner type's card: where pity stands, as paimon.moe's wish counter
//! draws it, then the banner type in detail (`details.rs`), so everything
//! about one banner type is in one place.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;

use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _};
use gpui_kit::{
    App, ClickEvent, Div, FontWeight, ImageSource, ParentElement as _, SharedString, Styled as _,
    Window, div, img,
};
use wish_core::data::GameData;
use wish_core::engine::{Counters, PoolAnalysis};
use wish_core::model::Rarity;

use super::details::{five_star_pills, stats_table};
use wish_ui::pool_icon;
use wish_ui::{badge, big_number, card, icon_button, stat_tile};
use wish_ui::{format, theme};

/// The narrowest a card may be, in rem: the table's three columns and its
/// longest label ("Won 50/50") side by side.
pub(super) const POOL_CARD_REMS: f32 = 19.;

/// One banner type: its name and latest banner, 5★ and 4★ pity in large
/// numbers with the odds of the next one, then its wishes, the 5★ and 4★
/// with their shares and average pity, and every 5★ with its pity.
pub(super) fn pool_card(
    pool: &PoolAnalysis,
    data: Option<&GameData>,
    primogem: Option<ImageSource>,
    on_history: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let Counters {
        total,
        pity5,
        pity4,
    } = pool.counters;
    let gold = theme::rarity(Rarity::Five, cx);
    let purple = theme::rarity(Rarity::Four, cx);

    // The latest banner of the type, with its version when known.
    let latest = data.and_then(|data| {
        let range = pool.banners.last()?;
        let banner = data.banners.for_pool(pool.pool).get(range.banner)?;
        Some(match &banner.version {
            Some(version) => format!("{} · {version}", banner.name),
            None => banner.name.clone(),
        })
    });

    // The odds of the next item of a rarity, as a badge on its tile.
    let odds = |guaranteed: bool, color| {
        let text = format::next_rate_up(pool.pool, guaranteed);
        if guaranteed {
            badge("Guaranteed", theme::soft(color), cx)
        } else {
            badge(text, (theme.muted_foreground, theme.group_box), cx)
        }
    };
    // The odds of the next item sit by its label, leaving the right side
    // to the number.
    let pity_label =
        |label: &'static str, odds: Option<Div>| div().h_flex().gap_2().child(label).children(odds);

    let header = div()
        .h_flex()
        .gap_3()
        .child(
            Icon::new(pool_icon(pool.pool))
                .size_5()
                .text_color(theme.primary),
        )
        .child(
            div()
                .v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .child(pool.pool.display_name()),
                )
                .children(latest.map(|banner| {
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .truncate()
                        .child(banner)
                })),
        )
        .child(
            icon_button(
                SharedString::from(format!("history-{}", pool.pool)),
                IconName::ChevronRight,
                format!("{} history", pool.pool.display_name()),
            )
            .ghost()
            .on_click(on_history),
        );

    let pity = div()
        .v_flex()
        .gap_2()
        .child(stat_tile(
            pity_label("5★ pity", pool.guarantee.map(|g| odds(g.five, gold))),
            format!("Guaranteed at {}", pool.pool.hard_pity_five()),
            big_number(pity5.to_string(), gold),
            cx,
        ))
        .child(stat_tile(
            pity_label("4★ pity", pool.guarantee.map(|g| odds(g.four, purple))),
            format!("Guaranteed at {}", pool.pool.hard_pity_four()),
            big_number(pity4.to_string(), purple),
            cx,
        ))
        .children(notes(pool).into_iter().map(|note| {
            // The icon carries the warning colour; the text keeps full
            // contrast and says what is wrong.
            div()
                .h_flex()
                .gap_2()
                .pt_1()
                .text_sm()
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .small()
                        .text_color(theme.warning),
                )
                .child(div().flex_1().min_w_0().child(note))
        }));

    // The wishes the table's shares are of, and what they cost.
    let wishes = div()
        .h_flex()
        .gap_3()
        .child(
            div()
                .flex_1()
                .font_weight(FontWeight::SEMIBOLD)
                .child(format!(
                    "{} {}",
                    format::count(total),
                    format::plural(total, "wish", "wishes")
                )),
        )
        .child(
            div()
                .h_flex()
                .gap_1p5()
                .text_sm()
                .text_color(theme.muted_foreground)
                .children(primogem.map(|primogem| img(primogem).size_4()))
                .child(format!("{} primogems", format::primogems(total))),
        );

    card(cx)
        .p_4()
        .gap_4()
        .child(header)
        .child(pity)
        .child(
            div()
                .v_flex()
                .gap_2()
                .child(wishes)
                .child(stats_table(pool, cx)),
        )
        .child(five_star_pills(pool, data, cx))
}

/// What may make the numbers wrong, said under the pity.
fn notes(pool: &PoolAnalysis) -> Vec<String> {
    let mut notes = Vec::new();
    if pool.outside_banners > 0 {
        notes.push(format!(
            "{} {} outside every known banner. Update the game data.",
            pool.outside_banners,
            format::plural(pool.outside_banners, "wish falls", "wishes fall"),
        ));
    }
    if pool.unknown_items > 0 {
        notes.push(format!(
            "{} {} an item this version doesn’t know.",
            pool.unknown_items,
            format::plural(pool.unknown_items, "wish has", "wishes have"),
        ));
    }
    if pool.manual_counters {
        notes.push("Counters typed by hand in paimon.moe.".to_owned());
    }
    notes
}
