//! Sync from HoYoLAB: the player gives their cookie (typed into masked
//! fields, or read from the clipboard, which is then cleared), the app asks
//! HoYoLAB for the characters of their Genshin accounts, shows what would
//! change in the collection, and writes it once confirmed.
//!
//! The cookie is never stored: it lives in this dialog until the fetch
//! starts (the fields are emptied then), in the fetch until its requests are
//! done, and nowhere after. Lives in a dialog the shell opens
//! (`SyncFromHoyolab`).

mod form;
pub(super) mod model;
mod preview;

use std::sync::Arc;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::InputState;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, WindowExt as _};
use gpui_kit::{
    AnyElement, App, AppContext as _, ClipboardItem, Context, Div, Entity, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Task, Window, div, prelude::FluentBuilder as _, rems,
};
use wish_core::hoyolab::{Cookie, Secret};

use crate::app::library::{FetchControl, HoyolabAccount, Library, is_cancelled};
use crate::app::{Notice, Session, notify};
use wish_ui::{ButtonExt as _, Segmented, button, input};

/// The cookie fields the form asks for, as hoyolab.com names them.
const FIELDS: [&str; 3] = ["ltuid_v2", "ltoken_v2", "ltmid_v2"];

enum Step {
    Form,
    Fetching,
    Preview {
        accounts: Vec<HoyolabAccount>,
        /// The game account shown.
        choice: usize,
        /// For each game account, the index of the app account it goes to
        /// (into its `choices`).
        targets: Vec<usize>,
    },
    Failed(SharedString),
}

/// The HoYoLAB sync's dialog, one step at a time: the cookie, the fetch, the
/// preview with the account to sync into, then the sync.
pub struct HoyolabSync {
    library: Entity<Library>,
    session: Entity<Session>,
    /// `ltuid_v2`, `ltoken_v2` and `ltmid_v2`, masked.
    fields: [Entity<InputState>; 3],
    /// A whole cookie read from the clipboard, in place of the fields.
    pasted: Option<Cookie>,
    /// What is wrong with the form.
    note: Option<SharedString>,
    step: Step,
    control: Arc<FetchControl>,
    task: Option<Task<()>>,
}

impl HoyolabSync {
    pub fn new(
        library: Entity<Library>,
        session: Entity<Session>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let fields = FIELDS
            .map(|name| cx.new(|cx| InputState::new(window, cx).placeholder(name).masked(true)));
        Self {
            library,
            session,
            fields,
            pasted: None,
            note: None,
            step: Step::Form,
            control: Arc::new(FetchControl::default()),
            task: None,
        }
    }

    /// The first field, to focus when the dialog opens.
    pub fn first_field(&self) -> &Entity<InputState> {
        &self.fields[0]
    }

    /// Reads a whole cookie from the clipboard (a `Cookie` header or
    /// `name=value; …` pairs), then empties the clipboard. A clipboard
    /// history (Win+V, Klipper…) keeps its own copy, which the dialog asks the
    /// player to delete.
    fn paste_cookie(&mut self, cx: &mut Context<Self>) {
        let text = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .map(Secret::new);
        match text.as_ref().map(|text| Cookie::parse(text.expose())) {
            Some(Ok(cookie)) => {
                self.pasted = Some(cookie);
                self.note = None;
                cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
            }
            Some(Err(_)) | None => {
                self.note = Some(
                    "The clipboard holds no HoYoLAB cookie: copy ltuid_v2, ltoken_v2 and \
                     ltmid_v2 as one line (ltuid_v2=…; ltoken_v2=…; ltmid_v2=…), or type \
                     them in the fields."
                        .into(),
                );
            }
        }
        cx.notify();
    }

    /// The cookie from the clipboard, or from the fields.
    fn cookie(&self, cx: &App) -> Result<Cookie, SharedString> {
        if let Some(cookie) = &self.pasted {
            return Ok(cookie.clone());
        }
        let values = self
            .fields
            .each_ref()
            .map(|field| Secret::new(field.read(cx).value().trim().to_owned()));
        if values.iter().any(Secret::is_empty) {
            return Err(t!("hoyolab-fill-fields").into());
        }
        let line = Secret::new(
            FIELDS
                .iter()
                .zip(&values)
                .map(|(name, value)| format!("{name}={}", value.expose()))
                .collect::<Vec<_>>()
                .join("; "),
        );
        Cookie::parse(line.expose()).map_err(|e| e.to_string().into())
    }

    /// Empties the fields and forgets a pasted cookie: once the fetch has it,
    /// the dialog keeps none.
    fn forget_cookie(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pasted = None;
        for field in &self.fields {
            field.update(cx, |field, cx| field.set_value("", window, cx));
        }
    }

    fn fetch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.step, Step::Form) {
            return;
        }
        let cookie = match self.cookie(cx) {
            Ok(cookie) => cookie,
            Err(note) => {
                self.note = Some(note);
                cx.notify();
                return;
            }
        };
        self.forget_cookie(window, cx);
        self.start_fetch(cookie, window, cx);
    }

    /// Asks HoYoLAB with `cookie`; the preview follows (the screenshot tool
    /// calls it with a sample cookie and a fake HoYoLAB).
    pub(crate) fn start_fetch(
        &mut self,
        cookie: Cookie,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.note = None;
        self.control = Arc::new(FetchControl::default());
        self.step = Step::Fetching;
        cx.notify();
        let shown = self.session.read(cx).account();
        let fetch = self.library.update(cx, |library, cx| {
            library.fetch_from_hoyolab(cookie, shown, self.control.clone(), cx)
        });
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |this, cx| {
                this.step = match result {
                    Ok(accounts) => {
                        // The account with the highest Adventure Rank first.
                        let choice = (0..accounts.len())
                            .max_by_key(|&i| accounts[i].account.level)
                            .unwrap_or(0);
                        let targets = accounts.iter().map(|a| a.suggested).collect();
                        Step::Preview {
                            accounts,
                            choice,
                            targets,
                        }
                    }
                    Err(err) if is_cancelled(&err) => Step::Form,
                    Err(err) => Step::Failed(format!("{err:#}").into()),
                };
                cx.notify();
            })
            .ok();
        }));
    }

    fn apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Step::Preview {
            accounts,
            choice,
            targets,
        } = &self.step
        else {
            return;
        };
        let Some(fetched) = accounts.get(*choice).cloned() else {
            return;
        };
        let target = (fetched.choices.get(targets[*choice]))
            .and_then(|choice| choice.target.as_ref().map(|account| account.id));
        let nickname = fetched.account.nickname.clone();
        let apply = self.library.update(cx, |library, cx| {
            library.apply_hoyolab_sync(fetched, target, cx)
        });
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = apply.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok((account, plan)) => {
                    this.session
                        .update(cx, |session, cx| session.select_account(account, cx));
                    notify(
                        Notice::done(t!(
                            "hoyolab-synced",
                            nickname = nickname.as_str(),
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
}

impl Render for HoyolabSync {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.step {
            Step::Form => self.render_form(cx),
            Step::Fetching => self.render_fetching(cx),
            Step::Preview { .. } => self.render_preview(cx),
            Step::Failed(message) => {
                let message = message.clone();
                self.render_failed(message, cx)
            }
        }
    }
}

impl Drop for HoyolabSync {
    /// Closing the dialog, however it happens, stops a fetch in progress.
    fn drop(&mut self) {
        self.control.cancel();
    }
}

fn footer() -> Div {
    div().h_flex().justify_end().gap_2().pt_2()
}
