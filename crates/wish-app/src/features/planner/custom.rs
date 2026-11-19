//! The custom character or weapon dialog, after genshin-center's: one the
//! game data does not have yet (a leak), given a name, its stars and the
//! material of each of its roles (six for a character, three for a weapon). Clicking a role shows the series the game data
//! has for it, its placeholder "?" first (a material not known yet); one the
//! data does not have yet can also be typed by name. It then costs what any
//! character, or any weapon of its rarity, costs
//! (`PlannerData::custom_character_cost`, `custom_weapon_cost`).

use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, WindowExt as _};
use gpui_kit::{
    AppContext as _, Context, Div, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, prelude::FluentBuilder as _, rems,
};
use wish_core::planner::{
    CharacterGoal, CustomItem, CustomMaterials, MaterialId, PlanEntry, PlanTarget, PlannerData,
    Role, WeaponGoal,
};

use super::{PlannerView, report_plan};
use crate::app::images::Images;
use wish_ui::{MaterialTile, Segmented, button, dialog_footer, form_dialog, icon_button, input};

/// The roles side by side in the form.
const SLOT_COLUMNS: usize = 3;

/// What a custom entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CustomKind {
    Character,
    Weapon,
}

impl CustomKind {
    /// The roles whose material the dialog asks for.
    fn roles(self) -> &'static [Role] {
        match self {
            CustomKind::Character => &Role::CHARACTER,
            CustomKind::Weapon => &Role::WEAPON,
        }
    }

    /// What the form says when it has no name.
    fn name_it(self) -> String {
        match self {
            CustomKind::Character => t!("planner-name-character"),
            CustomKind::Weapon => t!("planner-name-weapon"),
        }
    }
}

/// The custom entry dialog's form: its kind, name, stars and the material
/// chosen for each role, or the series picker of one role.
pub(super) struct CustomForm {
    planner: Arc<PlannerData>,
    images: Arc<Images>,
    kind: CustomKind,
    /// An edit keeps its kind.
    editing: bool,
    name: Entity<InputState>,
    stars: u8,
    materials: CustomMaterials,
    /// The role whose material is being picked: the picker takes the
    /// form's place meanwhile.
    picking: Option<Role>,
    search: Entity<InputState>,
    /// A material the game data does not have yet, by name.
    other: Entity<InputState>,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl CustomForm {
    fn new(
        entry: Option<&PlanTarget>,
        planner: Arc<PlannerData>,
        images: Arc<Images>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (kind, name, stars, materials) = match entry {
            Some(PlanTarget::CustomCharacter {
                name,
                stars,
                materials,
                ..
            }) => (
                CustomKind::Character,
                name.clone(),
                *stars,
                materials.clone(),
            ),
            Some(PlanTarget::CustomWeapon {
                name,
                stars,
                materials,
                ..
            }) => (CustomKind::Weapon, name.clone(), *stars, materials.clone()),
            _ => (
                CustomKind::Character,
                String::new(),
                5,
                CustomMaterials::new(),
            ),
        };
        let editing = entry.is_some();
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(t!("planner-name"))
                .default_value(name)
        });
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(t!("planner-search")));
        let other = cx.new(|cx| InputState::new(window, cx).placeholder(t!("planner-type-name")));
        // The picker filters as the search is typed.
        let subscriptions = vec![cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })];
        Self {
            planner,
            images,
            kind,
            editing,
            name,
            stars,
            materials,
            picking: None,
            search,
            other,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    pub(super) fn pick(&mut self, role: Role, window: &mut Window, cx: &mut Context<Self>) {
        self.picking = Some(role);
        self.error = None;
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.other
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.search.update(cx, |state, cx| state.focus(window, cx));
        cx.notify();
    }

    fn choose(&mut self, role: Role, item: CustomItem, cx: &mut Context<Self>) {
        self.materials.insert(role, item);
        self.picking = None;
        self.error = None;
        cx.notify();
    }

    /// Uses the typed name for the role being picked.
    fn choose_other(&mut self, cx: &mut Context<Self>) {
        let Some(role) = self.picking else {
            return;
        };
        let name = self.other.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.error = Some(t!("planner-type-name-first").into());
            cx.notify();
            return;
        }
        // A name the game data knows is that material's series.
        let known = self.planner.material_named(&name).and_then(|id| {
            self.planner
                .families(role)
                .iter()
                .find(|tiers| tiers.contains(&id))
                .map(|tiers| tiers[0])
        });
        let item = known.map_or(CustomItem::Named(name), CustomItem::Material);
        self.choose(role, item, cx);
    }

    /// Switches between a character and a weapon; the materials of roles
    /// the other has go.
    fn set_kind(&mut self, kind: CustomKind, cx: &mut Context<Self>) {
        self.kind = kind;
        self.materials.retain(|role, _| kind.roles().contains(role));
        self.error = None;
        cx.notify();
    }

    /// The character or weapon, or `None` with the reason shown.
    fn submit(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<(CustomKind, String, u8, CustomMaterials)> {
        let name = self.name.read(cx).value().trim().to_owned();
        let missing = (self.kind.roles().iter()).find(|role| !self.materials.contains_key(role));
        let problem = if name.is_empty() {
            Some(self.kind.name_it())
        } else {
            missing.map(|role| t!("planner-choose-its", role = role.to_string()))
        };
        self.error = problem.clone().map(Into::into);
        self.picking = None;
        cx.notify();
        match problem {
            Some(_) => None,
            None => Some((self.kind, name, self.stars, self.materials.clone())),
        }
    }

    /// The series a role's material stands for: the top tier, as the game
    /// shows it.
    fn shown(&self, role: Role, item: &CustomItem) -> (Option<gpui_kit::ImageSource>, u8, String) {
        match item {
            CustomItem::Material(first) => {
                let top = self
                    .planner
                    .family(role, *first)
                    .and_then(|tiers| tiers.last().copied())
                    .unwrap_or(*first);
                match self.planner.material(top) {
                    // "?", by the role's name, without a tier.
                    Some(material) if material.is_placeholder() => {
                        (None, material.rarity, role.placeholder_name())
                    }
                    Some(material) => (
                        self.images.material(material),
                        material.rarity,
                        material.name.to_string(),
                    ),
                    None => (None, 1, top.to_string()),
                }
            }
            CustomItem::Named(name) => (None, 1, name.clone()),
        }
    }

    fn render_form(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let form = cx.entity().downgrade();
        let stars = Segmented::new("custom-stars")
            .option("4★")
            .option("5★")
            .selected(usize::from(self.stars == 5))
            .on_select(move |ix, _, cx| {
                form.update(cx, |form, cx| {
                    form.stars = if *ix == 1 { 5 } else { 4 };
                    cx.notify();
                })
                .ok();
            });
        let kinds = (!self.editing).then(|| {
            let form = cx.entity().downgrade();
            Segmented::new("custom-kind")
                .option(t!("planner-character"))
                .option(t!("planner-weapon"))
                .selected(usize::from(self.kind == CustomKind::Weapon))
                .on_select(move |ix, _, cx| {
                    let kind = if *ix == 1 {
                        CustomKind::Weapon
                    } else {
                        CustomKind::Character
                    };
                    form.update(cx, |form, cx| form.set_kind(kind, cx)).ok();
                })
        });
        // Each role takes three rows of its column: its name, its tile, the
        // material chosen. A name that wraps ("Matériau hebdomadaire") makes
        // its whole row taller, so the tiles beside it stay level.
        let slots = self
            .kind
            .roles()
            .iter()
            .enumerate()
            .flat_map(|(ix, &role)| {
                let column = i16::try_from(ix % SLOT_COLUMNS).unwrap_or(0) + 1;
                let row = i16::try_from(ix / SLOT_COLUMNS).unwrap_or(0) * 3 + 1;
                let cell = |div: Div, row: i16| {
                    (div.col_start(column).col_end(column + 1))
                        .row_start(row)
                        .row_end(row + 1)
                };
                let chosen = self.materials.get(&role).map(|item| self.shown(role, item));
                let tile = match &chosen {
                    Some((icon, stars, _)) => div().child(MaterialTile::bare(icon.clone(), *stars)),
                    None => div()
                        .size(rems(3.5))
                        .rounded(theme.radius)
                        .border_2()
                        .border_dashed()
                        .border_color(theme.border)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Icon::new(IconName::Plus)
                                .size_5()
                                .text_color(theme.muted_foreground),
                        ),
                };
                [
                    // The slot's surface, under its three cells, which take no
                    // pointer: hovering or clicking any of them is the slot's.
                    div()
                        .id(SharedString::from(format!("slot-{role}")))
                        .col_start(column)
                        .col_end(column + 1)
                        .row_start(row)
                        .row_end(row + 3)
                        .rounded(theme.radius_lg)
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.muted))
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.pick(role, window, cx)),
                        )
                        .into_any_element(),
                    // A name on one line sits on the tile, as one on two does.
                    cell(div(), row)
                        .v_flex()
                        .justify_end()
                        .px_2()
                        .pt_2()
                        .text_center()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(role.label())
                        .into_any_element(),
                    cell(div(), row + 1)
                        .flex()
                        .justify_center()
                        .child(tile)
                        .into_any_element(),
                    cell(div(), row + 2)
                        .px_2()
                        .pb_2()
                        .text_xs()
                        .text_center()
                        .text_color(theme.muted_foreground)
                        .line_clamp(2)
                        .text_ellipsis()
                        .child(chosen.map_or_else(|| t!("planner-choose"), |(_, _, name)| name))
                        .into_any_element(),
                ]
            });
        div()
            .v_flex()
            .gap_4()
            .children(kinds.map(|kinds| div().h_flex().child(kinds)))
            .child(input(&self.name, cx))
            .child(
                div()
                    .h_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(t!("planner-stars")),
                    )
                    .child(stars),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(SLOT_COLUMNS as u16)
                    .gap_x_2()
                    .gap_y_1p5()
                    .children(slots),
            )
    }

    fn render_picker(&self, role: Role, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let query = self.search.read(cx).value().trim().to_lowercase();
        let chosen = self.materials.get(&role).cloned();
        let tiles = self
            .planner
            .families(role)
            .iter()
            .filter_map(|tiers| {
                let first = tiers[0];
                let (icon, stars, name) = self.shown(role, &CustomItem::Material(first));
                // Any tier's name finds the series.
                let matches = query.is_empty()
                    || tiers.iter().any(|id| {
                        self.planner
                            .material(*id)
                            .is_some_and(|m| m.name.to_lowercase().contains(&query))
                    });
                matches.then_some((first, icon, stars, name))
            })
            .map(|(first, icon, stars, name): (MaterialId, _, _, String)| {
                let selected = chosen == Some(CustomItem::Material(first));
                let tip = SharedString::from(name);
                div()
                    .id(SharedString::from(format!("pick-{first}")))
                    .rounded(theme.radius)
                    .border_2()
                    .border_color(if selected {
                        theme.primary
                    } else {
                        gpui_kit::transparent_black()
                    })
                    .cursor_pointer()
                    .hover(|style| style.opacity(0.8))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.choose(role, CustomItem::Material(first), cx)
                    }))
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
                    })
                    .child(MaterialTile::bare(icon, stars))
            })
            .collect::<Vec<_>>();
        let empty = tiles.is_empty();
        div()
            .v_flex()
            .gap_3()
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(
                        icon_button("picker-back", IconName::ArrowLeft, t!("planner-back"))
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.picking = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(t!("planner-choose-the", role = role.to_string())),
                    ),
            )
            .child(input(&self.search, cx))
            .child(
                div()
                    .id("picker-grid")
                    .h(rems(18.))
                    .overflow_y_scrollbar()
                    .child(div().h_flex().flex_wrap().gap_1().children(tiles).when(
                        empty,
                        |this| {
                            this.child(
                                div()
                                    .p_2()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(t!("planner-nothing-in-data")),
                            )
                        },
                    )),
            )
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(div().flex_1().child(input(&self.other, cx)))
                    .child(
                        button("use-other")
                            .label(t!("planner-use-name"))
                            .on_click(cx.listener(|this, _, _, cx| this.choose_other(cx))),
                    ),
            )
    }
}

impl Render for CustomForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let error = self.error.clone();
        let body = match self.picking {
            Some(role) => self.render_picker(role, cx),
            None => self.render_form(cx),
        };
        let danger = cx.theme().danger;
        body.when_some(error, |this, error| {
            this.child(div().text_sm().text_color(danger).child(error))
        })
    }
}

impl PlannerView {
    /// Opens the dialog for a new custom character or weapon, or to edit
    /// `entry`, and gives its form.
    pub(super) fn open_custom(
        &mut self,
        entry: Option<PlanEntry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<CustomForm> {
        let library = self.library.read(cx);
        let (planner, images) = (library.planner().clone(), library.images(cx).clone());
        let form = cx.new(|cx| {
            CustomForm::new(
                entry.as_ref().map(|e| &e.target),
                planner,
                images,
                window,
                cx,
            )
        });
        let view = cx.entity().downgrade();
        let title = match entry.as_ref().map(|e| &e.target) {
            Some(PlanTarget::CustomWeapon { .. }) => t!("planner-edit-weapon-title"),
            Some(_) => t!("planner-edit-character-title"),
            None => t!("planner-add-custom-title"),
        };
        // An edit keeps the goal its sliders show.
        let edited = entry.map(|entry| {
            let goal = self.goal(entry.id, cx).or(entry.target.goal().copied());
            let weapon_goal =
                (self.weapon_goal(entry.id, cx)).or(entry.target.weapon_goal().copied());
            (entry.id, goal, weapon_goal)
        });
        let dialog_form = form.clone();
        window.open_dialog(cx, move |dialog, window, _| {
            let (form, view) = (dialog_form.clone(), view.clone());
            form_dialog(dialog)
                .title(title.clone())
                .w(window.rem_size() * 38.)
                .child(form.clone())
                .footer(dialog_footer(button("save-custom").primary().label(
                    match edited {
                        Some(_) => t!("planner-save"),
                        None => t!("planner-add"),
                    },
                )))
                .on_ok(move |_, _, cx| {
                    let Some((kind, name, stars, materials)) =
                        form.update(cx, |form, cx| form.submit(cx))
                    else {
                        // Keep the dialog open on the error.
                        return false;
                    };
                    let target = match kind {
                        CustomKind::Character => PlanTarget::CustomCharacter {
                            name,
                            stars,
                            materials,
                            goal: (edited.and_then(|(_, goal, _)| goal))
                                .unwrap_or(CharacterGoal::FULL),
                        },
                        CustomKind::Weapon => PlanTarget::CustomWeapon {
                            name,
                            stars,
                            materials,
                            goal: (edited.and_then(|(_, _, goal)| goal))
                                .unwrap_or_else(|| WeaponGoal::full(stars)),
                        },
                    };
                    view.update(cx, |view, cx| {
                        let account = view.account(cx);
                        let task = view.library.update(cx, |library, cx| match edited {
                            Some((id, _, _)) => {
                                let update = library.update_plan_entry(
                                    account,
                                    PlanEntry { id, target },
                                    cx,
                                );
                                cx.background_spawn(async move { update.await.map(|()| id) })
                            }
                            None => library.add_plan_entry(account, target, cx),
                        });
                        report_plan(task, cx);
                    })
                    .ok();
                    true
                })
        });
        form
    }
}
