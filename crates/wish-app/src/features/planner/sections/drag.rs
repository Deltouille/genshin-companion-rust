//! Moving a card to another place in the plan: the card is taken by its
//! header, a copy of it follows the pointer, and the card it is dropped on
//! gives it its place (the order decides who gets the inventory first).

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{ActiveTheme as _, Icon};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Context, Div, ImageSource, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, Render, SharedString, Styled as _, Window, div, px, relative,
};
use wish_ui::{MaterialTile, badge, format};

use super::super::model::{EntryModel, talent_names};
use super::card::Picture;
use super::{Actions, Look};

/// A card being dragged to another place in the plan, and what follows the
/// pointer meanwhile.
#[derive(Clone)]
pub(in super::super) struct DraggedEntry {
    pub id: i64,
    pub card: CardPreview,
}

/// A copy of a card, without its controls: what follows the pointer while
/// the card is moved.
#[derive(Clone)]
pub(in super::super) struct CardPreview {
    width: Pixels,
    picture: Picture,
    name: SharedString,
    detail: Option<SharedString>,
    /// Each slider: its label, its range as text, and where the range starts
    /// and ends (0 to 1).
    ranges: Vec<(SharedString, SharedString, f32, f32)>,
    /// Each tile: icon, stars, what is still lacking, whether nothing is,
    /// how many are crafted.
    tiles: Vec<(Option<ImageSource>, u8, SharedString, bool, u64)>,
    /// What the game data does not have: label, done.
    named: Vec<(SharedString, bool)>,
}

impl CardPreview {
    pub(super) fn of(entry: &EntryModel, look: &Look) -> Self {
        let mut ranges = Vec::new();
        if let Some(goal) = &entry.ranges {
            ranges.push((
                t!("planner-level").into(),
                goal.level_text().into(),
                goal.position(goal.from),
                goal.position(goal.to),
            ));
            for (label, &(from, to)) in talent_names().into_iter().zip(&goal.talents) {
                let at = |level: u8| f32::from(level.saturating_sub(1)) / 9.;
                ranges.push((
                    label.into(),
                    format!("{from} → {to}").into(),
                    at(from),
                    at(to),
                ));
            }
        }
        let tiles = (entry.materials.known.iter())
            .filter_map(|line| {
                let material = look.planner.material(line.id)?;
                Some((
                    look.images.material(material),
                    material.rarity,
                    format::compact(line.lack).into(),
                    line.lack == 0,
                    line.crafted,
                ))
            })
            .collect();
        let named = (entry.materials.named.iter())
            .map(|line| (line.label().into(), line.lack == 0))
            .collect();
        Self {
            width: look.card_width,
            picture: Picture::of(&entry.portrait, look),
            name: entry.name.clone().into(),
            detail: entry.detail.clone().map(Into::into),
            ranges,
            tiles,
            named,
        }
    }

    pub(super) fn picture(&self) -> &Picture {
        &self.picture
    }

    pub(super) fn name(&self) -> &SharedString {
        &self.name
    }

    pub(super) fn detail(&self) -> Option<&SharedString> {
        self.detail.as_ref()
    }
}

impl Render for DraggedEntry {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Drawn where the card was: the pointer took it by its header, which
        // sits a padding (one rem) in.
        let rem = window.rem_size();
        card_copy(&self.card, cx).ml(-rem).mt(-rem)
    }
}

/// The copy of a card that follows the pointer.
fn card_copy(card: &CardPreview, cx: &App) -> Div {
    let theme = cx.theme();
    let ranges = card.ranges.iter().map(|(label, value, from, to)| {
        div()
            .v_flex()
            .gap_1p5()
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .text_sm()
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .child(label.clone()),
                    )
                    .child(
                        div()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(value.clone()),
                    ),
            )
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(6.))
                    .my(px(5.))
                    .rounded_full()
                    .bg(theme.primary.opacity(0.25))
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .h(px(6.))
                            .left(relative(*from))
                            .w(relative(to - from))
                            .rounded_full()
                            .bg(theme.primary),
                    ),
            )
    });
    let tiles = card
        .tiles
        .iter()
        .map(|(icon, stars, label, done, crafted)| {
            MaterialTile::new(icon.clone(), *stars, label.clone())
                .done(*done)
                .crafted(*crafted)
        });
    let named = card.named.iter().map(|(label, done)| {
        badge(label.clone(), (theme.foreground, theme.muted), cx)
            .text_sm()
            .when(*done, |this| this.opacity(0.35))
    });
    div()
        .w(card.width)
        .p_4()
        .v_flex()
        .gap_4()
        .rounded(theme.radius_lg)
        .bg(theme.group_box)
        .text_color(theme.group_box_foreground)
        .border_1()
        .border_color(theme.primary)
        .shadow_lg()
        .child(
            div()
                .h_flex()
                .gap_2()
                .child(
                    Icon::new(IconName::GripVertical)
                        .size_4()
                        .flex_shrink_0()
                        .text_color(theme.muted_foreground),
                )
                .child(card.picture.render(cx))
                .child(
                    div()
                        .v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .truncate()
                                .child(card.name.clone()),
                        )
                        .children(card.detail.clone().map(|detail| {
                            div()
                                .text_sm()
                                .truncate()
                                .text_color(theme.muted_foreground)
                                .child(detail)
                        })),
                ),
        )
        .when(!card.ranges.is_empty(), |this| {
            this.child(div().v_flex().gap_3().children(ranges))
        })
        .child(div().h_flex().flex_wrap().gap_2().children(tiles))
        .when(!card.named.is_empty(), |this| {
            this.child(div().h_flex().flex_wrap().gap_2().children(named))
        })
}

/// Makes `card`, entry `id`'s, a place to drop another entry: it takes this
/// one's place in the plan. `dragged`: this card is the one being moved, and
/// stays dimmed in its place meanwhile.
pub(super) fn droppable(
    card: Div,
    id: i64,
    dragged: bool,
    actions: &Actions,
    cx: &App,
) -> gpui_kit::Stateful<Div> {
    let dragged = cx.has_active_drag() && dragged;
    let drop_onto = actions.drop_onto.clone();
    card.id(SharedString::from(format!("plan-card-{id}")))
        .when(dragged, |this| this.opacity(0.35))
        .drag_over::<DraggedEntry>(move |style, dragged, _, cx| {
            if dragged.id == id {
                style
            } else {
                style.bg(cx.theme().primary.opacity(0.15))
            }
        })
        .on_drop(move |dragged: &DraggedEntry, _, cx| drop_onto(dragged.id, id, cx))
}
