//! Sync from HoYoLAB: the player gives their cookie (typed into masked
//! fields, or read from the clipboard, which is then cleared), the app asks
//! HoYoLAB for the characters of their Genshin accounts, shows what would
//! change in the collection, and writes it once confirmed.
//!
//! The cookie is never stored: it lives in this dialog until the fetch
//! starts (the fields are emptied then), in the fetch until its requests are
//! done, and nowhere after. Lives in a dialog the shell opens
//! (`SyncFromHoyolab`).

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
use wish_core::collection::{Change, OwnedCharacter, OwnedWeapon, SyncPlan};
use wish_core::hoyolab::{Cookie, Secret};
use wish_core::planner::{Level, PlannerData};

use crate::app::library::{FetchControl, HoyolabAccount, Library, is_cancelled};
use crate::app::{Notice, Session, notify};
use wish_ui::{ButtonExt as _, Segmented, button, format, input};

/// The cookie fields the form asks for, as hoyolab.com names them.
const FIELDS: [&str; 3] = ["ltuid_v2", "ltoken_v2", "ltmid_v2"];

enum Step {
    Form,
    Fetching,
    Preview {
        accounts: Vec<HoyolabAccount>,
        choice: usize,
    },
    Failed(SharedString),
}

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
    /// `name=value; …` pairs), then clears the clipboard so it doesn't linger.
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
            return Err("Fill in the three fields, or paste the whole cookie.".into());
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
        self.note = None;
        self.control = Arc::new(FetchControl::default());
        self.step = Step::Fetching;
        cx.notify();
        let fetch = self.library.update(cx, |library, cx| {
            library.fetch_from_hoyolab(cookie, self.control.clone(), cx)
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
                        Step::Preview { accounts, choice }
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
        let Step::Preview { accounts, choice } = &self.step else {
            return;
        };
        let Some(fetched) = accounts.get(*choice).cloned() else {
            return;
        };
        let nickname = fetched.account.nickname.clone();
        let apply = self
            .library
            .update(cx, |library, cx| library.apply_hoyolab_sync(fetched, cx));
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = apply.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok((account, plan)) => {
                    this.session
                        .update(cx, |session, cx| session.select_account(account, cx));
                    notify(
                        Notice::done(format!(
                            "{nickname}’s characters are synced from HoYoLAB: {} {}.",
                            format::count(plan.changes() as u64),
                            format::plural(plan.changes() as u32, "change", "changes")
                        )),
                        cx,
                    );
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

    fn render_form(&mut self, cx: &mut Context<Self>) -> AnyElement {
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
                            "Cookie read from the clipboard ({}); the clipboard was cleared.",
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
                "HoYoLAB doesn’t show the weapons in your bag or your materials. Characters at \
                 a level cap (20, 40 … 80) count as not yet ascended, since HoYoLAB doesn’t say.",
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

    fn render_fetching(&mut self, cx: &mut Context<Self>) -> AnyElement {
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

    fn render_preview(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let library = self.library.read(cx);
        let (busy, planner) = (library.is_busy(), library.planner().clone());
        let Step::Preview { accounts, choice } = &self.step else {
            return div().into_any_element();
        };
        let Some(fetched) = accounts.get(*choice) else {
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
        let into = match &fetched.target {
            Some(account) => format!("into “{}”", account.label),
            None => format!("into a new account, “{}”", game.nickname),
        };
        let plan = &fetched.plan;
        let lines = change_lines(plan, &planner);
        let changes = plan.changes();
        div()
            .v_flex()
            .gap_3()
            .children(choices.map(|choices| div().h_flex().child(choices)))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("UID {} · {}, {into}", game.uid, game.nickname)),
            )
            .child(div().text_sm().child(summary(plan)))
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

    fn render_failed(&mut self, message: SharedString, cx: &mut Context<Self>) -> AnyElement {
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

/// "112 characters to add, 3 to raise · 112 weapons to add".
fn summary(plan: &SyncPlan) -> String {
    let count = |added: usize, raised: usize, what: &str| -> Option<String> {
        let mut parts = Vec::new();
        if added > 0 {
            parts.push(format!("{} to add", format::count(added as u64)));
        }
        if raised > 0 {
            parts.push(format!("{} to raise", format::count(raised as u64)));
        }
        (!parts.is_empty()).then(|| format!("{what}: {}", parts.join(", ")))
    };
    let (characters_added, characters_raised) = tally(&plan.characters);
    let (weapons_added, weapons_raised) = tally(&plan.weapons);
    let parts: Vec<String> = [
        count(characters_added, characters_raised, "Characters"),
        count(weapons_added, weapons_raised, "Weapons"),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        "Nothing to change.".to_owned()
    } else {
        parts.join(" · ")
    }
}

/// One line per character or weapon copy the sync adds or raises.
fn change_lines(plan: &SyncPlan, planner: &PlannerData) -> Vec<Div> {
    let level = |level: Level| format!("{}/{}", level.level, level.cap());
    let name = |key: &str| {
        planner
            .character(key)
            .map_or(key, |c| c.name.as_str())
            .to_owned()
    };
    let character = |c: &OwnedCharacter| {
        let [a, e, q] = c.talents;
        format!(
            "{} · {} · C{} · talents {a}/{e}/{q}",
            name(&c.key),
            level(c.level),
            c.constellation
        )
    };
    let weapon = |w: &OwnedWeapon| {
        let weapon = planner
            .weapon(&w.key)
            .map_or(w.key.as_str(), |w| w.name.as_str());
        let holder = w
            .holder
            .as_deref()
            .map_or(String::new(), |h| format!(" · held by {}", name(h)));
        format!("{weapon} · {} · R{}{holder}", level(w.level), w.refinement)
    };
    let line = |sign: &'static str, text: String| {
        div()
            .h_flex()
            .gap_2()
            .child(div().w(rems(1.)).flex_none().child(sign))
            .child(div().flex_1().min_w_0().child(text))
    };
    let characters = plan.characters.iter().filter_map(|change| match change {
        Change::Added(c) => Some(line("+", character(c))),
        Change::Raised { before, after } => Some(line(
            "↑",
            format!("{}  (was {})", character(after), character(before)),
        )),
        Change::Unchanged(_) => None,
    });
    let weapons = plan.weapons.iter().filter_map(|change| match change {
        Change::Added(w) => Some(line("+", weapon(w))),
        Change::Raised { before, after } => Some(line(
            "↑",
            format!("{}  (was {})", weapon(after), weapon(before)),
        )),
        Change::Unchanged(_) => None,
    });
    characters.chain(weapons).collect()
}

fn footer() -> Div {
    div().h_flex().justify_end().gap_2().pt_2()
}

/// How many of `changes` add, and how many raise.
fn tally<T>(changes: &[Change<T>]) -> (usize, usize) {
    changes
        .iter()
        .fold((0, 0), |(added, raised), change| match change {
            Change::Added(_) => (added + 1, raised),
            Change::Raised { .. } => (added, raised + 1),
            Change::Unchanged(_) => (added, raised),
        })
}
