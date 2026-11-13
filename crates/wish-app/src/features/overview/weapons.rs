//! The overview's Weapons tab: the weapons the account owns (the archive's
//! "Owned", one however many copies), counted against the game's: by rarity
//! and type, then each 5★ and 4★ with its best refinement, its copies and
//! who holds them. A weapon opens its archive page.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, ClickEvent, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
};
use wish_core::collection::{Collection, OwnedWeaponStats, OwnedWeaponSummary, Tally};
use wish_core::model::Rarity;
use wish_core::planner::PlannerData;
use wish_ui::{MaterialTile, big_number, button, format, stat, stat_tile, theme};

use super::wishes::figures;
use super::{
    NavigateHandler, OpenWeaponHandler, card_room, empty_state, list_grid, section, tile_grid,
};
use crate::app::images::Images;
use crate::features::imports::hoyolab_sync_button;
use crate::shell::{ArchivePage, Page};

/// The narrowest a weapon type's tile may be, in rem.
const TYPE_REMS: f32 = 9.;
/// The narrowest a weapon's entry may be, in rem: its tile and a long name.
const ENTRY_REMS: f32 = 15.;

pub(super) fn weapons_tab(
    owned: &Collection,
    planner: &PlannerData,
    images: &Images,
    on_navigate: &NavigateHandler,
    on_open_weapon: &OpenWeaponHandler,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let stats = OwnedWeaponStats::of(owned, planner);
    if stats.weapons.is_empty() {
        let on_navigate = on_navigate.clone();
        return empty_state(
            "No weapons yet",
            "Mark the weapons you have in the archive, or sync them from HoYoLAB with the \
             characters holding them: they are counted here, by rarity and type.",
            div()
                .h_flex()
                .gap_2()
                .child(hoyolab_sync_button().primary())
                .child(button("open-weapons").label("Open the weapons").on_click(
                    move |_, window, cx| {
                        on_navigate(Page::Archive(ArchivePage::Weapons), window, cx)
                    },
                )),
            cx,
        );
    }
    let room = card_room(window);
    let colors = cx.theme();
    let (foreground, muted) = (colors.foreground, colors.muted_foreground);
    let count = |n: usize| format::count(n as u64);
    let of = |tally: Tally| format!("of {}", count(tally.total));

    let totals = figures(
        [
            stat(
                "Weapons",
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
                "Refined to R5",
                big_number(count(stats.refined_fully), foreground),
                format!(
                    "{} of {} {} held",
                    count(stats.held),
                    count(stats.copies),
                    format::plural(stats.copies as u32, "copy", "copies")
                ),
                cx,
            ),
        ],
        if room > 44. { 4 } else { 2 },
        cx,
    );

    let types = stats.types.iter().map(|(kind, tally)| {
        stat_tile(
            div().child(*kind),
            of(*tally),
            big_number(count(tally.owned), foreground),
            cx,
        )
    });

    let mut body = div().v_flex().gap_6().child(totals).child(section(
        "By type",
        tile_grid(stats.types.len(), TYPE_REMS, room).children(types),
        cx,
    ));
    for (rarity, title) in [(5, "5★ weapons"), (4, "4★ weapons")] {
        let list: Vec<&OwnedWeaponSummary> = stats.of_rarity(rarity).collect();
        if list.is_empty() {
            continue;
        }
        let entries = list
            .iter()
            .map(|weapon| entry(weapon, planner, images, on_open_weapon, cx));
        body = body.child(section(
            title,
            list_grid(ENTRY_REMS, room).children(entries),
            cx,
        ));
    }
    let lesser = stats.weapons.iter().filter(|w| w.rarity < 4).count();
    body.when(lesser > 0, |this| {
        this.child(div().text_sm().text_color(muted).child(format!(
            "And {} {} of 3★ or less, in the archive.",
            count(lesser),
            format::plural(lesser as u32, "weapon", "weapons")
        )))
    })
}

/// One weapon: its icon on its rarity tile, its name, then its best
/// refinement, its copies and who holds them. It opens its archive page.
fn entry(
    weapon: &OwnedWeaponSummary,
    planner: &PlannerData,
    images: &Images,
    on_open_weapon: &OpenWeaponHandler,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let icon = planner.weapon(&weapon.key).and_then(|w| images.weapon(w));
    let mut detail = format!("R{}", weapon.refinement);
    if weapon.copies > 1 {
        detail.push_str(&format!(" · {} copies", weapon.copies));
    }
    let holders: Vec<&str> = weapon
        .holders
        .iter()
        .map(|key| {
            planner
                .character(key)
                .map_or(key.as_str(), |c| c.name.as_str())
        })
        .collect();
    if !holders.is_empty() {
        detail.push_str(&format!(" · {}", holders.join(", ")));
    }
    let open = {
        let (key, on_open_weapon) = (weapon.key.clone(), on_open_weapon.clone());
        move |_: &ClickEvent, window: &mut Window, cx: &mut App| on_open_weapon(&key, window, cx)
    };
    div()
        .id(SharedString::from(format!("owned-{}", weapon.key)))
        .h_flex()
        .gap_3()
        .p_2()
        .rounded(theme.radius_lg)
        .bg(theme.muted)
        .cursor_pointer()
        .hover(|this| this.bg(theme.accent))
        .on_click(open)
        .child(MaterialTile::bare(icon, weapon.rarity))
        .child(
            div()
                .v_flex()
                .min_w_0()
                .gap_0p5()
                .child(
                    div()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(weapon.name.clone()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .truncate()
                        .child(detail),
                ),
        )
}
