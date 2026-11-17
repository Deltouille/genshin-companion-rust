//! The list of characters: the filter bar ([`super::filters`]: search, the
//! filters in a sheet, the order), then two grids, as genshin-tracker
//! lists them: the characters the account owns ("Owned": level and
//! constellation on the card, a star for favourites, × to remove), then the
//! others, dimmed, with + to add them. A card's picture is the icon on its
//! rarity colour, the element in a corner, "New" on the latest release's.

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Context, Entity, FontWeight, ImageSource, InteractiveElement as _,
    IntoElement, ObjectFit, ParentElement as _, SharedString, Styled as _, StyledImage as _,
    WeakEntity, Window, div, img, rems,
};
use wish_core::model::Rarity;

use std::rc::Rc;

use super::super::filters::{self, FilterGroup, FilterOption};
use super::super::shared::{corner_button, section_title};
use super::CharactersView;
use super::model::{self, CharacterCard, Filters};
use crate::app::images::Images;
use wish_core::planner::PlannerData;
use wish_ui::format;
use wish_ui::{PageFrame, badge, theme};

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
            library.images(cx).clone(),
        );
        let owned = library.owned(account).clone();
        let warning = library.game_data_warning().cloned();
        let query = self.search.read(cx).value().to_lowercase();
        let filters = Filters {
            query: &query,
            elements: &self.elements,
            weapons: &self.weapons,
            stars: &self.stars,
            favourites: self.favourites,
            sort: self.sort,
            reversed: self.reversed,
        };
        let filtered = filters.any();
        let list = model::list(&planner, &archive, &owned, &filters);
        let mine_cards: Vec<AnyElement> = (list.mine.iter())
            .map(|card| self.card(card, &planner, &images, cx))
            .collect();
        let other_cards: Vec<AnyElement> = (list.others.iter())
            .map(|card| self.card(card, &planner, &images, cx))
            .collect();

        let theme = cx.theme();
        let view = cx.entity();
        let groups = self.filter_groups(view.downgrade(), cx);
        let clear = Self::clear_filters(&view);
        let toolbar = filters::filter_bar(
            &self.search,
            &self.sort_select,
            self.reversed,
            &groups,
            cx.listener(|this, _, window, cx| this.open_filters(window, cx)),
            cx.listener(|this, _, _, cx| {
                this.reversed = !this.reversed;
                cx.notify();
            }),
            clear,
            cx,
        );

        let (mine_count, other_count) = (mine_cards.len(), other_cards.len());
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
                            .child(t!("archive-built-in-data", warning = warning.to_string()))
                    }))
                    .child(section_title(t!("archive-owned"), mine_count, cx))
                    .when(mine_count == 0, |this| {
                        this.child(div().text_sm().text_color(muted).child(if filtered {
                            t!("archive-no-owned-character-match")
                        } else {
                            t!("archive-owned-characters-help")
                        }))
                    })
                    .child(div().h_flex().flex_wrap().gap_5().children(mine_cards))
                    .when(other_count > 0 || mine_count == 0, |this| {
                        this.child(div().pt_2().child(section_title(
                            t!("archive-not-owned"),
                            other_count,
                            cx,
                        )))
                    })
                    .child(div().h_flex().flex_wrap().gap_5().children(other_cards))
                    .when(mine_count + other_count == 0, |this| {
                        this.child(
                            div()
                                .text_color(muted)
                                .child(t!("archive-no-character-match")),
                        )
                    }),
            );
        let page = PageFrame::new(t!("archive-characters"), body)
            .subtitle(t!(
                "archive-characters-subtitle",
                owned = list.owned,
                total = list.total
            ))
            .action(crate::features::imports::hoyolab_sync_button())
            .toolbar(toolbar)
            .body_scrolls();
        let drawer = (self.drawer != filters::Drawer::Closed).then(|| {
            let weak = view.downgrade();
            let close: filters::Handler = Rc::new(move |_, cx| {
                weak.update(cx, |this, cx| this.close_filters(cx)).ok();
            });
            filters::filter_drawer(
                self.drawer,
                &self.drawer_focus,
                self.filter_groups(view.downgrade(), cx),
                close,
                Self::clear_filters(&view),
                cx,
            )
        });
        div()
            .relative()
            .size_full()
            .child(page)
            .children(drawer)
            .into_any_element()
    }

    /// The list's filters, grouped as the sheet shows them: elements (with
    /// their icons), weapons, rarities, favourites.
    fn filter_groups(&self, view: WeakEntity<Self>, cx: &App) -> Vec<FilterGroup> {
        let this = self;
        let images = this.library.read(cx).images(cx).clone();
        let weak = view;
        let toggle = |change: Rc<dyn Fn(&mut Self)>| -> filters::Handler {
            let weak = weak.clone();
            Rc::new(move |_, cx| {
                weak.update(cx, |this, cx| {
                    change(this);
                    cx.notify();
                })
                .ok();
            })
        };
        let elements = ELEMENTS
            .iter()
            .map(|element| FilterOption {
                label: format::element(element).into(),
                icon: images.element(element),
                on: this.elements.contains(element),
                toggle: toggle(Rc::new(move |this| {
                    Self::toggle(&mut this.elements, *element)
                })),
            })
            .collect();
        let weapons = WEAPONS
            .iter()
            .map(|weapon| FilterOption {
                label: format::weapon_type(weapon).into(),
                icon: None,
                on: this.weapons.contains(weapon),
                toggle: toggle(Rc::new(move |this| {
                    Self::toggle(&mut this.weapons, *weapon)
                })),
            })
            .collect();
        let stars = [5u8, 4]
            .into_iter()
            .map(|stars| FilterOption {
                label: format!("{stars}★").into(),
                icon: None,
                on: this.stars.contains(&stars),
                toggle: toggle(Rc::new(move |this| Self::toggle(&mut this.stars, stars))),
            })
            .collect();
        let collection = vec![FilterOption {
            label: t!("archive-favourites").into(),
            icon: None,
            on: this.favourites,
            toggle: toggle(Rc::new(|this| this.favourites = !this.favourites)),
        }];
        vec![
            FilterGroup {
                title: t!("archive-group-element"),
                options: elements,
            },
            FilterGroup {
                title: t!("archive-group-weapon"),
                options: weapons,
            },
            FilterGroup {
                title: t!("archive-group-rarity"),
                options: stars,
            },
            FilterGroup {
                title: t!("archive-group-collection"),
                options: collection,
            },
        ]
    }

    /// Turns every filter off.
    fn clear_filters(view: &Entity<Self>) -> filters::Handler {
        let weak = view.downgrade();
        Rc::new(move |_, cx| {
            weak.update(cx, |this, cx| {
                this.elements.clear();
                this.weapons.clear();
                this.stars.clear();
                this.favourites = false;
                cx.notify();
            })
            .ok();
        })
    }

    /// Opens the filter drawer, with the keyboard in it (Escape closes it).
    pub(crate) fn open_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.drawer = filters::Drawer::Open;
        self.drawer_focus.focus(window, cx);
        cx.notify();
    }

    /// Slides the filter drawer out, then forgets it.
    fn close_filters(&mut self, cx: &mut Context<Self>) {
        if self.drawer != filters::Drawer::Open {
            return;
        }
        self.drawer = filters::Drawer::Closing;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(filters::DRAWER_SLIDE).await;
            this.update(cx, |this, cx| {
                if this.drawer == filters::Drawer::Closing {
                    this.drawer = filters::Drawer::Closed;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// A character's card: its picture with what the account has on it, its
    /// name; the corner buttons add, remove or mark it as a favourite.
    fn card(
        &self,
        character: &CharacterCard,
        planner: &PlannerData,
        images: &Images,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let (ink, parchment) = theme::tile_label(cx);
        let (key, name) = (character.key.clone(), character.name.clone());
        let owned = character.owned.clone();
        let corner = match &owned {
            Some((_, favourite)) => {
                let favourite = *favourite;
                let gold = theme::rarity(Rarity::Five, cx);
                let star = corner_button(
                    format!("favourite-{key}"),
                    if favourite {
                        IconName::StarFill
                    } else {
                        IconName::Star
                    },
                    if favourite {
                        t!("archive-remove-favourite")
                    } else {
                        t!("archive-add-favourite")
                    },
                    cx,
                )
                .when(favourite, |this| this.text_color(gold))
                .on_click(cx.listener({
                    let key = key.clone();
                    move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.toggle_favourite(&key, cx);
                    }
                }));
                let remove = corner_button(
                    format!("disown-{key}"),
                    IconName::Close,
                    t!("archive-remove"),
                    cx,
                )
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
                    t!("archive-add-character"),
                    cx,
                )
                .on_click(cx.listener({
                    let (key, name) = (key.clone(), name.clone());
                    move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.own(key.clone(), name.clone(), cx);
                    }
                })),
            ),
        };
        let icon = planner.character(&key).and_then(|c| images.character(c));
        let picture = div()
            .relative()
            .w_full()
            .h(rems(8.25))
            .rounded_t(theme.radius_lg)
            .rounded_br(rems(2.))
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
                    .children(owned.as_ref().map(|(summary, _)| {
                        badge(
                            summary.clone(),
                            (theme.foreground, theme.background.opacity(0.85)),
                            cx,
                        )
                    }))
                    .child(div().flex_1())
                    .when(character.new, |this| {
                        this.child(badge(
                            t!("archive-new"),
                            (theme.primary_foreground, theme.primary),
                            cx,
                        ))
                    }),
            );
        Button::new(SharedString::from(format!("character-{key}")))
            .ghost()
            .p_0()
            .w(rems(8.25))
            .h_auto()
            .rounded(theme.radius_lg)
            .cursor_pointer()
            .accessibility_label(character.label())
            .child(
                div()
                    .v_flex()
                    .w_full()
                    .rounded(theme.radius_lg)
                    .bg(if owned.is_some() {
                        parchment
                    } else {
                        // Not in the collection: a slate strip, the name muted.
                        theme.muted
                    })
                    .child(picture)
                    .child(
                        div()
                            .w_full()
                            .px_2()
                            .py_1p5()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_center()
                            .truncate()
                            .text_color(if owned.is_some() {
                                ink
                            } else {
                                theme.muted_foreground
                            })
                            .child(name),
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
