//! The charts above the wish table, after paimon.moe's history page: how the
//! wishes split by rarity, and how many went to each banner.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::chart::{AreaChart, PieChart};
use gpui_kit::{App, FontWeight, IntoElement, ParentElement as _, SharedString, Styled as _, div};
use wish_core::data::{Banner, GameData};
use wish_core::engine::PoolAnalysis;
use wish_core::model::Rarity;
use wish_core::stats::Stats;

use wish_ui::{PityPoint, card, card_title, format, theme};

/// One slice of the rarity donut.
#[derive(Clone, Copy)]
struct Slice {
    rarity: Rarity,
    count: u32,
}

/// The wishes split by rarity, as a donut with the total in its hole and
/// each rarity's count and share beside it (the 5★ slice is often a sliver,
/// so the numbers carry it).
pub(super) fn rarity_donut(pool: &PoolAnalysis, cx: &App) -> impl IntoElement {
    let colors = cx.theme();
    let total = pool.pulls.len() as u32;
    let (five, four) = (pool.stats.five.count, pool.stats.four.count);
    let slices = [
        Slice {
            rarity: Rarity::Five,
            count: five,
        },
        Slice {
            rarity: Rarity::Four,
            count: four,
        },
        Slice {
            rarity: Rarity::Three,
            count: total.saturating_sub(five + four),
        },
    ];
    let share = move |count: u32| {
        if total == 0 {
            0.0
        } else {
            f64::from(count) / f64::from(total)
        }
    };

    // `rarity_chart`'s colours, validated together against both card
    // surfaces; a small gap between the slices keeps them apart.
    let chart = PieChart::new(slices)
        .appear(false)
        .inner_radius_ratio(0.68)
        .pad_angle(0.02)
        .value(|slice: &Slice| slice.count as f32)
        .color({
            let colors = slices.map(|slice| theme::rarity_chart(slice.rarity, cx));
            move |slice: &Slice| colors[rarity_ix(slice.rarity)]
        })
        .tooltip_name(|slice: &Slice| format!("{}★", slice.rarity.stars()).into())
        .tooltip_value(move |slice: &Slice, _, _| {
            format!(
                "{} ({})",
                format::count(slice.count),
                format::share(share(slice.count))
            )
            .into()
        });

    let legend = slices.iter().map(|slice| {
        div()
            .h_flex()
            .gap_2()
            .child(
                div()
                    .size_2p5()
                    .rounded_sm()
                    .bg(theme::rarity_chart(slice.rarity, cx)),
            )
            .child(
                div()
                    .w_8()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::rarity(slice.rarity, cx))
                    .child(format!("{}★", slice.rarity.stars())),
            )
            .child(
                div()
                    .w_12()
                    .text_right()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format::count(slice.count)),
            )
            .child(
                div()
                    .w_16()
                    .text_right()
                    .text_color(colors.muted_foreground)
                    .child(format::share(share(slice.count))),
            )
    });

    card(cx)
        .h_full()
        .p_4()
        .gap_2()
        .child(card_title("By rarity", cx))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .h_flex()
                .gap_4()
                .child(
                    // A square as high as the room under the title, so the ring
                    // (80% of it) never leaves the card, whatever its height.
                    div()
                        .relative()
                        .h_full()
                        .aspect_ratio(1.)
                        .flex_none()
                        .child(chart)
                        .child(
                            div()
                                .absolute()
                                .inset_0()
                                .v_flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::BLACK)
                                        .child(format::count(total)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(colors.muted_foreground)
                                        .child(format::plural(total, "wish", "wishes")),
                                ),
                        ),
                )
                .child(div().v_flex().gap_1p5().text_sm().children(legend)),
        )
}

fn rarity_ix(rarity: Rarity) -> usize {
    match rarity {
        Rarity::Five => 0,
        Rarity::Four => 1,
        Rarity::Three => 2,
    }
}

/// One banner on the chart of wishes per banner.
struct BannerWishes {
    /// Under the line: the version (two banners of a version share it).
    tick: SharedString,
    title: SharedString,
    stats: Stats,
}

/// Wishes per banner, oldest first, as an area like the wishes per month on
/// the overview, with each banner's name, 5★ and 4★ in the tooltip. `None`
/// for a pool with fewer than two banners (the Standard banner never
/// changes), where it would say nothing.
pub(super) fn banner_chart(
    pool: &PoolAnalysis,
    data: &GameData,
    cx: &App,
) -> Option<impl IntoElement> {
    if pool.banners.len() < 2 {
        return None;
    }
    let banners = data.banners.for_pool(pool.pool);
    let bars: Vec<BannerWishes> = pool
        .banners
        .iter()
        .map(|range| {
            let banner = banners.get(range.banner);
            let tick = banner
                .and_then(|b| b.version.clone())
                .unwrap_or_default()
                .into();
            let pulls = &pool.pulls[range.pulls.clone()];
            BannerWishes {
                tick,
                title: banner.map(banner_title).unwrap_or_default().into(),
                stats: Stats::of(pulls, pulls.len() as u32, false),
            }
        })
        .collect();

    let most = bars.iter().map(|bar| bar.stats.total).max().unwrap_or(0);
    let (top, y_ticks) = wish_ui::axis::round_axis(most, 5);
    // Version labels need about 3 rem each; the tooltip names every banner.
    let ticks = bars.len().min(6);
    let color = theme::rarity_chart(Rarity::Three, cx);
    let chart = AreaChart::new(bars)
        .appear(false)
        .linear()
        .x(|bar: &BannerWishes| bar.tick.clone())
        .x_tick_count(ticks)
        .y(|bar: &BannerWishes| f64::from(bar.stats.total))
        .stroke(color)
        .fill(color.opacity(0.3))
        .name("Wishes")
        .y_axis(true)
        .y_domain(0.0, f64::from(top))
        .y_padding(0.0, 0.0)
        .y_tick_count(y_ticks)
        .y_tick_format(|v| format!("{v:.0}"))
        .tooltip_title(|bar: &BannerWishes| bar.title.clone())
        .tooltip_value(|bar: &BannerWishes, _, _| {
            format!(
                "{} · {} × 5★ · {} × 4★",
                format::count(bar.stats.total),
                bar.stats.five.count,
                bar.stats.four.count
            )
            .into()
        });

    Some(
        card(cx)
            .h_full()
            .p_4()
            .gap_2()
            .child(card_title("Wishes per banner", cx))
            .child(div().flex_1().min_h_0().child(chart)),
    )
}

/// The pity of each 5★ of the pool, oldest first.
pub(super) fn five_star_pity(pool: &PoolAnalysis, data: &GameData, cx: &App) -> impl IntoElement {
    let banners = data.banners.for_pool(pool.pool);
    let points = pool
        .pulls
        .iter()
        .filter(|pull| pull.wish.rarity == Rarity::Five)
        .map(|pull| {
            let time = pull.wish.time.naive();
            let mut title = vec![pull.wish.name.clone(), time.format("%-d %b %Y").to_string()];
            title.extend(pull.banner.and_then(|ix| banners.get(ix)).map(banner_title));
            title.extend(
                pull.rate_up
                    .map(|r| format::outcome(r.outcome()).to_owned()),
            );
            PityPoint {
                tick: time.format("%Y-%m").to_string().into(),
                title: title.join(" · ").into(),
                pity: pull.pity.unwrap_or(0),
            }
        })
        .collect();
    wish_ui::pity_chart(points, pool.pool.hard_pity_five(), cx)
}

/// "Sigewinne · 4.7", or the banner's name alone.
fn banner_title(banner: &Banner) -> String {
    match &banner.version {
        Some(version) if !banner.short_name.is_empty() => {
            format!("{} · {version}", banner.short_name)
        }
        Some(version) => format!("{} · {version}", banner.name),
        None => banner.name.clone(),
    }
}
