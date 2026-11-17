//! The weapon list: the filter bar, then the cards of the
//! [`WeaponList`](super::model::WeaponList), "Owned" then "Not owned", and the
//! filter drawer over the page when open.

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, ObjectFit,
    ParentElement as _, SharedString, Styled as _, StyledImage as _, div, img, rems,
};
use wish_core::model::Rarity;

use super::super::filters;
use super::super::shared::{corner_button, section_title};
use super::WeaponsView;
use super::model::{self, Filters, WeaponCard};
use crate::app::images::Images;
use wish_core::planner::PlannerData;
use wish_ui::{PageFrame, badge, theme};

impl WeaponsView {
    pub(super) fn render_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let library = self.library.read(cx);
        let (planner, archive, images) = (
            library.planner().clone(),
            library.archive().clone(),
            library.images(cx).clone(),
        );
        let owned = library.owned(self.account(cx)).clone();
        let query = self.search.read(cx).value().to_lowercase();
        let filters = Filters {
            query: &query,
            types: &self.types,
            stars: &self.stars,
            favourites: self.favourites,
            sort: self.sort,
            reversed: self.reversed,
        };
        let filtered = filters.any();
        let list = model::list(&planner, &archive, &owned, &filters);
        let mine_cards: Vec<AnyElement> = (list.mine.iter())
            .map(|card| self.weapon_card(card, &planner, &images, cx))
            .collect();
        let other_cards: Vec<AnyElement> = (list.others.iter())
            .map(|card| self.weapon_card(card, &planner, &images, cx))
            .collect();
        let theme = cx.theme();

        let view = cx.entity();
        let groups = self.filter_groups(view.downgrade());
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
            Self::clear_filters(&view),
            cx,
        );

        let (mine_count, other_count) = (mine_cards.len(), other_cards.len());
        let muted = theme.muted_foreground;
        let body = div()
            .id("weapon-grid")
            .size_full()
            .overflow_y_scrollbar()
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .px_6()
                    .pt_2()
                    .pb_6()
                    .child(section_title(t!("archive-owned"), mine_count, cx))
                    .when(mine_count == 0, |this| {
                        this.child(div().text_sm().text_color(muted).child(if filtered {
                            t!("archive-no-owned-weapon-match")
                        } else {
                            t!("archive-owned-weapons-help")
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
                        this.child(div().text_color(muted).child(t!("archive-no-weapon-match")))
                    }),
            );
        let page = PageFrame::new(t!("archive-weapons"), body)
            .subtitle(t!(
                "archive-weapons-subtitle",
                owned = list.owned,
                total = list.total
            ))
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
                self.filter_groups(view.downgrade()),
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

    /// A weapon's card: its picture with what the account has of it (the
    /// best refinement, and how many copies), its name; the corner buttons
    /// mark it as a favourite or add a copy.
    fn weapon_card(
        &self,
        weapon: &WeaponCard,
        planner: &PlannerData,
        images: &Images,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let (ink, parchment) = theme::tile_label(cx);
        let (key, name) = (weapon.key.clone(), weapon.name.clone());
        let owned = weapon.owned;
        let add = corner_button(
            format!("add-copy-{key}"),
            IconName::Plus,
            t!("archive-add-copy"),
            cx,
        )
        .on_click(cx.listener({
            let (key, name) = (key.clone(), name.clone());
            move |this, _, _, cx| {
                cx.stop_propagation();
                this.add_copy(key.clone(), name.clone(), cx);
            }
        }));
        let corner = if owned {
            let favourite = weapon.favourite;
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
                    this.toggle_weapon_favourite(&key, cx);
                }
            }));
            div().h_flex().gap_1().child(star).child(add)
        } else {
            div().child(add)
        };
        let icon = planner.weapon(&key).and_then(|w| images.weapon(w));
        let picture = div()
            .relative()
            .w_full()
            .h(rems(8.25))
            .rounded_t(theme.radius_lg)
            .rounded_br(rems(2.))
            .bg(if owned {
                theme::stars_tile(weapon.rarity, cx)
            } else {
                // Faded with its picture: the card reads as one not owned.
                theme::stars_tile(weapon.rarity, cx).opacity(0.4)
            })
            .children(icon.map(|icon| {
                img(icon)
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .when(!owned, |this| this.opacity(0.5))
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
                    .children(weapon.summary.clone().map(|text| {
                        badge(text, (theme.foreground, theme.background.opacity(0.85)), cx)
                    }))
                    .child(div().flex_1())
                    .when(weapon.new, |this| {
                        this.child(badge(
                            t!("archive-new"),
                            (theme.primary_foreground, theme.primary),
                            cx,
                        ))
                    }),
            );
        Button::new(SharedString::from(format!("weapon-{key}")))
            .ghost()
            .p_0()
            .w(rems(8.25))
            .h_auto()
            .rounded(theme.radius_lg)
            .cursor_pointer()
            .accessibility_label(weapon.label())
            .child(
                div()
                    .v_flex()
                    .w_full()
                    .rounded(theme.radius_lg)
                    .bg(if owned { parchment } else { theme.muted })
                    .child(picture)
                    .child(
                        div()
                            .w_full()
                            .px_2()
                            .py_1p5()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_center()
                            .truncate()
                            .text_color(if owned { ink } else { theme.muted_foreground })
                            .child(name),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.show_weapon(&key, cx)))
            .into_any_element()
    }
}
