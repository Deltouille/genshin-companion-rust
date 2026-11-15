//! The other steps: the fetch under way, the preview of what a sync would
//! change (which game account, into which app account, each change), and a
//! failure. The preview's words are `model.rs`'s.

use super::*;

impl HoyolabSync {
    pub(super) fn render_fetching(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .v_flex()
            .gap_3()
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(Spinner::new())
                    .child("Asking HoYoLAB for your characters…"),
            )
            .child(
                footer().child(button("cancel-fetch").label("Cancel").on_click(cx.listener(
                    |this, _, _, _| {
                        this.control.cancel();
                    },
                ))),
            )
            .into_any_element()
    }

    pub(super) fn render_preview(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let library = self.library.read(cx);
        let (busy, planner) = (library.is_busy(), library.planner().clone());
        let Step::Preview {
            accounts,
            choice,
            targets,
        } = &self.step
        else {
            return div().into_any_element();
        };
        let Some(fetched) = accounts.get(*choice) else {
            return div().into_any_element();
        };
        let target = targets[*choice];
        let Some(chosen) = fetched.choices.get(target) else {
            return div().into_any_element();
        };
        let choices = (accounts.len() > 1).then(|| {
            accounts
                .iter()
                .fold(Segmented::new("hoyolab-account"), |segments, a| {
                    segments.option(format!("{} · AR {}", a.account.nickname, a.account.level))
                })
                .selected(*choice)
                .on_select(cx.listener(|this, ix: &usize, _, cx| {
                    if let Step::Preview { choice, .. } = &mut this.step {
                        *choice = *ix;
                        cx.notify();
                    }
                }))
        });
        let game = &fetched.account;
        // No account has the UID yet: the player picks where it goes, the
        // account whose wish history is theirs first.
        let destinations = (fetched.choices.len() > 1).then(|| {
            let segments = (fetched.choices.iter())
                .fold(Segmented::new("hoyolab-target"), |segments, choice| {
                    segments.option(model::target_label(&choice.target, game))
                })
                .selected(target)
                .on_select(cx.listener(|this, ix: &usize, _, cx| {
                    if let Step::Preview {
                        choice, targets, ..
                    } = &mut this.step
                    {
                        targets[*choice] = *ix;
                        cx.notify();
                    }
                }));
            div()
                .v_flex()
                .gap_2()
                .child(div().text_sm().child(format!(
                    "No account here has UID {} yet. Choose the one whose wish history is this \
                     player’s: it takes the UID, so the next import from the game goes there too.",
                    game.uid
                )))
                .child(div().h_flex().child(segments))
        });
        let into = model::into(&chosen.target, game);
        let plan = &chosen.plan;
        let lines: Vec<Div> = (model::lines(plan, &planner).into_iter())
            .map(|(sign, text)| {
                div()
                    .h_flex()
                    .gap_2()
                    .child(div().w(rems(1.)).flex_none().child(sign))
                    .child(div().flex_1().min_w_0().child(text))
            })
            .collect();
        let changes = plan.changes();
        div()
            .v_flex()
            .gap_3()
            .children(choices.map(|choices| div().h_flex().child(choices)))
            .children(destinations)
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("UID {} · {}, {into}", game.uid, game.nickname)),
            )
            .child(div().text_sm().child(model::summary(plan)))
            .when(!lines.is_empty(), |this| {
                this.child(
                    div()
                        .id("hoyolab-changes")
                        .max_h(rems(18.))
                        .overflow_y_scrollbar()
                        .rounded(theme.radius_lg)
                        .bg(theme.muted)
                        .child(div().v_flex().gap_0p5().p_3().text_sm().children(lines)),
                )
            })
            .when(changes == 0, |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child("Your collection already has all of this."),
                )
            })
            .child(
                footer()
                    .child(
                        button("cancel")
                            .label(if changes == 0 { "Close" } else { "Cancel" })
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .when(changes > 0, |this| {
                        this.child(
                            button("apply-hoyolab")
                                .primary()
                                .label("Sync")
                                .loading(busy)
                                .unavailable(busy)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.apply(window, cx)),
                                ),
                        )
                    }),
            )
            .into_any_element()
    }

    pub(super) fn render_failed(
        &mut self,
        message: SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .v_flex()
            .gap_3()
            .child(div().text_sm().child(message))
            .child(
                footer()
                    .child(
                        button("close")
                            .label("Close")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        button("try-again")
                            .primary()
                            .label("Try again")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.step = Step::Form;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}
