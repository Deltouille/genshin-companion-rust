//! The app's buttons and selects: pills, as in the game's menus, at the app's size.
//!
//! GPUI Kit's controls are desktop-dense (32 px buttons, 14 px text); next
//! to the 56 px sidebar entries and large figures they look like another
//! app. These builders return the kit's own `Button` (and `Input`),
//! keeping their keyboard, focus and accessibility behaviour, with the
//! app's sizes: use them instead of `Button::new`. For a choice in a list,
//! use [`super::picker`], not the kit's `Select`.
//!
//! They also show the pointing hand over what can be clicked, as a web page
//! does: the kit gives every button but a link the arrow.

use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _};
use gpui_kit::{
    App, ElementId, Entity, FontWeight, Hsla, ParentElement as _, SharedString, Styled as _,
};

use gpui_kit::assets::IconName;

/// A button: 44 px high, base-size semibold text, the theme's corner radius
/// (paimon.moe's buttons). Chain the kit's builders on it: `.primary()`,
/// `.ghost()`, `.danger()`, `.label()`, `.icon()`, `.on_click()`.
///
/// Give it a label: an icon-only button wants its own square size.
pub fn button(id: impl Into<ElementId>) -> Button {
    button_style(Button::new(id))
}

/// Gives a button the kit builds itself, such as a dialog's close button, the
/// size of [`button`].
pub fn button_style(button: Button) -> Button {
    button
        .large()
        .h_11()
        .px_5()
        // Pills, as the game's buttons.
        .rounded_full()
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
}

/// The app's buttons point with a hand, so one that can't be used now must
/// show the arrow again.
pub trait ButtonExt {
    /// Disables the button when `disabled`, with the arrow cursor. Use it in
    /// place of the kit's `disabled` on the app's buttons.
    fn unavailable(self, disabled: bool) -> Self;
}

impl ButtonExt for Button {
    fn unavailable(self, disabled: bool) -> Self {
        let button = self.disabled(disabled);
        if disabled {
            button.cursor_default()
        } else {
            button
        }
    }
}

/// A dialog's footer with the app's buttons: Cancel, which closes the dialog,
/// then `action`, which runs the dialog's `on_ok` (`button(…).primary()`, or
/// `.danger()` for what removes data).
pub fn dialog_footer(action: Button) -> DialogFooter {
    DialogFooter::new()
        .child(DialogClose::new().trigger(|close| button_style(close).label("Cancel")))
        .child(DialogAction::new().child(action))
}

/// A dialog's footer with one button, `label`, which closes it.
pub fn dialog_close_footer(label: impl Into<SharedString>) -> DialogFooter {
    let label = label.into();
    DialogFooter::new()
        .child(DialogClose::new().trigger(move |close| button_style(close).label(label)))
}

/// A square button holding only an icon, as high as [`button`]. It has no
/// visible text, so it needs an accessibility label and a tooltip.
pub fn icon_button(
    id: impl Into<ElementId>,
    icon: impl Into<Icon>,
    label: impl Into<SharedString>,
) -> Button {
    let label = label.into();
    Button::new(id)
        .large()
        .size_11()
        .rounded_full()
        .icon(icon)
        .accessibility_label(label.clone())
        .tooltip(label)
        .cursor_pointer()
}

/// A text field: a 48 px pill on the page's colour, as the game's search
/// fields.
pub fn input(state: &Entity<InputState>, cx: &App) -> Input {
    let theme = cx.theme();
    Input::new(state)
        .large()
        .h_12()
        .px_5()
        .rounded_full()
        .bg(theme.background)
}

/// A filter that is on or off, as a chip: on, it is tinted
/// with `color` and ticked; off, only outlined. The tick says the state as
/// well as the colour does.
pub fn filter_chip(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    color: Hsla,
    on: bool,
    cx: &App,
) -> Button {
    // A 32 px pill; on, it is tinted and ticked; off, only outlined.
    let theme = cx.theme();
    let chip = Button::new(id)
        .h_8()
        .px_3()
        .rounded_full()
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .label(label)
        .toggled(on);
    if on {
        chip.custom(
            ButtonCustomVariant::new(cx)
                .color(color)
                .foreground(color)
                .hover(color.opacity(0.3))
                .active(color.opacity(0.4)),
        )
        .icon(IconName::Check)
    } else {
        chip.outline()
            .border_color(theme.border)
            .text_color(theme.muted_foreground)
    }
}
