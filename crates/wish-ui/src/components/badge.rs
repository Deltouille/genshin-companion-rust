use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, Div, FontWeight, Hsla, ParentElement as _, SharedString, Styled as _, div};

/// A small rounded label in a soft colour pair `(text, fill)`, as made by
/// [`crate::theme::soft`].
pub fn badge(text: impl Into<SharedString>, (color, fill): (Hsla, Hsla), cx: &App) -> Div {
    div()
        .px_2()
        .py_0p5()
        .rounded(cx.theme().radius)
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .bg(fill)
        .child(text.into())
}

/// A wish's 5★ or 4★ rate-up result as a soft badge: won green, guaranteed
/// gold (the rarity colour), lost red.
pub fn outcome_badge(outcome: wish_core::engine::RateUpOutcome, cx: &App) -> Div {
    use wish_core::engine::RateUpOutcome;
    use wish_core::model::Rarity;
    let theme = cx.theme();
    let colors = match outcome {
        RateUpOutcome::Won => crate::theme::soft(theme.green),
        RateUpOutcome::Guaranteed => crate::theme::soft(crate::theme::rarity(Rarity::Five, cx)),
        RateUpOutcome::Lost | RateUpOutcome::LostWhileGuaranteed => crate::theme::soft(theme.red),
    };
    badge(crate::format::outcome(outcome), colors, cx)
}
