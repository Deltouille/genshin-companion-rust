//! The form: the cookie's three fields (masked), or a whole cookie from the
//! clipboard, and what to know before giving it.

use super::*;

impl HoyolabSync {
    pub(super) fn render_form(&mut self, cx: &mut Context<Self>) -> AnyElement {
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
            "Log in to hoyolab.com in your browser.",
            "Open its developer tools (F12), then Storage (Application in Chrome or Edge) → \
             Cookies → https://www.hoyolab.com.",
            "Copy the values of ltuid_v2, ltoken_v2 and ltmid_v2 into the fields below.",
        ];
        div()
            .v_flex()
            .gap_4()
            .child(div().text_sm().child(
                "HoYoLAB shows your characters as they are in the game: their level, \
                 constellations and talents, and the weapon each holds. The app reads them with \
                 your HoYoLAB cookie, for this sync only: the cookie is never saved, and goes to \
                 HoYoverse only. Nothing is written before you confirm.",
            ))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.warning)
                    .child(
                        "These values let whoever has them use your HoYoLAB account. Give them \
                         to this app only: never to a website, a Discord bot or another person.",
                    ),
            )
            .child(
                div().v_flex().gap_1().text_sm().text_color(muted).children(
                    steps
                        .iter()
                        .enumerate()
                        .map(|(ix, step)| format!("{}. {step}", ix + 1)),
                ),
            )
            .map(|this| match &self.pasted {
                Some(cookie) => this.child(
                    div()
                        .h_flex()
                        .gap_2()
                        .text_sm()
                        .child(Icon::new(IconName::Check).small())
                        .child(format!(
                            "Cookie read from the clipboard ({}); the clipboard was emptied. If \
                             your system keeps a clipboard history (Win+V, Klipper…), delete the \
                             entry there too.",
                            cookie.names().join(", ")
                        ))
                        .child(
                            button("forget-pasted")
                                .ghost()
                                .small()
                                .label("Use the fields")
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
                        .ghost()
                        .icon(IconName::Copy)
                        .label("Paste a whole cookie")
                        .on_click(cx.listener(|this, _, _, cx| this.paste_cookie(cx))),
                ),
            )
            .child(div().text_xs().text_color(muted).child(
                "Typing the three values is safer than pasting: a clipboard history keeps what \
                 is copied. HoYoLAB doesn’t show the weapons in your bag or your materials. \
                 Characters at a level cap (20, 40 … 80) count as not yet ascended, since \
                 HoYoLAB doesn’t say. HoYoLAB has no public API: this uses the one its website \
                 uses, which HoYoverse may change or block.",
            ))
            .when_some(self.note.clone(), |this, note| {
                this.child(div().text_sm().text_color(theme.danger).child(note))
            })
            .child(
                footer()
                    .child(
                        button("cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        button("fetch-hoyolab")
                            .primary()
                            .label("Fetch from HoYoLAB")
                            .on_click(cx.listener(|this, _, window, cx| this.fetch(window, cx))),
                    ),
            )
            .into_any_element()
    }
}
