//! One sidebar entry, at the app's proportions.

use super::controls::ButtonExt as _;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::component::button::Button;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, ClickEvent, ElementId, FontWeight, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Window, div, rems,
};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A sidebar destination: a 40 px pill with a 20 px icon and 14 px text, the
/// current one on a lighter panel (`sidebar.primary`). A page
/// inside a [`super::NavGroup`] is [`NavItem::nested`]: no icon, nor any
/// ornament (the user's choice on 2026-11-17), its label indented under the
/// group's, a 36 px row.
///
/// GPUI Kit's `SidebarMenuItem` is a fixed 28 px rectangle, so this builds on
/// its `Button` instead, which keeps keyboard focus, activation and the
/// hover and pressed states.
#[derive(IntoElement)]
#[must_use = "a component draws only once it is a child of an element"]
pub struct NavItem {
    id: ElementId,
    label: SharedString,
    icon: Option<Icon>,
    active: bool,
    on_click: Option<ClickHandler>,
}

impl NavItem {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        icon: impl Into<Icon>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: Some(icon.into()),
            active: false,
            on_click: None,
        }
    }

    /// A page of a feature group, listed under the group's header.
    pub fn nested(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            active: false,
            on_click: None,
        }
    }

    /// Marks the page shown.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for NavItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let label = if self.active {
            t!("ui-current-page", label = self.label.as_ref())
        } else {
            self.label.to_string()
        };
        let nested = self.icon.is_none();
        // The current page on a lighter panel, drawn behind a ghost button so
        // hovering lays the state layer over it.
        let (fill, text) = if self.active {
            (theme.sidebar_primary, theme.sidebar_primary_foreground)
        } else {
            (theme.transparent, theme.sidebar_foreground)
        };
        let icon = self.icon;
        let button = Button::new(self.id)
            .quiet_in(text, cx)
            .w_full()
            .map(|this| if nested { this.h_9() } else { this.h_10() })
            .px_4()
            // A group's page starts under the group's label, so the hierarchy
            // still reads: the group indents its pages by 1 rem, and this
            // padding adds the header's icon and gap.
            .when(nested, |this| this.pl(rems(2.)))
            .rounded_full()
            .cursor_pointer()
            .accessibility_label(label)
            .child(
                div()
                    .w_full()
                    .h_flex()
                    .gap_3()
                    .children(icon.map(|icon| icon.size_5().flex_shrink_0()))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(if self.active {
                                FontWeight::BOLD
                            } else {
                                FontWeight::MEDIUM
                            })
                            .truncate()
                            .child(self.label),
                    ),
            )
            .when_some(self.on_click, |this, on_click| {
                this.on_click(move |event, window, cx| on_click(event, window, cx))
            });
        div().w_full().rounded_full().bg(fill).child(button)
    }
}
