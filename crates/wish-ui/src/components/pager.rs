use std::ops::Range;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::base::{
    Pagination as PaginationLandmark, PaginationItem, PaginationState, Selectable as _,
    StyledExt as _,
};
use gpui_kit::component::Icon;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::{
    App, ElementId, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window,
    div, prelude::FluentBuilder as _,
};

use super::controls::{ButtonExt as _, button};

type PageHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

/// The most pages an ellipsis lists when opened.
const MAX_HIDDEN_PAGES: usize = 50;

/// Page navigation at the app's size: previous, the page numbers around the
/// current one with the first and last, and next. The current page is filled
/// with the primary colour; an ellipsis opens the pages it hides.
///
/// Pages count from 1. GPUI Kit's `Pagination` only comes in its own
/// sizes, so this builds the same control from its state and landmark.
#[derive(IntoElement)]
pub struct Pager {
    id: ElementId,
    page: usize,
    pages: usize,
    on_change: Option<PageHandler>,
}

impl Pager {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            page: 1,
            pages: 1,
            on_change: None,
        }
    }

    /// The page shown, from 1.
    pub fn page(mut self, page: usize) -> Self {
        self.page = page;
        self
    }

    pub fn pages(mut self, pages: usize) -> Self {
        self.pages = pages;
        self
    }

    /// Called with the page asked for, from 1.
    pub fn on_change(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Pager {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut state = PaginationState::new(self.page, self.pages);
        if let Some(on_change) = self.on_change {
            state = state.on_change(move |page, window, cx| on_change(&page, window, cx));
        }
        let current = state.current_page();
        let square = |id: SharedString| button(id).px_3().min_w_11();
        let request = |state: &PaginationState, page: usize| {
            let state = state.clone();
            move |_: &_, window: &mut Window, cx: &mut App| state.request_page(page, window, cx)
        };

        let step = |id: &'static str, label: &'static str, target: Option<usize>| {
            let icon = if id == "previous" {
                IconName::ChevronLeft
            } else {
                IconName::ChevronRight
            };
            let content = div()
                .h_flex()
                .gap_2()
                .when(id == "previous", |this| this.flex_row_reverse())
                .child(label)
                .child(Icon::new(icon));
            button(id)
                .ghost()
                .child(content)
                .unavailable(target.is_none())
                .when_some(target, |this, page| this.on_click(request(&state, page)))
        };

        let items = state.items().into_iter().map(|item| match item {
            PaginationItem::Page(page) => {
                let this = square(SharedString::from(format!("page-{page}")))
                    .label(page.to_string())
                    .accessibility_label(format!("Page {page}"));
                if page == current {
                    this.primary()
                        .selected(true)
                        .cursor_default()
                        .into_any_element()
                } else {
                    this.ghost()
                        .on_click(request(&state, page))
                        .into_any_element()
                }
            }
            PaginationItem::Ellipsis(hidden) => {
                let state = state.clone();
                let pages = listed(hidden.clone(), current);
                square(SharedString::from(format!(
                    "pages-{}-{}",
                    hidden.start, hidden.end
                )))
                .ghost()
                .label("…")
                .accessibility_label(format!("Pages {} to {}", hidden.start, hidden.end - 1))
                .dropdown_menu(move |mut menu, window, _| {
                    for page in pages.clone() {
                        let state = state.clone();
                        menu =
                            menu.item(PopupMenuItem::new(page.to_string()).on_click(
                                move |_, window, cx| state.request_page(page, window, cx),
                            ));
                    }
                    let rem = window.rem_size();
                    menu.min_w(rem * 4.).max_h(rem * 15.).scrollable(true)
                })
                .into_any_element()
            }
        });

        PaginationLandmark::new(self.id, state.clone())
            .h_flex()
            .items_center()
            .gap_1()
            .child(step("previous", "Previous", state.previous_page()))
            .children(items)
            .child(step("next", "Next", state.next_page()))
    }
}

/// The hidden pages an ellipsis lists: all of them, or the
/// [`MAX_HIDDEN_PAGES`] nearest the current page.
fn listed(hidden: Range<usize>, current: usize) -> Range<usize> {
    if hidden.len() <= MAX_HIDDEN_PAGES {
        hidden
    } else if hidden.start > current {
        hidden.start..hidden.start + MAX_HIDDEN_PAGES
    } else {
        hidden.end - MAX_HIDDEN_PAGES..hidden.end
    }
}
