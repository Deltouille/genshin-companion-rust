//! The form: the cookie's three fields (masked), or a whole cookie from the
//! clipboard, and what to know before giving it.

use gpui_kit::component::Disableable as _;
use gpui_kit::component::switch::Switch;

use super::*;
use crate::app::library::KeptLogin;

impl HoyolabSync {
    /// "Remember this cookie on this computer", and what it means; off where
    /// the system has no keyring.
    fn render_remember(&self, cx: &mut Context<Self>) -> Div {
        let kept = self.library.read(cx).kept_login();
        let usable = matches!(kept, KeptLogin::None | KeptLogin::Kept);
        let help = match kept {
            KeptLogin::Unavailable => t!("hoyolab-remember-unavailable"),
            KeptLogin::Kept => t!("hoyolab-remember-replaces"),
            KeptLogin::None | KeptLogin::Checking => t!("hoyolab-remember-help"),
        };
        div()
            .v_flex()
            .gap_1()
            .child(
                Switch::new("remember-cookie")
                    .cursor_pointer()
                    .checked(self.remember && usable)
                    .disabled(!usable)
                    .label(t!("hoyolab-remember"))
                    .accessibility_label(t!("hoyolab-remember"))
                    .on_change(cx.listener(|this, on: &bool, _, cx| {
                        this.remember = *on;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .pl(rems(2.75))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(help),
            )
    }

    pub(super) fn render_form(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let remember = self.render_remember(cx);
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let field = |ix: usize| {
            div()
                .v_flex()
                .gap_1p5()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(FIELDS[ix]),
                )
                .child(input(&self.fields[ix], cx))
        };
        let steps = [
            t!("hoyolab-step-1"),
            t!("hoyolab-step-2"),
            t!("hoyolab-step-3"),
        ];
        div()
            .v_flex()
            .gap_4()
            .child(div().text_sm().child(t!("hoyolab-intro")))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.warning)
                    .child(t!("hoyolab-warning")),
            )
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .text_sm()
                    .text_color(muted)
                    .children(steps),
            )
            .map(|this| match &self.pasted {
                Some(cookie) => this.child(
                    div()
                        .h_flex()
                        .gap_2()
                        .text_sm()
                        .child(Icon::new(IconName::Check).small())
                        .child(t!("hoyolab-pasted", names = cookie.names().join(", ")))
                        .child(
                            button("forget-pasted")
                                .quiet(cx)
                                .small()
                                .label(t!("hoyolab-use-fields"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.pasted = None;
                                    cx.notify();
                                })),
                        ),
                ),
                None => this.child(
                    div()
                        .grid()
                        .grid_cols(3)
                        .gap_3()
                        .child(field(0))
                        .child(field(1))
                        .child(field(2)),
                ),
            })
            .child(
                div().h_flex().child(
                    button("paste-cookie")
                        .quiet(cx)
                        .icon(IconName::Copy)
                        .label(t!("hoyolab-paste"))
                        .on_click(cx.listener(|this, _, _, cx| this.paste_cookie(cx))),
                ),
            )
            .child(remember)
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(t!("hoyolab-fine-print")),
            )
            .when_some(self.note.clone(), |this, note| {
                this.child(div().text_sm().text_color(theme.danger).child(note))
            })
            .child(
                footer()
                    .child(
                        button("cancel")
                            .label(t!("app-cancel"))
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        button("fetch-hoyolab")
                            .primary()
                            .label(t!("hoyolab-fetch"))
                            .on_click(cx.listener(|this, _, window, cx| this.fetch(window, cx))),
                    ),
            )
            .into_any_element()
    }
}
