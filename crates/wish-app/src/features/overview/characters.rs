//! The overview's Characters tab: the characters the account owns (the
//! archive's "Owned"), counted against the game's: by rarity; how the
//! collection grew with the game's versions; constellations, levels and
//! talents; what raising them cost; then by element, weapon and region.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::chart::AreaChart;
use gpui_kit::{
    App, Div, IntoElement, ParentElement as _, SharedString, Styled as _, Window, div, img, rems,
};
use wish_core::archive::ArchiveData;
use wish_core::collection::{
    CharacterStats, Collection, Constellations, LEVEL_BRACKETS, MAX_TALENT, TALENT_BRACKETS, Tally,
    VersionTally,
};
use wish_core::model::Rarity;
use wish_core::planner::{CROWN, EXP_BOOKS, Level, MORA, PlannerData, ResinOptions, Role};
use wish_ui::{
    Donut, DonutSlice, big_number, button, card, card_title, format, legend, stat, stat_tile, theme,
};

use super::wishes::figures;
use super::{NavigateHandler, card_room, empty_state, section, tile_grid};
use crate::app::images::Images;
use crate::shell::{ArchivePage, Page};

/// The narrowest a tile may be, in rem: an element's icon and name, "of 14"
/// and the count.
const TILE_REMS: f32 = 9.;
/// The narrowest a donut's card may be, in rem: the ring and its legend.
const DONUT_REMS: f32 = 24.;
/// A donut card's height, in rem.
const DONUT_HEIGHT_REMS: f32 = 16.;
/// The narrowest a tile of what raising the characters cost may be, in rem.
const INVESTED_REMS: f32 = 13.;

/// Everything the tab shows, from the account's collection and the game data.
pub(super) struct CharactersTab<'a> {
    pub owned: &'a Collection,
    pub planner: &'a PlannerData,
    pub archive: &'a ArchiveData,
    pub resin: &'a ResinOptions,
    pub images: &'a Images,
    pub on_navigate: &'a NavigateHandler,
}

pub(super) fn characters_tab(tab: CharactersTab, window: &mut Window, cx: &mut App) -> Div {
    let CharactersTab {
        owned,
        planner,
        archive,
        resin,
        images,
        on_navigate,
    } = tab;
    if owned.characters.is_empty() {
        let on_navigate = on_navigate.clone();
        return empty_state(
            t!("overview-no-characters"),
            t!("overview-no-characters-help"),
            div()
                .h_flex()
                .gap_2()
                .child(crate::features::imports::hoyolab_sync_button().primary())
                .child(
                    button("open-characters")
                        .label(t!("overview-open-characters"))
                        .on_click(move |_, window, cx| {
                            on_navigate(Page::Archive(ArchivePage::Characters), window, cx)
                        }),
                ),
            cx,
        );
    }
    let stats = CharacterStats::of(owned, planner, archive);
    let room = card_room(window);
    let foreground = cx.theme().foreground;
    let count = |n: usize| format::count(n as u64);
    let of = |tally: Tally| t!("overview-of", total = tally.total);

    let totals = figures(
        [
            stat(
                t!("overview-characters"),
                big_number(count(stats.all.owned), foreground),
                of(stats.all),
                cx,
            ),
            stat(
                "5★",
                big_number(count(stats.five.owned), theme::rarity(Rarity::Five, cx)),
                of(stats.five),
                cx,
            ),
            stat(
                "4★",
                big_number(count(stats.four.owned), theme::rarity(Rarity::Four, cx)),
                of(stats.four),
                cx,
            ),
            stat(
                t!("overview-level", level = Level::MAX.level),
                big_number(count(stats.max_level()), foreground),
                t!(
                    "overview-crowns",
                    count = stats.crowns(),
                    level = MAX_TALENT
                ),
                cx,
            ),
        ],
        if room > 44. { 4 } else { 2 },
        cx,
    );

    let tile = |label: Div, tally: Tally, cx: &App| {
        stat_tile(
            label,
            of(tally),
            big_number(count(tally.owned), foreground),
            cx,
        )
    };
    let elements = stats.elements.iter().map(|(element, tally)| {
        let label = div()
            .h_flex()
            .gap_2()
            .children(images.element(element).map(|icon| img(icon).size_6()))
            .child(format::element(element));
        tile(label, *tally, cx)
    });
    let weapons = stats
        .weapons
        .iter()
        .map(|(weapon, tally)| tile(div().child(format::weapon_type(weapon)), *tally, cx));
    let regions = stats.regions.iter().map(|(region, tally)| {
        let name = match region {
            Some(region) => region.to_string(),
            None => t!("overview-region-none"),
        };
        tile(div().child(name), *tally, cx)
    });

    let donuts = [
        constellations(
            "five-constellations",
            t!("overview-five-constellations"),
            &stats.five_constellations,
            Rarity::Five,
            cx,
        ),
        constellations(
            "four-constellations",
            t!("overview-four-constellations"),
            &stats.four_constellations,
            Rarity::Four,
            cx,
        ),
        Donut::new(
            "levels",
            t!("overview-levels"),
            brackets(&LEVEL_BRACKETS, Level::MAX.level, &stats.levels, cx),
            t!("overview-characters-word", count = stats.all.owned),
        )
        .label_rems(3.),
        Donut::new(
            "talents",
            t!("overview-talents"),
            brackets(&TALENT_BRACKETS, MAX_TALENT, &stats.talents, cx),
            t!(
                "overview-talents-word",
                count = stats.talents.iter().sum::<usize>()
            ),
        )
        .label_rems(3.)
        .footer(t!(
            "overview-talent-averages",
            normal = stats.average_talent(0).unwrap_or_default(),
            skill = stats.average_talent(1).unwrap_or_default(),
            burst = stats.average_talent(2).unwrap_or_default()
        )),
    ];

    div()
        .v_flex()
        .gap_6()
        .child(totals)
        .child(by_version(&stats.by_version, cx))
        .child(
            tile_grid(donuts.len(), DONUT_REMS, room).children(
                donuts
                    .into_iter()
                    .map(|donut| div().h(rems(DONUT_HEIGHT_REMS)).child(donut)),
            ),
        )
        .child(invested(&stats, planner, resin, images, room, cx))
        .child(section(
            t!("overview-by-element"),
            tile_grid(stats.elements.len(), TILE_REMS, room).children(elements),
            cx,
        ))
        .child(section(
            t!("overview-by-weapon"),
            tile_grid(stats.weapons.len(), TILE_REMS, room).children(weapons),
            cx,
        ))
        .child(section(
            t!("overview-by-region"),
            tile_grid(stats.regions.len(), TILE_REMS, room).children(regions),
            cx,
        ))
}

/// How the collection grew with the game: the characters out by each version
/// as an area, and the account's among them over it. Both count characters,
/// on one axis; the gap between them is what the account lacks.
fn by_version(versions: &[VersionTally], cx: &App) -> impl IntoElement {
    // The characters out: the muted colour, a backdrop; the account's: the
    // chart blue, which passes the dataviz validator on both card surfaces.
    let released = cx.theme().muted_foreground;
    let owned = theme::rarity_chart(Rarity::Three, cx);
    let most = versions.last().map_or(0, |version| version.released);
    let (top, y_ticks) = wish_ui::axis::round_axis(u32::try_from(most).unwrap_or(u32::MAX), 6);
    // Version labels need about 3 rem each; the tooltip names every one.
    let ticks = versions.len().min(10);
    let chart = AreaChart::new(versions.to_vec())
        .appear(false)
        .linear()
        .x(|v: &VersionTally| SharedString::from(v.version.clone()))
        .x_tick_count(ticks)
        .y_axis(true)
        .y_domain(0.0, f64::from(top))
        // No headroom above the top, or the ticks would leave the round steps.
        .y_padding(0.0, 0.0)
        .y_tick_count(y_ticks)
        .y_tick_format(|v| format!("{v:.0}"))
        .y(|v: &VersionTally| v.released as f64)
        .stroke(released)
        .fill(released.opacity(0.15))
        .name(t!("overview-released"))
        .y(|v: &VersionTally| v.owned as f64)
        .stroke(owned)
        .fill(owned.opacity(0.35))
        .name(t!("overview-owned"))
        .tooltip_title(|v: &VersionTally| {
            t!("overview-version", version = v.version.clone()).into()
        })
        .tooltip_value(|_, _, value| format::count(value as u64).into());

    card(cx)
        .h(rems(18.))
        .gap_3()
        .p_4()
        .child(card_title(t!("overview-by-version"), cx).child(legend(
            [
                (released, t!("overview-released").into()),
                (owned, t!("overview-owned").into()),
            ],
            cx,
        )))
        .child(div().flex_1().min_h_0().child(chart))
}

/// A donut of the owned characters of one rarity by constellation, C0 to C6,
/// in shades of the rarity's colour: the higher, the stronger.
fn constellations(
    id: &'static str,
    title: String,
    counts: &Constellations,
    rarity: Rarity,
    cx: &App,
) -> Donut {
    let shades = theme::rarity_ramp(rarity, counts.len(), cx);
    let slices = (counts.iter().zip(shades))
        .enumerate()
        .map(|(c, (&n, color))| DonutSlice {
            label: format!("C{c}").into(),
            label_color: None,
            count: n as u64,
            color,
        })
        .collect();
    let owned: usize = counts.iter().sum();
    Donut::new(
        id,
        title,
        slices,
        t!("overview-characters-word", count = owned),
    )
}

/// The slices of `counts`, one per bracket of `brackets` (each by its lowest
/// value, highest first, the first ending at `top`): "90", "80–89"…, in
/// shades of the chart blue, the highest the strongest.
fn brackets(brackets: &[u8], top: u8, counts: &[usize], cx: &App) -> Vec<DonutSlice> {
    let mut shades = theme::rarity_ramp(Rarity::Three, counts.len(), cx);
    shades.reverse();
    (brackets.iter().enumerate().zip(counts.iter().zip(shades)))
        .map(|((ix, &lowest), (&n, color))| {
            let highest = if ix == 0 { top } else { brackets[ix - 1] - 1 };
            let label = if lowest == highest {
                lowest.to_string()
            } else {
                format!("{lowest}–{highest}")
            };
            DonutSlice {
                label: label.into(),
                label_color: None,
                count: n as u64,
                color,
            }
        })
        .collect()
}

/// What raising the owned characters to where they are cost, from level 1
/// with talents at 1: Mora, EXP, talent books, weekly boss materials and
/// crowns, and the resin all that takes on average.
fn invested(
    stats: &CharacterStats,
    planner: &PlannerData,
    resin: &ResinOptions,
    images: &Images,
    room: f32,
    cx: &App,
) -> Div {
    let foreground = cx.theme().foreground;
    let spent = |id| stats.invested.get(&id).copied().unwrap_or(0);
    let material = |id| planner.material(id);
    // A material by its icon and name in the game data.
    let named = |id| {
        let label = div().h_flex().gap_2();
        match material(id) {
            Some(material) => label
                .children(images.material(material).map(|icon| img(icon).size_6()))
                .child(material.name.to_string()),
            None => label,
        }
    };
    let estimate = planner.resin_estimate(&stats.invested, resin);
    let tiles = [
        stat_tile(
            named(MORA),
            format::count(spent(MORA)),
            big_number(format::compact(spent(MORA)), foreground),
            cx,
        ),
        stat_tile(
            named(EXP_BOOKS[0].0),
            t!("overview-invested-levels"),
            big_number(format::count(spent(EXP_BOOKS[0].0)), foreground),
            cx,
        ),
        stat_tile(
            div().child(t!("overview-talent-books")),
            t!("overview-every-tier"),
            big_number(
                format::count(stats.invested_in(Role::TalentBook, planner)),
                foreground,
            ),
            cx,
        ),
        stat_tile(
            div().child(t!("overview-weekly-materials")),
            t!("overview-invested-talents"),
            big_number(
                format::count(stats.invested_in(Role::WeeklyMaterial, planner)),
                foreground,
            ),
            cx,
        ),
        stat_tile(
            named(CROWN),
            t!("overview-invested-talents"),
            big_number(format::count(spent(CROWN)), foreground),
            cx,
        ),
        stat_tile(
            div()
                .h_flex()
                .gap_2()
                .children(images.resin().map(|icon| img(icon).size_6()))
                .child(t!("overview-resin")),
            t!("planner-days", days = estimate.days(resin.resin_per_day)),
            big_number(format::compact(estimate.resin()), foreground),
            cx,
        ),
    ];
    let count = tiles.len();
    section(
        t!("overview-invested"),
        div()
            .v_flex()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("overview-invested-help")),
            )
            .child(tile_grid(count, INVESTED_REMS, room).children(tiles)),
        cx,
    )
}
