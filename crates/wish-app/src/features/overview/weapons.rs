//! The overview's Weapons tab: the weapons the account's wishes brought, by
//! rarity and type, then each 4★ and 5★ with the refinement its copies make.
//!
//! Counted from the wishes alone, so a refinement is a floor: copies from the
//! shop, events or the forge aren't wishes.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Div, FontWeight, InteractiveElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div,
};
use wish_core::collection::{WishedWeapon, WishedWeapons};
use wish_core::model::Rarity;
use wish_core::planner::PlannerData;
use wish_ui::{MaterialTile, big_number, format, stat, stat_tile, theme};

use super::wishes::figures;
use super::{OpenWeaponHandler, card_room, empty_state, list_grid, section, tile_grid};
use crate::app::images::Images;
use crate::app::library::AccountSummary;
use crate::features::wishes::import::{import_file_button, import_game_button};

/// The narrowest a weapon type's tile may be, in rem.
const TYPE_REMS: f32 = 9.;
/// The narrowest a weapon's entry may be, in rem: its tile and a long name.
const ENTRY_REMS: f32 = 15.;

pub(super) fn weapons_tab(
    summary: Option<&AccountSummary>,
    planner: &PlannerData,
    images: &Images,
    on_open_weapon: &OpenWeaponHandler,
    busy: bool,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let weapons = summary.map_or_else(WishedWeapons::default, |summary| {
        let wishes = summary
            .analysis
            .pools
            .iter()
            .flat_map(|pool| pool.pulls.iter().map(|pull| &pull.wish));
        WishedWeapons::of(wishes, planner)
    });
    if weapons.is_empty() {
        return empty_state(
            "No weapons wished yet",
            "The weapons your wishes bring are counted here, with the refinement their copies make.",
            div()
                .h_flex()
                .gap_2()
                .child(import_game_button().primary())
                .child(import_file_button(busy)),
            cx,
        );
    }
    let room = card_room(window);
    let colors = cx.theme();
    let (foreground, muted) = (colors.foreground, colors.muted_foreground);
    let count = |n: u64| format::count(n);
    let copies = |n: u32| {
        format!(
            "{} {}",
            count(n.into()),
            format::plural(n, "copy", "copies")
        )
    };
    let distinct = |rarity: Rarity| weapons.of_rarity(rarity).count() as u64;

    let totals = figures(
        [
            stat(
                "Weapons wished",
                big_number(count(weapons.total().into()), foreground),
                format!("{} 3★", count(weapons.three_stars.into())),
                cx,
            ),
            stat(
                "5★",
                big_number(
                    count(distinct(Rarity::Five)),
                    theme::rarity(Rarity::Five, cx),
                ),
                copies(weapons.copies(Rarity::Five)),
                cx,
            ),
            stat(
                "4★",
                big_number(
                    count(distinct(Rarity::Four)),
                    theme::rarity(Rarity::Four, cx),
                ),
                copies(weapons.copies(Rarity::Four)),
                cx,
            ),
            stat(
                "Refined to R5",
                big_number(count(weapons.refined_fully() as u64), foreground),
                "From five copies",
                cx,
            ),
        ],
        if room > 44. { 4 } else { 2 },
        cx,
    );

    let by_type = weapons.by_type();
    let types = by_type.iter().map(|(kind, n, c)| {
        stat_tile(
            div().child(*kind),
            copies(*c),
            big_number(count(*n as u64), foreground),
            cx,
        )
    });

    let mut body = div()
        .v_flex()
        .gap_6()
        .child(
            div()
                .text_sm()
                .text_color(muted)
                .child("From your wishes alone: copies from the shop, events and the forge aren’t counted, so a refinement may be higher in the game."),
        )
        .child(totals)
        .child(section(
            "By type",
            tile_grid(by_type.len(), TYPE_REMS, room).children(types),
            cx,
        ));
    for (rarity, title) in [(Rarity::Five, "5★ weapons"), (Rarity::Four, "4★ weapons")] {
        let list: Vec<&WishedWeapon> = weapons.of_rarity(rarity).collect();
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
    body
}

/// One weapon: its icon on its rarity tile, its name, its refinement and
/// copies. A weapon the game data knows opens its archive page.
fn entry(
    weapon: &WishedWeapon,
    planner: &PlannerData,
    images: &Images,
    on_open_weapon: &OpenWeaponHandler,
    cx: &App,
) -> impl gpui_kit::IntoElement {
    let theme = cx.theme();
    let known = weapon.key.as_deref().and_then(|key| planner.weapon(key));
    let icon = known.and_then(|weapon| images.weapon(weapon));
    let copies = format!(
        "R{} · {} {}",
        weapon.refinement(),
        weapon.copies,
        format::plural(weapon.copies, "copy", "copies")
    );
    let open = weapon.key.clone().map(|key| {
        let on_open_weapon = on_open_weapon.clone();
        move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
            on_open_weapon(&key, window, cx)
        }
    });
    div()
        .id(SharedString::from(format!("wished-{}", weapon.name)))
        .h_flex()
        .gap_3()
        .p_2()
        .rounded(theme.radius_lg)
        .bg(theme.muted)
        .child(MaterialTile::bare(icon, weapon.rarity.stars()))
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
                        .child(copies),
                ),
        )
        .when_some(open, |this, open| {
            this.cursor_pointer()
                .hover(|this| this.bg(theme.accent))
                .on_click(open)
        })
}
