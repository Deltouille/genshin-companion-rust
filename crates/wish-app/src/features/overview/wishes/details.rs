//! One banner type in detail, after paimon.moe's wish summary
//! (`src/routes/wish/_summaryItem.svelte`): the 5★ and 4★ with their share
//! of the wishes and average pity, the 50/50 record, the 4★ split into
//! characters and weapons, then every 5★ with its pity. The lower half of
//! the banner type's card (`pool_card.rs`).

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Div, ElementId, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, div, hsla,
};
use wish_core::data::GameData;
use wish_core::engine::{PoolAnalysis, Pull, RateUpOutcome};
use wish_core::model::Rarity;
use wish_core::stats::{RarityStats, RateStats};

use wish_ui::{format, theme};

/// The table of the 5★ and 4★: count, share of the wishes, average pity.
pub(super) fn stats_table(pool: &PoolAnalysis, cx: &App) -> Div {
    let colors = cx.theme();
    let (gold, purple) = (
        theme::rarity(Rarity::Five, cx),
        theme::rarity(Rarity::Four, cx),
    );
    let stats = &pool.stats;
    let total = stats.total;
    let odds = format::rate_up_odds(pool.pool);

    // Shares are of all the pool's wishes, as on paimon.moe.
    let row = |label: &str, nested: bool, stats: RarityStats, color: Hsla| {
        table_row(
            label,
            nested,
            [
                format::count(stats.count),
                format::share(stats.share(total)),
                stats
                    .average_pity()
                    .map_or_else(|| "–".to_owned(), format::number),
            ],
            color,
            cx,
        )
    };
    // The 50/50 record: wins, and the share of the 50/50s played that were
    // won (a guaranteed item is not a 50/50).
    let won = |rate: Option<RateStats>, count: u32, color: Hsla| {
        rate.filter(|_| count > 0).map(|rate| {
            table_row(
                &format!("Won {odds}"),
                true,
                [
                    format::count(rate.won),
                    format::share(rate.win_rate().unwrap_or(0.0)),
                    String::new(),
                ],
                color,
                cx,
            )
        })
    };

    let head = |text: &'static str| numeric(text).text_xs().text_color(colors.muted_foreground);
    let header = div()
        .h_flex()
        .pb_1()
        .child(div().flex_1())
        .child(head("Total").w_12())
        .child(head("Share").w_16())
        .child(head("Avg. pity").w_16());
    let rows = [
        Some(row("5★", false, stats.five, gold)),
        won(stats.rate_five, stats.five.count, gold),
        Some(row("4★", false, stats.four, purple)),
        Some(row("Character", true, stats.four_character, purple)),
        Some(row("Weapon", true, stats.four_weapon, purple)),
        won(stats.rate_four, stats.four.count, purple),
    ];

    div()
        .v_flex()
        .child(header)
        .children(rows.into_iter().flatten())
}

/// Every 5★ of the banner type, latest first, each with its pity.
pub(super) fn five_star_pills(pool: &PoolAnalysis, data: Option<&GameData>, cx: &App) -> Div {
    let fives: Vec<&Pull> = pool
        .pulls
        .iter()
        .rev()
        .filter(|pull| pull.wish.rarity == Rarity::Five)
        .collect();
    let caption = if fives.is_empty() {
        "No 5★ yet."
    } else {
        "Every 5★ and its pity, latest first"
    };
    let pills = fives
        .iter()
        .enumerate()
        .map(|(ix, pull)| pity_pill(pool, pull, ix, data, cx));
    div()
        .v_flex()
        .gap_2()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(caption),
        )
        .when(!fives.is_empty(), |this| {
            this.child(div().h_flex().flex_wrap().gap_1p5().children(pills))
        })
}

/// A row of the table: its label (indented under its rarity when `nested`),
/// then the count, the share and the average pity, in the row's colour.
///
/// A plain grid rather than GPUI Kit's `Table`, whose minimum cell widths
/// overflow a card a third of the page wide.
fn table_row(
    label: &str,
    nested: bool,
    [count, share, pity]: [String; 3],
    color: Hsla,
    cx: &App,
) -> AnyElement {
    div()
        .h_flex()
        .py_1()
        .text_sm()
        .border_t_1()
        .border_color(cx.theme().border)
        .text_color(color)
        .font_weight(FontWeight::SEMIBOLD)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .when(nested, |this| this.pl_4())
                .child(label.to_owned()),
        )
        .child(numeric(count).w_12())
        .child(numeric(share).w_16())
        .child(numeric(pity).w_16())
        .into_any_element()
}

/// A right-aligned cell of the table.
fn numeric(text: impl Into<SharedString>) -> Div {
    div().flex_none().text_right().child(text.into())
}

/// A 5★ and its pity, paimon.moe's way: the pity goes from green (early) to
/// red (at hard pity), and a won 50/50 (or a banner without one) is ringed
/// in gold, a lost or guaranteed one in grey. The tooltip says when, on
/// which banner, and how the 50/50 went, so the colours are not the only
/// carrier.
fn pity_pill(
    pool: &PoolAnalysis,
    pull: &Pull,
    ix: usize,
    data: Option<&GameData>,
    cx: &App,
) -> impl IntoElement {
    let colors = cx.theme();
    let pity = pull.pity.unwrap_or(0);
    let hard = pool.pool.hard_pity_five();
    let outcome = pull.rate_up.map(|rate_up| rate_up.outcome());
    let ring = match outcome {
        None | Some(RateUpOutcome::Won) => theme::rarity(Rarity::Five, cx),
        Some(_) => colors.border,
    };

    let banner = data
        .zip(pull.banner)
        .and_then(|(data, ix)| data.banners.for_pool(pool.pool).get(ix))
        .map(|banner| banner.name.clone());
    let mut about = vec![pull.wish.time.naive().format("%-d %b %Y").to_string()];
    about.extend(banner);
    about.extend(outcome.map(|outcome| match outcome {
        RateUpOutcome::Won => format!("won the {}", format::rate_up_odds(pool.pool)),
        RateUpOutcome::Lost | RateUpOutcome::LostWhileGuaranteed => {
            format!("lost the {}", format::rate_up_odds(pool.pool))
        }
        RateUpOutcome::Guaranteed => "guaranteed".to_owned(),
    }));
    let about = SharedString::from(about.join(" · "));

    div()
        .id(ElementId::NamedInteger(
            SharedString::new_static(pool.pool.display_name()),
            ix as u64,
        ))
        .h_flex()
        .gap_1p5()
        .px_2p5()
        .py_0p5()
        .rounded_full()
        .border_1()
        .border_color(ring)
        .text_sm()
        .child(
            div()
                .text_color(colors.muted_foreground)
                .child(pull.wish.name.clone()),
        )
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pity_color(pity, hard, colors.is_dark()))
                .child(pity.to_string()),
        )
        .tooltip(move |window, cx| Tooltip::new(about.clone()).build(window, cx))
}

/// paimon.moe's pity hue: 120° (green) at pity 0 down to 0° (red) at hard
/// pity. Light mode darkens it to stay readable on white.
fn pity_color(pity: u32, hard: u32, dark: bool) -> Hsla {
    let early = 1.0 - (pity as f32 / hard.max(1) as f32).clamp(0.0, 1.0);
    let hue = early * 120.0 / 360.0;
    if dark {
        hsla(hue, 1.0, 0.6, 1.0)
    } else {
        hsla(hue, 0.9, 0.34, 1.0)
    }
}
