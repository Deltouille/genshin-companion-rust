//! The overview's Characters tab: the characters the account owns (the
//! archive's "Owned"), counted against the game's: by rarity, then by
//! element and weapon, then their constellations.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::{App, Div, ParentElement as _, Styled as _, Window, div, img};
use wish_core::collection::{CharacterStats, Collection, MAX_TALENT, Tally};
use wish_core::model::Rarity;
use wish_core::planner::{Level, PlannerData};
use wish_ui::{big_number, button, format, stat, stat_tile, theme};

use super::wishes::figures;
use super::{NavigateHandler, card_room, empty_state, section, tile_grid};
use crate::app::images::Images;
use crate::shell::{ArchivePage, Page};

/// The narrowest a tile may be, in rem: an element's icon and name, "of 14"
/// and the count.
const TILE_REMS: f32 = 9.;

pub(super) fn characters_tab(
    owned: &Collection,
    planner: &PlannerData,
    images: &Images,
    on_navigate: &NavigateHandler,
    window: &mut Window,
    cx: &mut App,
) -> Div {
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
    let stats = CharacterStats::of(owned, planner);
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
                big_number(count(stats.max_level), foreground),
                t!("overview-crowns", count = stats.crowns, level = MAX_TALENT),
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
    // Each constellation's share of the characters owned.
    let constellations = stats.constellations.iter().enumerate().map(|(c, n)| {
        stat_tile(
            div().child(format!("C{c}")),
            format::whole_percent(*n as f64 / stats.all.owned.max(1) as f64),
            big_number(count(*n), foreground),
            cx,
        )
    });

    div()
        .v_flex()
        .gap_6()
        .child(totals)
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
            t!("overview-constellations"),
            tile_grid(stats.constellations.len(), TILE_REMS, room).children(constellations),
            cx,
        ))
}
