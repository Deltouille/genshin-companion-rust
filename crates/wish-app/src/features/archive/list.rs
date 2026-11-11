//! The list of characters: a search field, filters by element, weapon and
//! rarity, the order, and a grid of cards (the icon on its rarity colour, the
//! element in a corner, "New" on the latest release's characters).

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, ObjectFit,
    ParentElement as _, SharedString, Styled as _, StyledImage as _, Window, div, img, rems,
};
use wish_core::model::Rarity;
use wish_core::planner::Character;

use super::{CharactersView, Sort};
use wish_ui::{PageFrame, Segmented, badge, filter_chip, input, theme};

/// The elements in the game's order, as the filters show them.
const ELEMENTS: [&str; 7] = ["Pyro", "Hydro", "Anemo", "Electro", "Dendro", "Cryo", "Geo"];
const WEAPONS: [&str; 5] = ["Sword", "Claymore", "Polearm", "Bow", "Catalyst"];

impl CharactersView {
    pub(super) fn render_list(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let library = self.library.read(cx);
        let (planner, archive, images) = (
            library.planner().clone(),
            library.archive().clone(),
            library.images().clone(),
        );
        let warning = library.game_data_warning().cloned();
        let version = |key: &str| {
            archive
                .character(key)
                .and_then(|page| page.version.as_deref())
                .map(release)
                .unwrap_or_default()
        };
        let latest = planner.characters().iter().map(|c| version(&c.key)).max();

        let query = self.search.read(cx).value().to_lowercase();
        let mut shown: Vec<&Character> = planner
            .characters()
            .iter()
            .filter(|c| query.is_empty() || c.name.to_lowercase().contains(&query))
            .filter(|c| self.elements.is_empty() || self.elements.contains(c.element.as_str()))
            .filter(|c| self.weapons.is_empty() || self.weapons.contains(c.weapon.as_str()))
            .filter(|c| self.stars.is_empty() || self.stars.contains(&c.rarity))
            .collect();
        match self.sort {
            Sort::Newest => shown.sort_by(|a, b| {
                version(&b.key)
                    .cmp(&version(&a.key))
                    .then_with(|| a.name.cmp(&b.name))
            }),
            Sort::Name => shown.sort_by(|a, b| a.name.cmp(&b.name)),
            Sort::Rarity => {
                shown.sort_by(|a, b| b.rarity.cmp(&a.rarity).then_with(|| a.name.cmp(&b.name)))
            }
        }

        let theme = cx.theme();
        let cards = shown.iter().map(|character| {
            let key = character.key.clone();
            let new = latest.is_some() && Some(version(&character.key)) == latest;
            let picture = div()
                .relative()
                .w_full()
                .h(rems(7.5))
                .rounded_t(theme.radius_lg)
                .bg(theme::stars_tile(character.rarity, cx))
                .children(
                    images
                        .character(character)
                        .map(|icon| img(icon).size_full().object_fit(ObjectFit::Contain)),
                )
                .children(images.element(&character.element).map(|icon| {
                    div()
                        .absolute()
                        .top_1()
                        .left_1()
                        .size_6()
                        .child(img(icon).size_full())
                }))
                .when(new, |this| {
                    this.child(div().absolute().top_1().right_1().child(badge(
                        "New",
                        (theme.primary_foreground, theme.primary),
                        cx,
                    )))
                });
            Button::new(SharedString::from(format!("character-{}", character.key)))
                .ghost()
                .p_0()
                .w(rems(7.5))
                .h_auto()
                .rounded(theme.radius_lg)
                .cursor_pointer()
                .accessibility_label(format!(
                    "{}, {}★ {} {}",
                    character.name, character.rarity, character.element, character.weapon
                ))
                .child(
                    div()
                        .v_flex()
                        .w_full()
                        .rounded(theme.radius_lg)
                        .bg(theme.group_box)
                        .child(picture)
                        .child(
                            div()
                                .w_full()
                                .px_2()
                                .py_1p5()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_center()
                                .truncate()
                                .text_color(theme.foreground)
                                .child(character.name.clone()),
                        ),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.show_character(&key, cx)))
        });

        // Filters: the elements as their icons, the weapons and rarities as
        // chips; each one on or off, none on meaning all.
        let element_filters = ELEMENTS.map(|element| {
            let on = self.elements.contains(element);
            Button::new(SharedString::from(format!("element-{element}")))
                .ghost()
                .size_11()
                .p_1p5()
                .rounded(theme.radius)
                .cursor_pointer()
                .when(on, |this| this.bg(theme.secondary))
                .toggled(on)
                .tooltip(element)
                .accessibility_label(format!("{element} characters"))
                .children(images.element(element).map(|icon| img(icon).size_full()))
                .when(!on && !self.elements.is_empty(), |this| this.opacity(0.4))
                .on_click(cx.listener(move |this, _, _, cx| {
                    Self::toggle(&mut this.elements, element.into());
                    cx.notify();
                }))
        });
        let weapon_filters = WEAPONS.map(|weapon| {
            let on = self.weapons.contains(weapon);
            filter_chip(
                SharedString::from(format!("weapon-{weapon}")),
                weapon,
                theme.primary,
                on,
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                Self::toggle(&mut this.weapons, weapon.into());
                cx.notify();
            }))
        });
        let star_filters = [5u8, 4].map(|stars| {
            let on = self.stars.contains(&stars);
            filter_chip(
                SharedString::from(format!("stars-{stars}")),
                format!("{stars}★"),
                // The text colours of the rarities (readable on both themes).
                theme::rarity(
                    if stars == 5 {
                        Rarity::Five
                    } else {
                        Rarity::Four
                    },
                    cx,
                ),
                on,
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                Self::toggle(&mut this.stars, stars);
                cx.notify();
            }))
        });
        let sort = Segmented::new("character-sort")
            .option("Newest")
            .option("Name")
            .option("Rarity")
            .selected(match self.sort {
                Sort::Newest => 0,
                Sort::Name => 1,
                Sort::Rarity => 2,
            })
            .on_select(cx.listener(|this, ix: &usize, _, cx| {
                this.sort = match ix {
                    1 => Sort::Name,
                    2 => Sort::Rarity,
                    _ => Sort::Newest,
                };
                cx.notify();
            }));
        let toolbar = div()
            .v_flex()
            .gap_3()
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .gap_4()
                    .child(div().w(rems(18.)).child(input(&self.search, cx)))
                    .child(sort),
            )
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .gap_x_4()
                    .gap_y_2()
                    .child(div().h_flex().gap_1().children(element_filters))
                    .child(div().h_flex().gap_2().children(weapon_filters))
                    .child(div().h_flex().gap_2().children(star_filters)),
            );

        let count = shown.len();
        let body = div()
            .id("character-grid")
            .size_full()
            .overflow_y_scrollbar()
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .px_6()
                    .pt_2()
                    .pb_6()
                    .children(warning.map(|warning| {
                        div()
                            .text_sm()
                            .text_color(theme.warning)
                            .child(format!("Using the built-in game data: {warning}"))
                    }))
                    .child(div().h_flex().flex_wrap().gap_3().children(cards))
                    .when(count == 0, |this| {
                        this.child(
                            div()
                                .text_color(theme.muted_foreground)
                                .child("No character matches these filters."),
                        )
                    }),
            );
        PageFrame::new("Characters", body)
            .subtitle(format!(
                "{count} of {} characters",
                planner.characters().len()
            ))
            .toolbar(toolbar)
            .body_scrolls()
            .into_any_element()
    }
}

/// "4.10" → (4, 10), so versions sort as numbers.
pub(super) fn release(version: &str) -> (u32, u32) {
    let mut parts = version.split('.').map(|p| p.parse().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}
