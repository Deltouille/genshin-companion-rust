//! The app's buttons, fields, chips and dialog footers: pills, as in the game's
//! menus, at the app's size.
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
use gpui_kit::component::dialog::{Dialog, DialogAction, DialogClose, DialogFooter};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _};
use gpui_kit::{
    App, ElementId, Entity, FontWeight, Hsla, ParentElement as _, SharedString, Styled as _, Window,
};
use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::assets::IconName;

/// A button: a 44 px pill with base-size semibold text, filled parchment with
/// ink text in both themes (the `button.*` tokens). Chain the kit's builders on it: `.primary()`,
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
/// then `action`, which runs the dialog's `on_ok` (`button(…).primary()`).
/// Enter runs `on_ok` too (the kit binds it to the dialog's Confirm), so a
/// dialog that removes or replaces data uses [`confirm_destructive`] instead.
pub fn dialog_footer(action: Button) -> DialogFooter {
    DialogFooter::new()
        .child(DialogClose::new().trigger(|close| button_style(close).label(t!("ui-cancel"))))
        .child(DialogAction::new().child(action))
}

/// Asks before something that removes or replaces data: `title`, what will
/// happen (`description`), Cancel and a danger button labelled `action`,
/// which runs `confirmed` and closes the dialog.
///
/// Cancel has the focus when the dialog opens, and Enter only closes it: a key
/// pressed out of habit keeps the data. (The kit binds Enter to the dialog's
/// Confirm, which an action in `on_ok` would run.) Escape closes it too, and a
/// click outside doesn't: alert dialogs never close from the backdrop.
pub fn confirm_destructive(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    action: impl Into<SharedString>,
    confirmed: impl Fn(&mut Window, &mut App) + 'static,
) {
    let (title, description, action) = (title.into(), description.into(), action.into());
    let confirmed = Rc::new(confirmed);
    let cancel = cx.focus_handle();
    let focus_once = Cell::new(true);
    window.open_alert_dialog(cx, move |alert, window, cx| {
        if focus_once.replace(false) {
            cancel.focus(window, cx);
        }
        let (cancel, confirmed) = (cancel.clone(), confirmed.clone());
        alert
            .title(title.clone())
            .description(description.clone())
            .footer(
                DialogFooter::new()
                    .child(DialogClose::new().trigger(move |close| {
                        button_style(close)
                            .label(t!("ui-cancel"))
                            .track_focus(&cancel)
                    }))
                    .child(button("confirm").danger().label(action.clone()).on_click(
                        move |_, window, cx| {
                            confirmed(window, cx);
                            window.close_dialog(cx);
                        },
                    )),
            )
            // Enter: close, keeping everything.
            .on_ok(|_, _, _| true)
    });
}

/// A dialog the player types or picks values in: a click outside it doesn't
/// close it, since that would lose what was entered without a word. Escape and
/// Cancel still do. The kit's dialogs close on a click outside by default,
/// which suits a preview or a list, not a form.
pub fn form_dialog(dialog: Dialog) -> Dialog {
    dialog.overlay_closable(false)
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

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::Arc;

    use gpui_kit::component::{Root, WindowExt as _};
    use gpui_kit::{
        AppContext as _, Context, IntoElement, NoopTextSystem, Render, Styled as _, TestAppContext,
        TestDispatcher, VisualTestContext, Window, div,
    };

    use super::confirm_destructive;

    struct Page;

    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }

    /// A window with a destructive confirmation open, and how many times its
    /// action ran.
    fn confirmation(cx: &mut TestAppContext) -> (&mut VisualTestContext, Rc<Cell<u32>>) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let (_, cx) = cx.add_window_view(|window, cx| {
            let page = cx.new(|_| Page);
            Root::new(page, window, cx)
        });
        let ran = Rc::new(Cell::new(0));
        let count = ran.clone();
        cx.update(|window, cx| {
            confirm_destructive(window, cx, "Remove?", "It goes.", "Remove", move |_, _| {
                count.set(count.get() + 1)
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (cx, ran)
    }

    fn app() -> TestAppContext {
        TestAppContext::build_with_text_system(
            TestDispatcher::new(0),
            None,
            Arc::new(NoopTextSystem::new()),
        )
    }

    #[test]
    fn enter_in_a_destructive_confirmation_keeps_the_data() {
        let mut app = app();
        let (cx, ran) = confirmation(&mut app);
        assert!(cx.update(|window, cx| window.has_active_dialog(cx)));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(ran.get(), 0, "Enter ran the destructive action");
        assert!(!cx.update(|window, cx| window.has_active_dialog(cx)));
    }

    #[test]
    fn escape_closes_a_destructive_confirmation() {
        let mut app = app();
        let (cx, ran) = confirmation(&mut app);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert_eq!(ran.get(), 0);
        assert!(!cx.update(|window, cx| window.has_active_dialog(cx)));
    }
}
