use std::rc::Rc;

use gpui_kit::base::{Selectable as _, StyledExt as _};
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, Size};
use gpui_kit::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, SharedString, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::controls::button;

type SelectHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

/// A choice between a few views or values, one always on: a pill split in
/// segments, 40 px high, the current option filled with the primary colour
/// (parchment in the dark theme, ink in the light one), as the game marks a tab.
#[derive(IntoElement)]
pub struct Segmented {
    id: ElementId,
    options: Vec<SharedString>,
    selected: usize,
    on_select: Option<SelectHandler>,
}

impl Segmented {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            options: Vec::new(),
            selected: 0,
            on_select: None,
        }
    }

    pub fn option(mut self, label: impl Into<SharedString>) -> Self {
        self.options.push(label.into());
        self
    }

    /// The index of the current option.
    pub fn selected(mut self, ix: usize) -> Self {
        self.selected = ix;
        self
    }

    /// Called with the index of the option clicked, unless it is already the
    /// current one.
    pub fn on_select(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Segmented {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        // Material 3's segmented button: one outlined pill, the segments
        // split by hairlines, the current one filled with the primary colour
        // (the user's choice over M3's tonal fill and tick: the tick made the
        // segment wider when picked). The kit replays the caller's style
        // over its selected state, which also drops the hover layer there.
        // A pill, as the game's tabs and filters: the ends round to half the
        // height.
        let (outline, text, round) = (theme.input, theme.foreground, gpui_kit::rems(1.25));
        let (fill, on_fill) = (theme.primary, theme.primary_foreground);
        let last = self.options.len().saturating_sub(1);
        let mut children: Vec<AnyElement> = Vec::new();
        for (ix, label) in self.options.into_iter().enumerate() {
            if ix > 0 {
                // A hairline of its own: the kit repaints a button's border
                // when it is hovered or selected.
                children.push(div().w(px(1.)).h_full().bg(outline).into_any_element());
            }
            let current = ix == self.selected;
            let on_select = self.on_select.clone();
            let segment = button(ix)
                .ghost()
                .with_size(Size::Medium)
                .h_full()
                .font_weight(FontWeight::MEDIUM)
                .label(label)
                .rounded_l(if ix == 0 { round } else { gpui_kit::rems(0.) })
                .rounded_r(if ix == last {
                    round
                } else {
                    gpui_kit::rems(0.)
                })
                .map(|this| {
                    if current {
                        this.selected(true)
                            .bg(fill)
                            .text_color(on_fill)
                            .cursor_default()
                    } else {
                        this.text_color(text)
                    }
                })
                .when_some(on_select.filter(|_| !current), |this, on_select| {
                    this.on_click(move |_, window, cx| on_select(&ix, window, cx))
                });
            children.push(segment.into_any_element());
        }
        div()
            .id(self.id)
            .h_flex()
            .flex_shrink_0()
            .h_10()
            .rounded(round)
            .border_1()
            .border_color(outline)
            .children(children)
    }
}
