//! The planner's "Add a character" and "Add a weapon": a button opens a
//! dialog of cards, as the archive draws them (the picture on its rarity
//! tile over a parchment strip), with a search. What the account owns comes
//! first, with its level and constellation (or refinement); a character
//! already in the plan is faded and can't be added twice. Clicking a card
//! adds it and closes the dialog: a character from the levels the account
//! has, to 90/90 with its talents to 10; a weapon from level 1 to its
//! rarity's top.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FontWeight, ImageSource,
    InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Subscription, WeakEntity,
    Window, div, img, rems,
};
use wish_core::planner::PlanTarget;

use super::PlannerView;
use crate::app::{Library, Session};
use wish_ui::{badge, input, theme};

/// A card's width (and its picture's height), in rem.
const CARD_REMS: f32 = 6.;
/// Cards a row.
const COLUMNS: u16 = 7;

/// What the dialog adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Character,
    Weapon,
}

/// The dialog's content: the search, then the cards.
pub(super) struct AddDialog {
    kind: Kind,
    library: Entity<Library>,
    session: Entity<Session>,
    view: WeakEntity<PlannerView>,
    search: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

/// One card's content.
struct Card {
    key: String,
    name: String,
    stars: u8,
    icon: Option<ImageSource>,
    /// The element's icon, at the top left (characters).
    element: Option<ImageSource>,
    /// What the account has of it: "Lv. 80 · C2", "R2".
    owned: Option<String>,
    /// A character already in the plan: faded, not clickable.
    planned: bool,
}

impl AddDialog {
    fn new(
        kind: Kind,
        view: WeakEntity<PlannerView>,
        library: Entity<Library>,
        session: Entity<Session>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(match kind {
                Kind::Character => "Search a character",
                Kind::Weapon => "Search a weapon",
            })
        });
        let subscriptions = vec![cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })];
        Self {
            kind,
            library,
            session,
            view,
            search,
            _subscriptions: subscriptions,
        }
    }

    /// The cards, what the account owns first; each list by name (weapons
    /// by rarity first), narrowed by the search.
    fn cards(&self, cx: &App) -> (Vec<Card>, Vec<Card>) {
        let library = self.library.read(cx);
        let account = self.session.read(cx).account();
        let (planner, images) = (library.planner(), library.images());
        let owned = library.owned(account);
        let plan = library.plan(account);
        let query = self.search.read(cx).value().trim().to_lowercase();
        let matches = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
        let mut cards: Vec<Card> = match self.kind {
            Kind::Character => planner
                .characters()
                .iter()
                .filter(|c| matches(&c.name))
                .map(|c| {
                    let mine = owned.character(&c.key);
                    Card {
                        key: c.key.clone(),
                        name: c.name.clone(),
                        stars: c.rarity,
                        icon: images.character(c),
                        element: images.element(&c.element),
                        owned: mine.map(|m| format!("Lv. {} · C{}", m.level.level, m.constellation)),
                        planned: plan.iter().any(|entry| {
                            matches!(&entry.target, PlanTarget::Character { key, .. } if *key == c.key)
                        }),
                    }
                })
                .collect(),
            Kind::Weapon => planner
                .weapons()
                .iter()
                .filter(|w| matches(&w.name))
                .map(|w| {
                    let best = owned.copies(&w.key).map(|copy| copy.refinement).max();
                    Card {
                        key: w.key.clone(),
                        name: w.name.clone(),
                        stars: w.rarity,
                        icon: images.weapon(w),
                        element: None,
                        owned: best.map(|r| format!("R{r}")),
                        // A weapon may be planned twice: two characters can
                        // use one each.
                        planned: false,
                    }
                })
                .collect(),
        };
        cards.sort_by(|a, b| {
            let rarity = |card: &Card| match self.kind {
                Kind::Character => 0,
                Kind::Weapon => u8::MAX - card.stars,
            };
            (rarity(a), &a.name).cmp(&(rarity(b), &b.name))
        });
        cards.into_iter().partition(|card| card.owned.is_some())
    }

    /// Adds the card's character or weapon, then closes the dialog.
    fn pick(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        let kind = self.kind;
        self.view
            .update(cx, |view, cx| match kind {
                Kind::Character => view.add_character(key, window, cx),
                Kind::Weapon => view.add_weapon(key, window, cx),
            })
            .ok();
        window.close_dialog(cx);
    }

    fn card(&self, card: Card, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (ink, parchment) = theme::tile_label(cx);
        let owned = card.owned.is_some();
        let picture = div()
            .relative()
            .size(rems(CARD_REMS))
            .rounded_t(theme.radius_lg)
            .rounded_br(rems(1.5))
            .bg(if owned {
                theme::stars_tile(card.stars, cx)
            } else {
                theme::stars_tile(card.stars, cx).opacity(0.5)
            })
            .children(card.icon.map(|icon| {
                img(icon)
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .when(!owned, |this| this.opacity(0.6))
            }))
            .children(card.element.map(|icon| {
                div()
                    .absolute()
                    .top_1()
                    .left_1()
                    .size_5()
                    .child(img(icon).size_full())
            }))
            .children(card.owned.map(|text| {
                div().absolute().bottom_1().left_1().child(badge(
                    text,
                    (theme.foreground, theme.background.opacity(0.85)),
                    cx,
                ))
            }))
            .when(card.planned, |this| {
                this.child(div().absolute().top_1().right_1().child(badge(
                    "Planned",
                    (theme.primary_foreground, theme.primary),
                    cx,
                )))
            });
        let strip = div()
            .w_full()
            .h(rems(2.5))
            .px_1()
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_center()
            .line_clamp(2)
            .text_color(if owned { ink } else { theme.muted_foreground })
            .child(card.name.clone());
        let key = card.key;
        div()
            .id(SharedString::from(format!("add-{key}")))
            .v_flex()
            .w(rems(CARD_REMS))
            .rounded(theme.radius_lg)
            .bg(if owned { parchment } else { theme.muted })
            .child(picture)
            .child(strip)
            .map(|this| {
                if card.planned {
                    this.opacity(0.45)
                } else {
                    this.cursor_pointer()
                        .hover(|this| this.opacity(0.85))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.pick(key.clone(), window, cx)
                        }))
                }
            })
            .into_any_element()
    }
}

impl Render for AddDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let (owned, others) = self.cards(cx);
        let section = |title: String, cards: Vec<AnyElement>| {
            div()
                .v_flex()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(muted)
                        .child(title),
                )
                .child(div().grid().grid_cols(COLUMNS).gap_3().children(cards))
        };
        let (owned_title, others_title) = match self.kind {
            Kind::Character => ("Owned", "Not owned"),
            Kind::Weapon => ("Owned", "Every weapon"),
        };
        let empty = owned.is_empty() && others.is_empty();
        let (owned_count, others_count) = (owned.len(), others.len());
        let owned: Vec<AnyElement> = owned.into_iter().map(|c| self.card(c, cx)).collect();
        let others: Vec<AnyElement> = others.into_iter().map(|c| self.card(c, cx)).collect();
        // The list scrolls within the dialog, which keeps to the window.
        let room = window.viewport_size().height * 0.6;
        div().v_flex().gap_3().child(input(&self.search, cx)).child(
            div()
                .id("add-cards")
                .max_h(room)
                .overflow_y_scrollbar()
                .child(
                    div()
                        .v_flex()
                        .gap_4()
                        .pr_2()
                        .when(owned_count > 0, |this| {
                            this.child(section(format!("{owned_title} · {owned_count}"), owned))
                        })
                        .when(others_count > 0, |this| {
                            this.child(section(format!("{others_title} · {others_count}"), others))
                        })
                        .when(empty, |this| {
                            this.child(
                                div()
                                    .text_sm()
                                    .text_color(muted)
                                    .child("Nothing matches this search."),
                            )
                        }),
                ),
        )
    }
}

impl PlannerView {
    /// Opens the dialog that adds a character or a weapon.
    pub(crate) fn open_add(&mut self, kind: Kind, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let (library, session) = (self.library.clone(), self.session.clone());
        let dialog = cx.new(|cx| AddDialog::new(kind, view, library, session, window, cx));
        let search = dialog.read(cx).search.clone();
        let focus_once = std::cell::Cell::new(true);
        // Seven cards, their gaps and the dialog's padding.
        let width = CARD_REMS * f32::from(COLUMNS) + 0.75 * f32::from(COLUMNS - 1) + 4.5;
        window.open_dialog(cx, move |this, window, cx| {
            if focus_once.replace(false) {
                search.update(cx, |search, cx| search.focus(window, cx));
            }
            this.title(match kind {
                Kind::Character => "Add a character",
                Kind::Weapon => "Add a weapon",
            })
            .w(window.rem_size() * width)
            .child(dialog.clone())
        });
    }
}
