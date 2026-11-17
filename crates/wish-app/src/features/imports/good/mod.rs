//! Import a GOOD file: the player picks the file Irminsul, Inventory Kamera
//! or Genshin Optimizer exported (or pastes what Irminsul copied), the app
//! shows what would change in the account shown, and writes it once
//! confirmed. Lives in a dialog the shell opens (`ImportGood`). The preview's
//! words are `model.rs`'s.

mod model;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, WindowExt as _};
use gpui_kit::{
    AnyElement, App, Context, Div, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, PathPromptOptions, Render, SharedString, Styled as _, Task, Window, div,
    prelude::FluentBuilder as _, rems,
};
use wish_ui::{ButtonExt as _, button, format};

use crate::app::library::{GoodPreview, GoodSource, Library};
use crate::app::{Notice, Session, notify};

enum Step {
    Choose,
    Reading,
    Preview(Box<GoodPreview>),
    Failed(SharedString),
}

pub struct GoodImport {
    library: Entity<Library>,
    session: Entity<Session>,
    step: Step,
    /// What went wrong with the clipboard.
    note: Option<SharedString>,
    task: Option<Task<()>>,
}

impl GoodImport {
    pub fn new(library: Entity<Library>, session: Entity<Session>) -> Self {
        Self {
            library,
            session,
            step: Step::Choose,
            note: None,
            task: None,
        }
    }

    /// The name of the account the file goes into: the one shown.
    fn account_label(&self, cx: &App) -> Option<String> {
        let id = self.session.read(cx).account()?;
        let library = self.library.read(cx);
        Some(library.account(id)?.account.label.clone())
    }

    fn choose_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from(t!("good-prompt"))),
        });
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let path = match picked.await {
                Ok(Ok(Some(mut paths))) => paths.pop(),
                Ok(Err(err)) => {
                    this.update(cx, |_, cx| {
                        notify(Notice::failure(t!("import-file-picker-failed"), &err), cx);
                    })
                    .ok();
                    None
                }
                Ok(Ok(None)) | Err(_) => None,
            };
            let Some(path) = path else { return };
            this.update_in(cx, |this, window, cx| {
                this.read(GoodSource::File(path), window, cx);
            })
            .ok();
        }));
    }

    /// Reads a GOOD file from the clipboard (Irminsul copies its export
    /// there).
    fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = cx.read_from_clipboard().and_then(|item| item.text());
        match text.filter(|text| !text.trim().is_empty()) {
            Some(text) => self.read(GoodSource::Text(text), window, cx),
            None => {
                self.note = Some(t!("good-clipboard-empty").into());
                cx.notify();
            }
        }
    }

    /// Reads `source` and shows what it would change (the screenshot tool
    /// calls it with a sample file).
    pub(crate) fn read(&mut self, source: GoodSource, window: &mut Window, cx: &mut Context<Self>) {
        self.note = None;
        self.step = Step::Reading;
        cx.notify();
        let shown = self.session.read(cx).account();
        let preview = self
            .library
            .update(cx, |library, cx| library.preview_good(source, shown, cx));
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = preview.await;
            this.update(cx, |this, cx| {
                this.step = match result {
                    Ok(preview) => Step::Preview(Box::new(preview)),
                    Err(err) => Step::Failed(format!("{err:#}").into()),
                };
                cx.notify();
            })
            .ok();
        }));
    }

    fn apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Step::Preview(preview) = &self.step else {
            return;
        };
        let preview = (**preview).clone();
        let label =
            (preview.account.as_ref()).map_or_else(|| "Main".to_owned(), |a| a.label.clone());
        let apply = self
            .library
            .update(cx, |library, cx| library.apply_good(preview, cx));
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = apply.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok((account, plan)) => {
                    this.session
                        .update(cx, |session, cx| session.select_account(account, cx));
                    notify(
                        Notice::done(t!(
                            "good-imported",
                            account = format::quoted(&label),
                            count = plan.changes()
                        )),
                        cx,
                    );
                    window.close_dialog(cx);
                }
                Err(err) => {
                    this.step =
                        Step::Failed(format!("{err:#}\n\n{}", t!("import-nothing-changed")).into());
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    fn render_choose(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let intro = match self.account_label(cx) {
            Some(label) => t!("good-intro", account = format::quoted(&label)),
            None => t!("good-intro-new"),
        };
        div()
            .v_flex()
            .gap_4()
            .child(div().text_sm().child(intro))
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .text_sm()
                    .text_color(muted)
                    .child(t!("good-how-irminsul"))
                    .child(t!("good-how-kamera"))
                    .child(t!("good-how-optimizer")),
            )
            .child(div().text_xs().text_color(muted).child(t!("good-safe")))
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
                        button("paste-good")
                            .icon(IconName::Copy)
                            .label(t!("good-paste"))
                            .on_click(cx.listener(|this, _, window, cx| this.paste(window, cx))),
                    )
                    .child(
                        button("choose-good")
                            .primary()
                            .icon(IconName::FolderOpen)
                            .label(t!("good-choose-file"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.choose_file(window, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_preview(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let library = self.library.read(cx);
        let (busy, planner) = (library.is_busy(), library.planner().clone());
        let Step::Preview(preview) = &self.step else {
            return div().into_any_element();
        };
        let plan = &preview.plan;
        let changes = plan.changes();
        let sections: Vec<Div> = (model::sections(plan, &planner).into_iter())
            .map(|section| {
                div()
                    .v_flex()
                    .gap_0p5()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .pb_0p5()
                            .child(section.title),
                    )
                    .children(section.lines.into_iter().map(|(sign, text)| {
                        div()
                            .h_flex()
                            .gap_2()
                            .child(div().w(rems(1.)).flex_none().child(sign))
                            .child(div().flex_1().min_w_0().child(text))
                    }))
            })
            .collect();
        div()
            .v_flex()
            .gap_3()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(model::heading(&preview.file.source, &preview.account)),
            )
            .child(div().text_sm().child(model::summary(plan)))
            .when(!sections.is_empty(), |this| {
                this.child(
                    div()
                        .id("good-changes")
                        .max_h(rems(20.))
                        .overflow_y_scrollbar()
                        .rounded(theme.radius_lg)
                        .bg(theme.muted)
                        .child(div().v_flex().gap_3().p_3().text_sm().children(sections)),
                )
            })
            .when(changes == 0, |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child(t!("good-nothing-new")),
                )
            })
            // What the file holds that the app leaves out, said.
            .when(!plan.collection.notes.is_empty(), |this| {
                this.child(div().v_flex().gap_1().text_sm().text_color(muted).children(
                    plan.collection.notes.iter().map(|note| {
                        div()
                            .h_flex()
                            .items_start()
                            .gap_2()
                            .child(Icon::new(IconName::Info).small().flex_none().mt_0p5())
                            .child(div().flex_1().min_w_0().child(note.to_string()))
                    }),
                ))
            })
            .child(
                footer()
                    .child(
                        button("cancel")
                            .label(if changes == 0 {
                                t!("app-close")
                            } else {
                                t!("app-cancel")
                            })
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .when(changes > 0, |this| {
                        this.child(
                            button("apply-good")
                                .primary()
                                .label(t!("good-import"))
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

    fn render_failed(&mut self, message: SharedString, cx: &mut Context<Self>) -> AnyElement {
        div()
            .v_flex()
            .gap_3()
            .child(div().text_sm().child(message))
            .child(
                footer()
                    .child(
                        button("close")
                            .label(t!("app-close"))
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        button("try-again")
                            .primary()
                            .label(t!("hoyolab-try-again"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.step = Step::Choose;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}

impl Render for GoodImport {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.step {
            Step::Choose => self.render_choose(cx),
            Step::Reading => div()
                .h_flex()
                .gap_2()
                .child(Spinner::new())
                .child(t!("good-reading"))
                .into_any_element(),
            Step::Preview(_) => self.render_preview(cx),
            Step::Failed(message) => {
                let message = message.clone();
                self.render_failed(message, cx)
            }
        }
    }
}

fn footer() -> Div {
    div().h_flex().justify_end().gap_2().pt_2()
}
