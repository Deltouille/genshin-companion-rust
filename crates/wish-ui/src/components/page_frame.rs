use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Rems, RenderOnce, SharedString, Styled as _, Window, div,
};

/// Page layout shared by every page: a title row, then the content below it.
/// The title stands alone, without an icon (the user's choice).
#[derive(IntoElement)]
pub struct PageFrame {
    title: SharedString,
    subtitle: Option<SharedString>,
    actions: Vec<AnyElement>,
    toolbar: Option<AnyElement>,
    body: AnyElement,
    /// The body manages its own scrolling (a table), rather than the page.
    body_scrolls: bool,
    /// The title and the body in a column this wide at most, centred.
    max_width: Option<Rems>,
}

impl PageFrame {
    pub fn new(title: impl Into<SharedString>, body: impl IntoElement) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            actions: Vec::new(),
            toolbar: None,
            body: body.into_any_element(),
            body_scrolls: false,
            max_width: None,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// A control at the end of the title row (several stack in order).
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// A row of controls under the title.
    pub fn toolbar(mut self, toolbar: impl IntoElement) -> Self {
        self.toolbar = Some(toolbar.into_any_element());
        self
    }

    /// The body manages its own scrolling (a table) rather than the page.
    pub fn body_scrolls(mut self) -> Self {
        self.body_scrolls = true;
        self
    }

    /// Keeps the title and the body in a centred column at most `width`
    /// wide, for a page of settings or cards that would otherwise hug the
    /// left of a wide window.
    pub fn max_width(mut self, width: Rems) -> Self {
        self.max_width = Some(width);
        self
    }
}

impl RenderOnce for PageFrame {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let column = |content: Div| match self.max_width {
            Some(width) => content.w_full().max_w(width).mx_auto(),
            None => content,
        };
        let header = div().px_6().pt_5().pb_2().child(column(
            div()
                .v_flex()
                .gap_3()
                .child(
                    div()
                        .h_flex()
                        .gap_4()
                        .child(
                            div()
                                .v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_1()
                                .child(
                                    div()
                                        .text_2xl()
                                        .font_weight(FontWeight::BOLD)
                                        .child(self.title),
                                )
                                .when_some(self.subtitle, |this, subtitle| {
                                    this.child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.muted_foreground)
                                            .child(subtitle),
                                    )
                                }),
                        )
                        .children(self.actions),
                )
                .children(self.toolbar),
        ));

        let body = if self.body_scrolls {
            div().flex_1().min_h_0().child(self.body).into_any_element()
        } else {
            div()
                .id("page-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .child(div().px_6().py_5().child(column(div().child(self.body))))
                .into_any_element()
        };

        div().v_flex().size_full().child(header).child(body)
    }
}
