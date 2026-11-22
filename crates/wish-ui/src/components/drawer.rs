//! A drawer: a panel at the right of the page, its whole height, over a veil.
//! It slides in, and out again when closed (its Done button, the cross,
//! Escape, or a click on the veil). The archive's filters and the planner's
//! Battle Pass open in one.
//!
//! The app's own rather than GPUI Kit's `Sheet`, which has no exit animation.
//! It is drawn from its owner's state ([`DrawerState`]), so a click shows at
//! once; the owner closes it in two steps, [`DrawerState::Closing`] then,
//! [`drawer_slide`] later, [`DrawerState::Closed`].

use std::rc::Rc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, App, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Window, deferred, div, ease_out_quint, rems,
};

use super::controls::{ButtonExt as _, icon_button};

/// Whether a drawer shows: sliding in and open, or sliding out before it goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrawerState {
    #[default]
    Closed,
    Open,
    Closing,
}

/// How long a drawer takes to slide in or out.
const SLIDE: Duration = Duration::from_millis(200);

/// How long the drawer takes to slide in or out: no time at all when the
/// system asks for less motion, where it appears and goes at once.
pub fn drawer_slide(cx: &App) -> Duration {
    if cx.reduce_motion() {
        Duration::ZERO
    } else {
        SLIDE
    }
}

/// Above the page, under the dialogs (GPUI Kit's start at 10).
const PRIORITY: usize = 5;

/// What the drawer's buttons and veil call to close it.
pub type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A drawer titled `title`, its content scrolling under the title, a footer
/// at the bottom.
#[derive(IntoElement)]
#[must_use = "a component draws only once it is a child of an element"]
pub struct Drawer {
    id: SharedString,
    state: DrawerState,
    focus: FocusHandle,
    title: SharedString,
    close_label: SharedString,
    on_close: CloseHandler,
    body: Vec<AnyElement>,
    footer: Vec<AnyElement>,
    width: f32,
}

impl Drawer {
    /// `id` names its elements (unique on the page); `focus` takes the
    /// keyboard while it is open, so Escape closes it; `close_label` names
    /// the cross for a screen reader and its tooltip.
    pub fn new(
        id: impl Into<SharedString>,
        state: DrawerState,
        focus: &FocusHandle,
        title: impl Into<SharedString>,
        close_label: impl Into<SharedString>,
        on_close: CloseHandler,
    ) -> Self {
        Self {
            id: id.into(),
            state,
            focus: focus.clone(),
            title: title.into(),
            close_label: close_label.into(),
            on_close,
            body: Vec::new(),
            footer: Vec::new(),
            width: 26.,
        }
    }

    /// Its width in rems (26 by default).
    pub fn width(mut self, rems: f32) -> Self {
        self.width = rems;
        self
    }

    /// Adds `child` to the scrolling content.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.body.push(child.into_any_element());
        self
    }

    /// Adds `child` to the row at the bottom, the drawer's buttons.
    pub fn footer_child(mut self, child: impl IntoElement) -> Self {
        self.footer.push(child.into_any_element());
        self
    }
}

impl RenderOnce for Drawer {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let highlight = crate::theme::palette(cx).highlight;
        let closing = self.state == DrawerState::Closing;
        let id = self.id;
        let width = self.width;
        // Each way its own animation, so closing starts from the open drawer.
        let (slide, fade) = if closing {
            (format!("{id}-out"), format!("{id}-veil-out"))
        } else {
            (format!("{id}-in"), format!("{id}-veil-in"))
        };
        let animation = Animation::new(drawer_slide(cx)).with_easing(ease_out_quint());
        // Without motion, the final state at once.
        let still = cx.reduce_motion();
        let shown = move |delta: f32| {
            let delta = if still { 1. } else { delta };
            if closing { 1. - delta } else { delta }
        };
        let close = self.on_close;
        let veil = div()
            .id(SharedString::from(format!("{id}-veil")))
            .absolute()
            .inset_0()
            .bg(theme.overlay)
            .on_mouse_down(MouseButton::Left, {
                let close = close.clone();
                move |_, window, cx| close(window, cx)
            })
            .with_animation(
                SharedString::from(fade),
                animation.clone(),
                move |this, delta| this.opacity(shown(delta)),
            );
        let panel = div()
            .id(SharedString::from(format!("{id}-panel")))
            .track_focus(&self.focus)
            .absolute()
            .top_0()
            .bottom_0()
            .right_0()
            .w(rems(width))
            .v_flex()
            .bg(theme.group_box)
            .text_color(theme.foreground)
            .border_l_1()
            .border_color(theme.border)
            .shadow_xl()
            // Clicks inside stay inside; Escape closes it.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_key_down({
                let close = close.clone();
                move |event, window, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        close(window, cx);
                    }
                }
            })
            .child(
                div()
                    .h_flex()
                    .px_5()
                    .pt_5()
                    .pb_3()
                    .child(
                        div()
                            .flex_1()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(highlight)
                            .child(self.title),
                    )
                    .child(
                        icon_button(
                            SharedString::from(format!("{id}-close")),
                            IconName::Close,
                            self.close_label,
                        )
                        .quiet(cx)
                        .on_click({
                            let close = close.clone();
                            move |_, window, cx| close(window, cx)
                        }),
                    ),
            )
            .child(
                div()
                    .id(SharedString::from(format!("{id}-body")))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(div().v_flex().gap_6().px_5().pb_4().children(self.body)),
            )
            .when(!self.footer.is_empty(), |this| {
                this.child(
                    div()
                        .h_flex()
                        .gap_2()
                        .px_5()
                        .py_4()
                        .border_t_1()
                        .border_color(theme.border)
                        .children(self.footer),
                )
            })
            .with_animation(SharedString::from(slide), animation, move |this, delta| {
                this.right(rems(-width * (1. - shown(delta))))
            });
        deferred(div().absolute().inset_0().child(veil).child(panel)).with_priority(PRIORITY)
    }
}
