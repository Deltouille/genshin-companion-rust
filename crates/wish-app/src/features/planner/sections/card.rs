//! An entry's card: its picture, name and detail (the handle that moves the
//! card), its edit and remove buttons, the sliders of its goal one above the
//! other, its materials and its resin. A custom entry of earlier versions has
//! no sliders.

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::slider::{Slider, SliderState};
use gpui_kit::component::{ActiveTheme as _, Icon};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, CursorStyle, Div, Entity, FontWeight, ImageSource,
    InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _, RenderOnce, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window, div, img,
};
use wish_ui::{PickerState, card, icon_button, menu_button, theme};

use super::super::controls::Move;
use super::super::model::{EntryModel, PlannerModel, Portrait, talent_names};
use super::super::resin;
use super::drag::{CardPreview, DraggedEntry, droppable};
use super::materials::grid;
use super::{Actions, Look};

/// The sliders of an entry: its level, and a character's three talents.
#[derive(Clone)]
pub(in super::super) struct CardControls {
    pub level: Entity<SliderState>,
    pub talents: Vec<Entity<SliderState>>,
}

/// Entry `index` of the model's, as a card.
#[derive(IntoElement)]
pub(in super::super) struct EntryCard {
    pub model: Rc<PlannerModel>,
    pub index: usize,
    /// None until the view has made them (a moment after the entry appears).
    pub controls: Option<CardControls>,
    /// Its move menu; none while it is the plan's only entry, or until the
    /// view has made it.
    pub moves: Option<Entity<PickerState<Move>>>,
    /// This card is the one being moved.
    pub dragged: bool,
    pub look: Look,
    pub actions: Actions,
}

impl RenderOnce for EntryCard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let entry = &self.model.entries[self.index];
        let id = entry.id();
        let scope = format!("entry-{id}");
        let tiles = div()
            .v_flex()
            .gap_3()
            .child(grid(
                &scope,
                &entry.materials,
                &self.look,
                &self.actions,
                cx,
            ))
            .children(resin::entry_resin(
                id,
                &entry.resin,
                &self.look.resin,
                &self.look.images,
                cx,
            ))
            .into_any_element();
        let edit = entry.editable().then(|| {
            let edit = self.actions.edit.clone();
            icon_button(
                SharedString::from(format!("edit-plan-{id}")),
                IconName::Settings,
                t!("planner-edit", name = entry.name.as_str()),
            )
            .ghost()
            .on_click(move |_, window, cx| edit(id, window, cx))
            .into_any_element()
        });
        let header = entry_header(entry, edit, self.moves, &self.look, &self.actions, cx);
        let theme = cx.theme();
        let card = match (&entry.ranges, &self.controls) {
            // The free-form entries of earlier versions: no sliders.
            (None, _) => card(cx).p_4().gap_4().child(header).child(tiles),
            (Some(_), None) => return card(cx).p_4().child(header).into_any_element(),
            (Some(goal), Some(controls)) => {
                let level_row = slider_row(
                    &t!("planner-level"),
                    goal.level_text(),
                    Slider::new(&controls.level)
                        .cursor_pointer()
                        .accessibility_label(t!("planner-level-of", name = entry.name.as_str())),
                    cx,
                );
                let talent_rows = talent_names()
                    .into_iter()
                    .zip(&controls.talents)
                    .zip(&goal.talents)
                    .map(|((label, state), &(from, to))| {
                        slider_row(
                            &label,
                            format!("{from} → {to}"),
                            Slider::new(state).cursor_pointer().accessibility_label(t!(
                                "planner-talent-of",
                                name = entry.name.as_str(),
                                talent = label.as_str()
                            )),
                            cx,
                        )
                    });
                card(cx)
                    .p_4()
                    .gap_4()
                    .child(header)
                    .when_some(entry.warning.clone(), |this, warning| {
                        this.child(div().text_sm().text_color(theme.warning).child(warning))
                    })
                    .child(
                        div()
                            .v_flex()
                            .gap_3()
                            .child(level_row)
                            .children(talent_rows),
                    )
                    .child(tiles)
            }
        };
        droppable(card, id, self.dragged, &self.actions, cx).into_any_element()
    }
}

/// An entry's picture, name and detail, with its edit and remove buttons. Its
/// grip opens the move menu, the keyboard's way to reorder the plan; the
/// picture and the name are the handle that drags the card.
fn entry_header(
    entry: &EntryModel,
    edit: Option<AnyElement>,
    moves: Option<Entity<PickerState<Move>>>,
    look: &Look,
    actions: &Actions,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let id = entry.id();
    let preview = CardPreview::of(entry, look);
    let (name, detail) = (preview.name().clone(), preview.detail().cloned());
    let picture = preview.picture().render(cx);
    let dragged = DraggedEntry { id, card: preview };
    let start_drag = actions.start_drag.clone();
    let grip = Icon::new(IconName::GripVertical)
        .size_4()
        .flex_shrink_0()
        .text_color(theme.muted_foreground);
    // With somewhere to go, the grip is the move menu's button.
    let grip = match moves.filter(|menu| !menu.read(cx).is_empty()) {
        Some(menu) => menu_button(
            SharedString::from(format!("plan-moves-{id}")),
            &menu,
            grip,
            t!("planner-move-menu", name = name.as_ref()),
        )
        .into_any_element(),
        None => grip.into_any_element(),
    };
    // The picture and the name are the handle that drags the card.
    let handle = div()
        .id(SharedString::from(format!("plan-handle-{id}")))
        .h_flex()
        .flex_1()
        .min_w_0()
        .gap_2()
        .cursor(CursorStyle::OpenHand)
        .on_drag(dragged, move |dragged, _, _, cx| {
            start_drag(dragged.id, cx);
            cx.new(|_| dragged.clone())
        })
        .tooltip(|window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(t!("planner-drag")).build(window, cx)
        })
        .child(picture)
        .child(
            div()
                .v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(name.clone()),
                )
                .children(detail.map(|detail| {
                    div()
                        .text_sm()
                        .truncate()
                        .text_color(theme.muted_foreground)
                        .child(detail)
                })),
        );
    let remove = actions.remove.clone();
    let remove_name = name.to_string();
    div()
        .h_flex()
        .gap_1()
        .child(grip)
        .child(handle)
        .children(edit)
        .child(
            icon_button(
                SharedString::from(format!("remove-plan-{id}")),
                IconName::Close,
                t!("planner-remove-tooltip", name = name.as_ref()),
            )
            .ghost()
            .on_click(move |_, window, cx| remove(id, remove_name.clone(), window, cx)),
        )
}

/// A slider with its label and the range it shows.
fn slider_row(label: &str, value: String, slider: Slider, cx: &App) -> Div {
    let theme = cx.theme();
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
                        .child(label.to_owned()),
                )
                .child(div().font_weight(FontWeight::SEMIBOLD).child(value)),
        )
        .child(slider)
}

/// What a card shows in the corner, ready to draw: a character's or
/// weapon's icon on its rarity tile, or a sign for a custom entry.
#[derive(Clone)]
pub(super) enum Picture {
    Icon(Option<ImageSource>, u8),
    Custom(u8),
    CustomWeapon(u8),
    Entry,
}

impl Picture {
    pub(super) fn of(portrait: &Portrait, look: &Look) -> Self {
        match portrait {
            Portrait::Character(key, stars) => Picture::Icon(
                (look.planner.character(key)).and_then(|c| look.images.character(c)),
                *stars,
            ),
            Portrait::Weapon(key, stars) => Picture::Icon(
                (look.planner.weapon(key)).and_then(|w| look.images.weapon(w)),
                *stars,
            ),
            Portrait::Custom(stars) => Picture::Custom(*stars),
            Portrait::CustomWeapon(stars) => Picture::CustomWeapon(*stars),
            Portrait::Entry => Picture::Entry,
        }
    }

    pub(super) fn render(&self, cx: &App) -> Div {
        let theme = cx.theme();
        let tile = div().size_12().flex_shrink_0().rounded(theme.radius);
        match self {
            Picture::Icon(icon, stars) => tile
                .overflow_hidden()
                .bg(theme::stars_tile(*stars, cx))
                .children(
                    icon.clone()
                        .map(|icon| img(icon).size_full().object_fit(ObjectFit::Contain)),
                ),
            Picture::Custom(stars) => tile
                .bg(theme::stars_tile(*stars, cx))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::Sparkles)
                        .size_6()
                        .text_color(gpui_kit::white().opacity(0.9)),
                ),
            Picture::CustomWeapon(stars) => tile
                .bg(theme::stars_tile(*stars, cx))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::Swords)
                        .size_6()
                        .text_color(gpui_kit::white().opacity(0.9)),
                ),
            Picture::Entry => tile
                .bg(theme.muted)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::Sparkles)
                        .size_6()
                        .text_color(theme.muted_foreground),
                ),
        }
    }
}
