//! The list of characters: a search field, filters by element, weapon and
//! rarity (and favourites), the order, then two grids, as genshin-tracker
//! lists them: the characters the account owns ("Owned": level and
//! constellation on the card, a star for favourites, × to remove), then the
//! others, dimmed, with + to add them. A card's picture is the icon on its
//! rarity colour, the element in a corner, "New" on the latest release's.

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Context, Div, FontWeight, ImageSource, InteractiveElement as _, IntoElement,
    ObjectFit, ParentElement as _, SharedString, Styled as _, StyledImage as _, Window, div, img,
    rems,
};
use wish_core::collection::OwnedCharacter;
use wish_core::model::Rarity;
use wish_core::planner::Character;

use super::{CharactersView, Sort};
use crate::app::images::Images;
use wish_ui::{PageFrame, Segmented, badge, filter_chip, input, theme};

/// The elements in the game's order, as the filters show them.
const ELEMENTS: [&str; 7] = ["Pyro", "Hydro", "Anemo", "Electro", "Dendro", "Cryo", "Geo"];
const WEAPONS: [&str; 5] = ["Sword", "Claymore", "Polearm", "Bow", "Catalyst"];

impl CharactersView {
    pub(super) fn render_list(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let account = self.account(cx);
        let library = self.library.read(cx);
        let (planner, archive, images) = (
            library.planner().clone(),
            library.archive().clone(),
            library.images().clone(),
        );
        let owned = library.owned(account).clone();
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
            .filter(|c| !self.favourites || owned.character(&c.key).is_some_and(|o| o.favourite))
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
        // What the account owns first, its favourites leading; the others
        // after, each part in the chosen order.
        let (mut mine, others): (Vec<_>, Vec<_>) = shown
            .into_iter()
            .map(|c| (c, owned.character(&c.key).cloned()))
            .partition(|(_, owned)| owned.is_some());
        mine.sort_by_key(|(_, owned)| !owned.as_ref().is_some_and(|o| o.favourite));

        let is_new = |c: &Character| latest.is_some() && Some(version(&c.key)) == latest;
        let mine_cards: Vec<AnyElement> = mine
            .iter()
            .map(|(c, owned)| self.card(c, owned.as_ref(), is_new(c), &images, cx))
            .collect();
        let other_cards: Vec<AnyElement> = others
            .iter()
            .map(|(c, _)| self.card(c, None, is_new(c), &images, cx))
            .collect();

        let theme = cx.theme();
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
        let favourites = filter_chip(
            "favourites",
            "☆ Favourites",
            theme.primary,
            self.favourites,
            cx,
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.favourites = !this.favourites;
            cx.notify();
        }));
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
                    .child(favourites)
                    .child(div().h_flex().gap_1().children(element_filters))
                    .child(div().h_flex().gap_2().children(weapon_filters))
                    .child(div().h_flex().gap_2().children(star_filters)),
            );

        let (mine_count, other_count) = (mine_cards.len(), other_cards.len());
        let filtered = !query.is_empty()
            || !self.elements.is_empty()
            || !self.weapons.is_empty()
            || !self.stars.is_empty()
            || self.favourites;
        let muted = theme.muted_foreground;
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
                    .child(section_title("Owned", mine_count, cx))
                    .when(mine_count == 0, |this| {
                        this.child(div().text_sm().text_color(muted).child(if filtered {
                            "None of your characters matches these filters."
                        } else {
                            "The characters you have show here, with their level and \
                             constellation. Add them with + on their card."
                        }))
                    })
                    .child(div().h_flex().flex_wrap().gap_3().children(mine_cards))
                    .when(other_count > 0 || mine_count == 0, |this| {
                        this.child(
                            div()
                                .pt_2()
                                .child(section_title("Not owned", other_count, cx)),
                        )
                    })
                    .child(div().h_flex().flex_wrap().gap_3().children(other_cards))
                    .when(mine_count + other_count == 0, |this| {
                        this.child(
                            div()
                                .text_color(muted)
                                .child("No character matches these filters."),
                        )
                    }),
            );
        PageFrame::new("Characters", body)
            .subtitle(format!(
                "{} owned · {} characters",
                owned.characters.len(),
                planner.characters().len()
            ))
            .toolbar(toolbar)
            .body_scrolls()
            .into_any_element()
    }

    /// A character's card: its picture with what the account has on it, its
    /// name; the corner buttons add, remove or mark it as a favourite.
    fn card(
        &self,
        character: &Character,
        owned: Option<&OwnedCharacter>,
        new: bool,
        images: &Images,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let (key, name) = (character.key.clone(), character.name.clone());
        let corner = match owned {
            Some(owned) => {
                let gold = theme::rarity(Rarity::Five, cx);
                let star = corner_button(
                    format!("favourite-{key}"),
                    if owned.favourite {
                        IconName::StarFill
                    } else {
                        IconName::Star
                    },
                    if owned.favourite {
                        "Remove from favourites"
                    } else {
                        "Add to favourites"
                    },
                    cx,
                )
                .when(owned.favourite, |this| this.text_color(gold))
                .on_click(cx.listener({
                    let key = key.clone();
                    move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.toggle_favourite(&key, window, cx);
                    }
                }));
                let remove = corner_button(format!("disown-{key}"), IconName::Close, "Remove", cx)
                    .on_click(cx.listener({
                        let (key, name) = (key.clone(), name.clone());
                        move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.confirm_disown(key.clone(), name.clone(), window, cx);
                        }
                    }));
                div().h_flex().gap_1().child(star).child(remove)
            }
            None => div().child(
                corner_button(
                    format!("own-{key}"),
                    IconName::Plus,
                    "Add to my characters",
                    cx,
                )
                .on_click(cx.listener({
                    let (key, name) = (key.clone(), name.clone());
                    move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.own(key.clone(), name.clone(), window, cx);
                    }
                })),
            ),
        };
        let icon = images.character(character);
        let picture = div()
            .relative()
            .w_full()
            .h(rems(7.5))
            .rounded_t(theme.radius_lg)
            .bg(match owned {
                Some(_) => theme::stars_tile(character.rarity, cx),
                // Faded with its picture: the card reads as one not in the collection.
                None => theme::stars_tile(character.rarity, cx).opacity(0.4),
            })
            .children(icon.map(|icon| picture(icon, owned.is_some())))
            .children(images.element(&character.element).map(|icon| {
                div()
                    .absolute()
                    .top_1()
                    .left_1()
                    .size_6()
                    .child(img(icon).size_full())
            }))
            .child(div().absolute().top_1().right_1().child(corner))
            .child(
                div()
                    .absolute()
                    .bottom_1()
                    .left_1()
                    .right_1()
                    .h_flex()
                    .gap_1()
                    .children(owned.map(|owned| {
                        badge(
                            format!("Lv. {} · C{}", owned.level.level, owned.constellation),
                            (theme.foreground, theme.background.opacity(0.85)),
                            cx,
                        )
                    }))
                    .child(div().flex_1())
                    .when(new, |this| {
                        this.child(badge("New", (theme.primary_foreground, theme.primary), cx))
                    }),
            );
        Button::new(SharedString::from(format!("character-{key}")))
            .ghost()
            .p_0()
            .w(rems(7.5))
            .h_auto()
            .rounded(theme.radius_lg)
            .cursor_pointer()
            .accessibility_label(format!(
                "{}, {}★ {} {}{}",
                character.name,
                character.rarity,
                character.element,
                character.weapon,
                if owned.is_some() { ", owned" } else { "" }
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
                            .text_color(if owned.is_some() {
                                theme.foreground
                            } else {
                                theme.muted_foreground
                            })
                            .child(character.name.clone()),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.show_character(&key, cx)))
            .into_any_element()
    }
}

/// The icon on its tile; dimmed for a character the account doesn't own.
fn picture(icon: ImageSource, owned: bool) -> impl IntoElement {
    img(icon)
        .size_full()
        .object_fit(ObjectFit::Contain)
        .when(!owned, |this| this.opacity(0.5))
}

/// A small round button in a card's corner, on a veil of the page's colour
/// so it reads over any picture.
fn corner_button(
    id: impl Into<SharedString>,
    icon: IconName,
    label: &'static str,
    cx: &App,
) -> Button {
    let theme = cx.theme();
    Button::new(gpui_kit::ElementId::Name(id.into()))
        .ghost()
        .small()
        .size_7()
        .rounded_full()
        .bg(theme.background.opacity(0.8))
        .icon(icon)
        .tooltip(label)
        .accessibility_label(label)
        .cursor_pointer()
}

/// A part of the list: its name and how many it holds, as a chip.
fn section_title(title: &str, count: usize, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .h_flex()
        .gap_2()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_owned()),
        )
        .child(badge(
            count.to_string(),
            (theme.secondary_foreground, theme.secondary),
            cx,
        ))
}

/// "4.10" → (4, 10), so versions sort as numbers.
pub(super) fn release(version: &str) -> (u32, u32) {
    let mut parts = version.split('.').map(|p| p.parse().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}
