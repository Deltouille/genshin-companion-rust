use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, ClickEvent, ElementId, FontWeight, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Window, div, rems,
};

use super::NavItem;

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A feature in the sidebar (the wish counter, a planner…): a header as large
/// as a [`NavItem`], with a chevron, and when unfolded the feature's pages
/// ([`NavItem::nested`]) under it, each led by a diamond, as the game lists
/// a menu's sections.
///
/// The header is not filled when one of its pages is shown (the page is);
/// its icon turns gold and its text to full contrast.
#[derive(IntoElement)]
pub struct NavGroup {
    id: ElementId,
    label: SharedString,
    icon: Icon,
    current: bool,
    expanded: bool,
    items: Vec<NavItem>,
    on_click: Option<ClickHandler>,
}

impl NavGroup {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        icon: impl Into<Icon>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: icon.into(),
            current: false,
            expanded: true,
            items: Vec::new(),
            on_click: None,
        }
    }

    /// One of the group's pages is shown.
    pub fn current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }

    /// Whether the pages are listed under the header.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = NavItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Called when the header is clicked (open the feature, or fold it).
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for NavGroup {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (current, expanded) = (self.current, self.expanded);
        let state = if expanded { "expanded" } else { "collapsed" };
        let header = Button::new(self.id)
            .ghost()
            .text_color(if current {
                theme.sidebar_primary_foreground
            } else {
                theme.sidebar_foreground
            })
            .w_full()
            .h(rems(3.5))
            .px_4()
            .rounded_full()
            .cursor_pointer()
            .accessibility_label(format!("{}, {state}", self.label))
            .child(
                div()
                    .w_full()
                    .h_flex()
                    .gap_3()
                    .child(self.icon.size_6().flex_shrink_0().when(current, |icon| {
                        icon.text_color(crate::theme::palette(cx).gold)
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(self.label),
                    )
                    .child(
                        Icon::new(if expanded {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .size_4()
                        .text_color(theme.muted_foreground),
                    ),
            )
            .when_some(self.on_click, |this, on_click| {
                this.on_click(move |event, window, cx| on_click(event, window, cx))
            });

        div()
            .v_flex()
            .gap_1()
            .child(header)
            .when(expanded && !self.items.is_empty(), |this| {
                // Its pages, indented under the header, each led by a diamond.
                this.child(div().v_flex().gap_1().ml_4().children(self.items))
            })
    }
}
