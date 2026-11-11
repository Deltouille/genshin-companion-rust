//! Import from the game: find or paste the wish history link, fetch the
//! history (with progress, cancellable), preview what would change, then
//! write it. Lives inside a dialog opened by the shell.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::radio::RadioGroup;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, WindowExt as _};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, EventEmitter, FontWeight, IntoElement,
    ParentElement as _, PathPromptOptions, Render, SharedString, Styled as _, Subscription, Task,
    Window, div, prelude::FluentBuilder as _,
};
use wish_core::import::ImportReport;
use wish_core::import::api::GachaLink;
use wish_core::import::api_plan::{ApiPlan, Target};

use crate::app::library::{FetchControl, Library, is_cancelled};
use wish_ui::{ButtonExt as _, button, format, input};

pub enum GameImportEvent {
    Imported(ImportReport),
}

enum Step {
    Link,
    Fetching,
    Preview { plan: ApiPlan, choice: usize },
    Failed(SharedString),
}

/// A line under the link field: what was found, or what is wrong.
struct Note {
    text: SharedString,
    is_error: bool,
}

pub struct GameImport {
    library: Entity<Library>,
    input: Entity<InputState>,
    step: Step,
    finding: bool,
    note: Option<Note>,
    control: Arc<FetchControl>,
    task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<GameImportEvent> for GameImport {}

impl GameImport {
    pub fn new(library: Entity<Library>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("https://…"));
        let subscriptions = vec![
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::PressEnter { .. } => this.fetch(window, cx),
                InputEvent::Change => {
                    this.note = None;
                    cx.notify();
                }
                _ => {}
            }),
        ];
        Self {
            library,
            input,
            step: Step::Link,
            finding: false,
            note: None,
            control: Arc::new(FetchControl::default()),
            task: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn input(&self) -> &Entity<InputState> {
        &self.input
    }

    fn find_link(&mut self, folder: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        self.finding = true;
        self.note = None;
        cx.notify();
        let search = self
            .library
            .update(cx, |library, cx| library.find_game_link(folder.clone(), cx));
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let found = search.await;
            this.update_in(cx, |this, window, cx| {
                this.finding = false;
                match found {
                    Some(found) => {
                        this.input
                            .update(cx, |input, cx| input.set_value(found.url, window, cx));
                        this.note = Some(Note {
                            text: "Found the link of your last visit to the wish history. \
                                   If that was more than a day ago, open the wish history in the game again first."
                                .into(),
                            is_error: false,
                        });
                        if let Some(folder) = folder {
                            this.library
                                .update(cx, |library, cx| library.remember_game_folder(folder, cx))
                                .detach();
                        }
                    }
                    None => {
                        this.note = Some(Note {
                            text: "No link found. Open the wish history in the game (Wish, then History), \
                                   then try again, or choose the game folder."
                                .into(),
                            is_error: true,
                        });
                    }
                }
                cx.notify();
            })
            .ok();
        }));
    }

    fn choose_game_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose".into()),
        });
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(mut folders))) = picked.await
                && let Some(folder) = folders.pop()
            {
                this.update_in(cx, |this, window, cx| {
                    this.find_link(Some(folder), window, cx)
                })
                .ok();
            }
        }));
    }

    fn fetch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.step, Step::Link) {
            return;
        }
        let text = self.input.read(cx).value();
        let link = match GachaLink::parse(&text) {
            Ok(link) => link,
            Err(err) => {
                self.note = Some(Note {
                    text: capitalize(&err.to_string()).into(),
                    is_error: true,
                });
                cx.notify();
                return;
            }
        };

        self.control = Arc::new(FetchControl::default());
        self.step = Step::Fetching;
        cx.notify();
        let control = self.control.clone();
        let fetch = self
            .library
            .update(cx, |library, cx| library.fetch_from_game(link, control, cx));
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |this, cx| {
                this.step = match result {
                    Ok(plan) => Step::Preview {
                        choice: plan.suggested,
                        plan,
                    },
                    Err(err) if is_cancelled(&err) => Step::Link,
                    Err(err) => Step::Failed(capitalize(&format!("{err:#}")).into()),
                };
                cx.notify();
            })
            .ok();
        }));

        // Progress lives in the fetch's thread; redraw while it runs.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let fetching = this.update(cx, |this, cx| {
                    cx.notify();
                    matches!(this.step, Step::Fetching)
                });
                if !matches!(fetching, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    fn cancel_fetch(&mut self) {
        self.control.cancel();
    }

    fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Step::Preview { plan, choice } = &self.step else {
            return;
        };
        let Some(target) = plan.targets.get(*choice) else {
            return;
        };
        let apply = self.library.update(cx, |library, cx| {
            library.apply_import(target.batch.clone(), cx)
        });
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = apply.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(report) => {
                    cx.emit(GameImportEvent::Imported(report));
                    window.close_dialog(cx);
                }
                Err(err) => {
                    this.step = Step::Failed(format!("{err:#}\n\nNothing was changed.").into());
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    fn render_link(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let has_text = !self.input.read(cx).value().trim().is_empty();
        div()
            .v_flex()
            .gap_3()
            .child(div().text_sm().text_color(theme.muted_foreground).child(
                "Open the wish history in the game (Wish, then History), then find its link \
                         here or paste it. A link works for about a day. On a phone or a console, \
                         copy the link there and paste it here.",
            ))
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(
                        button("find-link")
                            .label("Find link")
                            .icon(IconName::Search)
                            .loading(self.finding)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.find_link(None, window, cx)),
                            ),
                    )
                    .child(
                        button("choose-game-folder")
                            .label("Choose game folder…")
                            .unavailable(self.finding)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.choose_game_folder(window, cx)
                            })),
                    ),
            )
            .child(input(&self.input, cx))
            .when_some(self.note.as_ref(), |this, note| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(if note.is_error {
                            theme.danger
                        } else {
                            theme.muted_foreground
                        })
                        .child(note.text.clone()),
                )
            })
            .child(
                footer()
                    .child(
                        button("cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        button("fetch")
                            .primary()
                            .label("Fetch history")
                            .unavailable(!has_text)
                            .on_click(cx.listener(|this, _, window, cx| this.fetch(window, cx))),
                    ),
            )
            .into_any_element()
    }

    fn render_fetching(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (status, count) = match self.control.progress() {
            Some(progress) => (
                format!(
                    "Fetching {} wishes, page {}…",
                    progress.gacha_type.pool().display_name(),
                    progress.page
                ),
                format!(
                    "{} new {} so far",
                    format::count(progress.fetched as u64),
                    format::plural(progress.fetched as u32, "wish", "wishes")
                ),
            ),
            None => ("Connecting to the game server…".to_owned(), String::new()),
        };
        div()
            .v_flex()
            .gap_3()
            .child(div().h_flex().gap_2().child(Spinner::new()).child(status))
            .child(div().text_sm().text_color(theme.muted_foreground).child(count))
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("The game server allows about one page of 20 wishes per second, so a long history takes a minute or two."),
            )
            .child(
                footer().child(
                    button("cancel-fetch")
                        .label("Cancel")
                        .on_click(cx.listener(|this, _, _, _| this.cancel_fetch())),
                ),
            )
            .into_any_element()
    }

    fn render_preview(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let busy = self.library.read(cx).is_busy();
        let Step::Preview { plan, choice } = &self.step else {
            return div().into_any_element();
        };

        let Some(target) = plan.targets.get(*choice) else {
            return div()
                .v_flex()
                .gap_3()
                .child("The game returned no wishes from the last six months.")
                .child(footer().child(close_button("close")))
                .into_any_element();
        };

        let mut heading = format!("UID {}", plan.uid.clone().unwrap_or_default());
        if let Some(server) = plan.server {
            heading = format!("{heading} · {}", server_name(server));
        }

        let new_total = target.new_total();
        let new_line = if new_total == 0 {
            "No new wishes: your history is up to date.".to_owned()
        } else {
            let pools = target
                .new_wishes
                .iter()
                .map(|(pool, n)| format!("{} {}", pool.display_name(), format::count(*n)))
                .collect::<Vec<_>>()
                .join(" · ");
            format!(
                "{} new {}: {pools}",
                format::count(new_total),
                format::plural(new_total, "wish", "wishes")
            )
        };
        let has_changes = new_total > 0 || target.linked > 0;

        let choices = (plan.targets.len() > 1).then(|| {
            let labels: Vec<SharedString> = plan
                .targets
                .iter()
                .map(|t| match &t.target {
                    Target::New => "A new account".into(),
                    Target::Existing(account) => format!("{} (no UID yet)", account.label).into(),
                })
                .collect();
            div()
                .v_flex()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .child("This UID has no account here yet. Add its wishes to:"),
                )
                .child(
                    RadioGroup::new("import-target")
                        .children(labels)
                        .selected_index(Some(*choice))
                        .on_change(cx.listener(|this, ix: &usize, _, cx| {
                            if let Step::Preview { choice, .. } = &mut this.step {
                                *choice = *ix;
                                cx.notify();
                            }
                        })),
                )
        });

        div()
            .v_flex()
            .gap_3()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(heading))
            .children(choices)
            .child(div().text_sm().child(new_line))
            .when(target.linked > 0, |this| {
                this.child(div().text_sm().text_color(theme.muted_foreground).child(format!(
                    "{} {} from paimon.moe matched the game’s records, which are noted for future imports.",
                    format::count(target.linked),
                    format::plural(target.linked, "wish", "wishes"),
                )))
            })
            .children(target.batch.warnings.iter().map(|warning| {
                div()
                    .h_flex()
                    .gap_2()
                    .text_sm()
                    .child(Icon::new(IconName::TriangleAlert).small().text_color(theme.warning))
                    .child(warning.clone())
            }))
            .child(if has_changes {
                footer()
                    .child(
                        button("cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        button("import")
                            .primary()
                            .label("Import")
                            .loading(busy)
                            .on_click(cx.listener(|this, _, window, cx| this.import(window, cx))),
                    )
            } else {
                footer().child(close_button("close"))
            })
            .into_any_element()
    }

    fn render_failed(&mut self, message: SharedString, cx: &mut Context<Self>) -> AnyElement {
        div()
            .v_flex()
            .gap_3()
            .child(div().text_sm().child(message))
            .child(
                footer().child(close_button("close")).child(
                    button("try-again")
                        .primary()
                        .label("Try again")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.step = Step::Link;
                            cx.notify();
                        })),
                ),
            )
            .into_any_element()
    }
}

impl Render for GameImport {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.step {
            Step::Link => self.render_link(cx),
            Step::Fetching => self.render_fetching(cx),
            Step::Preview { .. } => self.render_preview(cx),
            Step::Failed(message) => {
                let message = message.clone();
                self.render_failed(message, cx)
            }
        }
    }
}

impl Drop for GameImport {
    /// Closing the dialog, however it happens, stops a fetch in progress.
    fn drop(&mut self) {
        self.control.cancel();
    }
}

fn footer() -> gpui_kit::Div {
    div().h_flex().justify_end().gap_2().pt_2()
}

fn close_button(id: &'static str) -> Button {
    button(id)
        .label("Close")
        .on_click(|_, window, cx| window.close_dialog(cx))
}

fn server_name(server: wish_core::model::Server) -> &'static str {
    use wish_core::model::Server;
    match server {
        Server::Asia => "Asia",
        Server::America => "America",
        Server::Europe => "Europe",
        Server::TwHkMo => "TW, HK, MO",
        Server::China => "China",
    }
}

/// Error messages start lowercase (they are also used mid-sentence).
fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
