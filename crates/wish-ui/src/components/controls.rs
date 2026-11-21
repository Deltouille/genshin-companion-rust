//! The app's buttons, fields, chips and dialog footers, at the kit's medium
//! size (32 px, 14 px text) and with the theme's small corners (`radius`).
//!
//! These builders return the kit's own `Button` (and `Input`), keeping their
//! keyboard, focus and accessibility behaviour and their size, with the app's
//! shape, weight and colours: use them instead of `Button::new`. For a choice
//! in a list, use [`super::picker`], not the kit's `Select`.
//!
//! They also show the pointing hand over what can be clicked, as a web page
//! does: the kit gives every button but a link the arrow.

use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::dialog::{Dialog, DialogAction, DialogClose, DialogFooter};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, WindowExt as _};
use gpui_kit::{
    App, ElementId, Entity, FontWeight, Hsla, ParentElement as _, SharedString, Styled as _, Window,
};
use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::assets::IconName;

/// A button: 32 px high with the theme's small corners and semibold 14 px
/// text, on a grey fill (the `button.*` tokens). Chain the kit's builders on it: `.primary()` (the accent),
/// `.danger()`, `.label()`, `.icon()`, `.on_click()`; without fill, [`ButtonExt::quiet`]
/// (never the kit's `.ghost()`).
///
/// Give it a label: an icon-only button is an [`icon_button`].
pub fn button(id: impl Into<ElementId>) -> Button {
    button_style(Button::new(id))
}

/// Gives a button the kit builds itself, such as a dialog's close button, the
/// look of [`button`].
pub fn button_style(button: Button) -> Button {
    button
        // A little more room than the kit's for the semibold labels; the
        // corners are the kit's, the theme's `radius`.
        .px_4()
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
}

/// The app's own states on the kit's buttons.
pub trait ButtonExt {
    /// Disables the button when `disabled`, with the arrow cursor. Use it in
    /// place of the kit's `disabled` on the app's buttons: they point with a
    /// hand, so one that can't be used now must show the arrow again.
    fn unavailable(self, disabled: bool) -> Self;

    /// A button without fill: a layer of its text colour when hovered, a
    /// stronger one while pressed. Use it in place of the kit's `ghost`, which
    /// presses with the filled buttons' colour, a flash that hides the text
    /// on a dark surface.
    fn quiet(self, cx: &App) -> Self;

    /// [`ButtonExt::quiet`] on a surface of its own, with text, hover and
    /// pressed layers of `text`: the sidebar.
    fn quiet_in(self, text: Hsla, cx: &App) -> Self;
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

    fn quiet(self, cx: &App) -> Self {
        self.quiet_in(cx.theme().secondary_foreground, cx)
    }

    fn quiet_in(self, text: Hsla, cx: &App) -> Self {
        let theme = cx.theme();
        // The kit's ghost hover (the `accent` layer, half of it in the dark
        // theme); pressed, the state layer of Material 3.
        let hover = text.opacity(if theme.mode.is_dark() { 0.04 } else { 0.08 });
        self.custom(
            ButtonCustomVariant::new(cx)
                .color(theme.transparent)
                .foreground(text)
                .hover(hover)
                .active(text.opacity(0.12)),
        )
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
        .icon(icon)
        .accessibility_label(label.clone())
        .tooltip(label)
        .cursor_pointer()
}

/// A text field: 32 px high, on the page's colour, set into the card.
pub fn input(state: &Entity<InputState>, cx: &App) -> Input {
    let theme = cx.theme();
    Input::new(state)
        // As much room as a button's.
        .px_4()
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
    // 32 px high; on, it is tinted and ticked; off, only outlined.
    let theme = cx.theme();
    let chip = Button::new(id)
        .h_8()
        .px_3()
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
