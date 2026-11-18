//! Archive → Resources: every material of the game data, by type as the
//! game's bag sorts them (Mora and EXP, gems, boss and weekly boss materials,
//! talent materials, weapon materials, common and elite enemy drops, each
//! region's local specialties), each a card like the characters' and weapons'
//! with how many the selected account has.
//! A search and "Owned only" filter them; a click opens the planner's
//! inventory dialog on its series, where the count is set (the shell gives
//! the way to it: pages don't know each other).
//!
//! An entity: the search and the filter are its own state. What it shows is
//! `model.rs`'s, worked out from the [`Library`]'s inventory of the account
//! the [`Session`] has selected.

mod model;

use std::rc::Rc;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, AppContext as _, Context, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div, rems,
};
use wish_core::planner::MaterialId;
use wish_ui::{PageFrame, filter_chip, format, input};

use super::shared::{ItemCard, item_card, section_title};
use crate::app::{Library, Session};

/// Opens the inventory dialog on a material.
pub type EditMaterial = Rc<dyn Fn(MaterialId, &mut Window, &mut App)>;

/// Archive → Resources: every material by type, with how many the account has,
/// a search and "Owned only".
pub struct ResourcesView {
    session: Entity<Session>,
    library: Entity<Library>,
    search: Entity<InputState>,
    /// Only the materials the account has some of.
    owned_only: bool,
    edit: EditMaterial,
    _subscriptions: Vec<Subscription>,
}

impl ResourcesView {
    pub fn new(
        session: Entity<Session>,
        edit: EditMaterial,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let library = session.read(cx).library().clone();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(t!("resources-search")));
        let subscriptions = vec![
            // New counts, or a change of language among others.
            cx.observe_in(&library, window, |this, _, window, cx| {
                this.search.update(cx, |search, cx| {
                    search.set_placeholder(t!("resources-search"), window, cx)
                });
                cx.notify();
            }),
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        Self {
            session,
            library,
            search,
            owned_only: false,
            edit,
            _subscriptions: subscriptions,
        }
    }
}

impl Render for ResourcesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let account = self.session.read(cx).account();
        let library = self.library.read(cx);
        let (planner, images) = (library.planner().clone(), library.images(cx).clone());
        let query = self.search.read(cx).value().to_string();
        let page = model::resources(
            &planner,
            library.inventory(account),
            &query,
            self.owned_only,
        );
        let theme = cx.theme();
        let muted = theme.muted_foreground;

        let owned_only = filter_chip(
            "owned-only",
            t!("resources-owned-only"),
            theme.primary,
            self.owned_only,
            cx,
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.owned_only = !this.owned_only;
            cx.notify();
        }));
        let toolbar = div()
            .h_flex()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .min_w(rems(12.))
                    .child(input(&self.search, cx)),
            )
            .child(owned_only);

        let sections = page.sections.iter().map(|section| {
            let groups = section.groups.iter().map(|group| {
                let cards = group.tiles.iter().map(|tile| {
                    let edit = self.edit.clone();
                    let id = tile.id;
                    let owned = tile.owned > 0;
                    item_card(
                        ItemCard {
                            id: SharedString::from(format!("resource-{id}")),
                            icon: planner.material(id).and_then(|m| images.material(m)),
                            rarity: tile.stars,
                            owned,
                            name: tile.name.clone(),
                            label: tile.tip.clone(),
                            top_left: None,
                            corner: None,
                            summary: owned.then(|| format::count(tile.owned)),
                            new: false,
                        },
                        cx,
                    )
                    // The whole name, for one the strip cuts.
                    .tooltip(tile.tip.clone())
                    .on_click(move |_, window, cx| edit(id, window, cx))
                });
                div()
                    .v_flex()
                    .gap_2()
                    .when_some(group.title.clone(), |this, title| {
                        this.child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(muted)
                                .child(title),
                        )
                    })
                    .child(div().h_flex().flex_wrap().gap_5().children(cards))
            });
            div()
                .v_flex()
                .gap_3()
                .child(section_title(section.title.clone(), section.count, cx))
                .children(groups)
        });
        let empty = page.sections.is_empty().then(|| {
            div()
                .text_sm()
                .text_color(muted)
                .child(if self.owned_only && query.trim().is_empty() {
                    t!("resources-none-owned")
                } else {
                    t!("resources-no-match")
                })
        });
        let body = div()
            .id("resources")
            .size_full()
            .overflow_y_scrollbar()
            .child(
                div()
                    .v_flex()
                    .gap_6()
                    .px_6()
                    .pt_2()
                    .pb_6()
                    .children(sections)
                    .children(empty),
            );
        PageFrame::new(t!("resources-title"), body)
            .subtitle(t!(
                "resources-subtitle",
                owned = page.owned,
                total = page.total
            ))
            .toolbar(toolbar)
            .body_scrolls()
    }
}
