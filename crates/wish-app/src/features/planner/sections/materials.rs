//! A list of materials: the known ones on their rarity tiles, in the game's
//! inventory order, then those the game data doesn't have, by name. Each
//! opens the inventory dialog.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Div, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, div,
};
use wish_core::planner::CustomItem;
use wish_ui::{MaterialTile, badge, format};

use super::super::model::MaterialsModel;
use super::{Actions, Look};

/// `materials` as tiles and badges, or "Nothing to spend.". `scope` keeps the
/// tiles' ids apart from other lists'.
pub(super) fn grid(
    scope: &str,
    materials: &MaterialsModel,
    look: &Look,
    actions: &Actions,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    if materials.is_empty() {
        return div()
            .text_sm()
            .text_color(theme.muted_foreground)
            .child("Nothing to spend.")
            .into_any_element();
    }
    let tiles = (materials.known.iter()).filter_map(|line| {
        let material = look.planner.material(line.id)?;
        let item = CustomItem::Material(line.id);
        Some(
            clickable(format!("{scope}-{}", line.id), item, &line.tip, actions, cx).child(
                MaterialTile::new(
                    look.images.material(material),
                    material.rarity,
                    format::compact(line.lack),
                )
                .done(line.lack == 0)
                .crafted(line.crafted),
            ),
        )
    });
    let named: Vec<_> = (materials.named.iter())
        .map(|line| {
            let item = CustomItem::Named(line.name.clone());
            let id = format!("{scope}-named-{}", line.name);
            clickable(id, item, &line.tip, actions, cx).child(
                badge(line.label(), (theme.foreground, theme.muted), cx)
                    .text_sm()
                    .when(line.lack == 0, |this| this.opacity(0.35)),
            )
        })
        .collect();
    div()
        .v_flex()
        .gap_2()
        .child(div().h_flex().flex_wrap().gap_2().children(tiles))
        .when(!named.is_empty(), |this| {
            this.child(div().h_flex().flex_wrap().gap_2().children(named))
        })
        .into_any_element()
}

/// A material that opens the inventory dialog when clicked.
fn clickable(
    id: String,
    item: CustomItem,
    tip: &str,
    actions: &Actions,
    cx: &App,
) -> gpui_kit::Stateful<Div> {
    let tip = SharedString::from(tip.to_owned());
    let open = actions.open_material.clone();
    div()
        .id(SharedString::from(id))
        .rounded(cx.theme().radius)
        .cursor_pointer()
        .hover(|style| style.opacity(0.8))
        .on_click(move |_, window, cx| open(item.clone(), window, cx))
        .tooltip(move |window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
        })
}
