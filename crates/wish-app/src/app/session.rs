//! The window's shared context: which account is on screen, and the one flow
//! (import, export, restore…) in progress.
//!
//! Why an entity of its own: several independent parts need the selected
//! account (the sidebar's picker, the overview, the history page, settings'
//! spreadsheet export, the import flow that selects what it imported). Each
//! holds an `Entity<Session>`, reads it, and observes it to redraw. None
//! changes it by hand: they call its methods, which keep its rules:
//!
//! - the account is always one the library still has (or none), preferring
//!   the current one when the data changes;
//! - there is one flow at a time: starting one drops, so cancels, the
//!   previous, before it can open a stale dialog. Writes themselves are
//!   queued by [`Library`] and always finish; a cancelled flow only loses
//!   its next step (a dialog, a notification).

use gpui_kit::{App, Context, Entity, Subscription, Task};
use wish_core::model::{AccountId, Pool};

use super::library::{AccountSummary, Library};

pub struct Session {
    library: Entity<Library>,
    account: Option<AccountId>,
    flow: Option<Task<()>>,
    _library_changes: Subscription,
}

impl Session {
    pub fn new(library: Entity<Library>, cx: &mut Context<Self>) -> Self {
        let library_changes = cx.observe(&library, |this, _, cx| this.keep_account_valid(cx));
        let mut session = Self {
            library,
            account: None,
            flow: None,
            _library_changes: library_changes,
        };
        session.keep_account_valid(cx);
        session
    }

    pub fn library(&self) -> &Entity<Library> {
        &self.library
    }

    /// The account whose wishes are shown.
    pub fn account(&self) -> Option<AccountId> {
        self.account
    }

    /// Shows `id`'s wishes (the picker, an import that added them).
    pub fn select_account(&mut self, id: AccountId, cx: &mut Context<Self>) {
        if self.account != Some(id) && self.library.read(cx).account(id).is_some() {
            self.account = Some(id);
            cx.notify();
        }
    }

    /// The selected account with its analysis.
    pub fn summary<'a>(&self, cx: &'a App) -> Option<&'a AccountSummary> {
        self.library.read(cx).account(self.account?)
    }

    /// Banner types with wishes for the selected account, in display order.
    /// While counting by hand, every type, so that counting can start on an
    /// empty one.
    pub fn pools(&self, cx: &App) -> Vec<Pool> {
        if self.library.read(cx).settings().manual_counting {
            return Pool::ALL.to_vec();
        }
        self.summary(cx)
            .map(|s| s.analysis.pools.iter().map(|p| p.pool).collect())
            .unwrap_or_default()
    }

    /// Makes `flow` the flow in progress, cancelling the previous one.
    pub fn start_flow(&mut self, flow: Task<()>) {
        self.flow = Some(flow);
    }

    /// After the data changed: keeps the selection when the account still
    /// exists, else selects the first one (or none).
    fn keep_account_valid(&mut self, cx: &mut Context<Self>) {
        let library = self.library.read(cx);
        let account = self
            .account
            .filter(|id| library.account(*id).is_some())
            .or_else(|| library.accounts().first().map(|a| a.account.id));
        self.account = account;
        // The library changed, so what the selection shows changed too.
        cx.notify();
    }
}
