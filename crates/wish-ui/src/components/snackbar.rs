use std::time::Duration;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, IconName};
use gpui_kit::{
    Animation, AnimationExt as _, App, ClickEvent, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, Styled as _, Window, div, ease_out_quint, px, rems,
};

/// A short message about work done in the background, as Material 3's
/// snackbar: a dark bar (light in the dark theme) under the page, the text,
/// then a button that closes it. It rises into place when it appears.
///
/// The inverse of the page's colours sets it apart from the cards without a
/// colour of its own, so success and failure read from the words.
pub fn snackbar(
    id: impl Into<ElementId>,
    message: impl Into<SharedString>,
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    // Material 3's inverse surface and inverse on-surface: the page's text
    // colour as the fill, its background as the text.
    let (fill, text) = (theme.foreground, theme.background);
    let id = id.into();
    // A state layer of the text colour, as M3's icon buttons have.
    let close = Button::new("close")
        .custom(
            ButtonCustomVariant::new(cx)
                .color(theme.transparent)
                .foreground(text)
                .hover(text.opacity(0.08))
                .active(text.opacity(0.12)),
        )
        .icon(IconName::Close)
        .size_10()
        .rounded_full()
        .flex_none()
        .cursor_pointer()
        .accessibility_label("Close")
        .on_click(on_close);
    div()
        .id(id.clone())
        .h_flex()
        .gap_3()
        .min_h_12()
        .min_w(rems(20.))
        .max_w(rems(36.))
        .pl_4()
        .pr_1()
        .py_1()
        .rounded(px(4.))
        .bg(fill)
        .text_color(text)
        .text_sm()
        .shadow_lg()
        .child(div().flex_1().min_w_0().py_2().child(message.into()))
        .child(close)
        .with_animation(
            id,
            Animation::new(Duration::from_millis(200)).with_easing(ease_out_quint()),
            |this, delta| this.opacity(delta).mt(px(12. * (1. - delta))),
        )
}
