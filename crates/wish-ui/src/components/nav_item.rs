use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, ClickEvent, ElementId, FontWeight, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Window, div, rems,
};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A sidebar destination, as in Material 3's navigation drawer: a 56 px pill
/// with a 24 px icon, the current one on the secondary container. A page
/// inside a [`super::NavGroup`] is [`NavItem::nested`]: no icon, a 48 px
/// row.
///
/// GPUI Kit's `SidebarMenuItem` has a fixed 28 px height, so this builds on
/// its `Button` instead, which keeps keyboard focus, activation and the
/// hover and pressed states.
#[derive(IntoElement)]
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
            format!("{}, current page", self.label)
        } else {
            self.label.to_string()
        };
        let nested = self.icon.is_none();
        let gold = crate::theme::palette(cx).gold;
        // The current page on a lighter panel, drawn behind a ghost button so
        // hovering lays the state layer over it. The pages of a group lead
        // with a diamond, gold for the current one.
        let (fill, text) = if self.active {
            (theme.sidebar_primary, theme.sidebar_primary_foreground)
        } else {
            (theme.transparent, theme.sidebar_foreground)
        };
        let icon = self.icon.or_else(|| {
            Some(
                Icon::new(gpui_kit::assets::IconName::Diamond)
                    .size_3()
                    .text_color(if self.active {
                        gold
                    } else {
                        theme.sidebar_foreground.opacity(0.6)
                    }),
            )
        });
        let button = Button::new(self.id)
            .ghost()
            .text_color(text)
            .w_full()
            .map(|this| {
                if nested {
                    this.h_12()
                } else {
                    this.h(rems(3.5))
                }
            })
            .px_4()
            .rounded_full()
            .cursor_pointer()
            .accessibility_label(label)
            .child(
                div()
                    .w_full()
                    .h_flex()
                    .gap_3()
                    .children(icon.map(|icon| {
                        if nested {
                            icon.flex_shrink_0()
                        } else {
                            icon.size_6().flex_shrink_0()
                        }
                    }))
                    .child(
                        div()
                            .map(|this| {
                                if nested {
                                    this.text_sm()
                                } else {
                                    this.text_base()
                                }
                            })
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
