//! Messages about work done in the background ("Backup saved.", "Couldn’t
//! export: …"), shown as Material 3 snackbars under the page.
//!
//! GPUI Kit's toasts sit at the top right, over the pages' own buttons, and
//! show their close button on hover in a colour that can't be changed, which
//! an M3 snackbar's inverse surface would hide. So the app keeps its own:
//! [`Notices`], one per window, which the shell draws above everything (the
//! dialogs too), and [`notify`] to show one from anywhere.

use std::time::Duration;

use gpui_kit::base::StyledExt as _;
use gpui_kit::{
    App, AppContext as _, Context, Entity, Global, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Task, Window, div, rems,
};

/// What went right or wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    /// Work done: it goes away by itself.
    Done,
    /// Work that failed: it stays until closed, so it can't be missed.
    Failed,
}

/// A message for [`notify`].
#[derive(Clone, Debug)]
pub struct Notice {
    pub kind: NoticeKind,
    pub message: SharedString,
}

impl Notice {
    pub fn done(message: impl Into<SharedString>) -> Self {
        Self {
            kind: NoticeKind::Done,
            message: message.into(),
        }
    }

    pub fn failed(message: impl Into<SharedString>) -> Self {
        Self {
            kind: NoticeKind::Failed,
            message: message.into(),
        }
    }

    /// How long a [`NoticeKind::Done`] stays: 4 s, longer for a long
    /// message (Material 3 allows 4 to 10 s).
    fn duration(&self) -> Duration {
        let extra = self.message.chars().count().saturating_sub(50) as u64 * 40;
        Duration::from_millis((4_000 + extra).min(10_000))
    }
}

/// Shows `notice` at the bottom of the window. Before the window exists (or
/// in a test without one), it does nothing.
pub fn notify(notice: Notice, cx: &mut App) {
    if let Some(notices) = cx.try_global::<CurrentNotices>().map(|n| n.0.clone()) {
        notices.update(cx, |notices, cx| notices.push(notice, cx));
    }
}

/// The window's notices, as a global for [`notify`].
struct CurrentNotices(Entity<Notices>);

impl Global for CurrentNotices {}

/// At most this many notices show at once; a newer one pushes the oldest out.
const MOST_SHOWN: usize = 3;

/// The notices shown, oldest first. Created by the shell, which draws it.
pub struct Notices {
    shown: Vec<Shown>,
    next_id: u64,
}

struct Shown {
    id: u64,
    notice: Notice,
    /// Hides a [`NoticeKind::Done`] when it fires; dropped with the notice.
    _hide: Option<Task<()>>,
}

impl Notices {
    /// Creates the window's notices and makes [`notify`] show them.
    pub fn new(cx: &mut App) -> Entity<Self> {
        let notices = cx.new(|_| Self {
            shown: Vec::new(),
            next_id: 0,
        });
        cx.set_global(CurrentNotices(notices.clone()));
        notices
    }

    /// The messages shown, oldest first.
    pub fn messages(&self) -> Vec<SharedString> {
        self.shown
            .iter()
            .map(|shown| shown.notice.message.clone())
            .collect()
    }

    fn push(&mut self, notice: Notice, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;
        let hide = (notice.kind == NoticeKind::Done).then(|| {
            let duration = notice.duration();
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(duration).await;
                this.update(cx, |this, cx| this.dismiss(id, cx)).ok();
            })
        });
        self.shown.push(Shown {
            id,
            notice,
            _hide: hide,
        });
        if self.shown.len() > MOST_SHOWN {
            self.shown.remove(0);
        }
        cx.notify();
    }

    /// Hides them all.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.shown.clear();
        cx.notify();
    }

    fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        self.shown.retain(|shown| shown.id != id);
        cx.notify();
    }
}

impl Render for Notices {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notices = cx.entity().downgrade();
        let bars = self.shown.iter().map(|shown| {
            let (id, notices) = (shown.id, notices.clone());
            wish_ui::snackbar(
                ("notice", id),
                shown.notice.message.clone(),
                move |_, _, cx| {
                    notices
                        .update(cx, |notices, cx| notices.dismiss(id, cx))
                        .ok();
                },
                cx,
            )
        });
        // Centred under the page, beside the sidebar, 24 px above the
        // window's edge; the newest lowest, all as wide as the widest.
        div()
            .absolute()
            .bottom_6()
            .left(rems(crate::shell::SIDEBAR_REMS))
            .right_0()
            .px_6()
            .h_flex()
            .justify_center()
            .child(div().v_flex().items_stretch().gap_2().children(bars))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_messages_stay_longer_up_to_ten_seconds() {
        assert_eq!(Notice::done("Backup saved.").duration().as_secs(), 4);
        let long = Notice::done("x".repeat(150));
        assert_eq!(long.duration(), Duration::from_millis(8_000));
        assert_eq!(Notice::done("x".repeat(500)).duration().as_secs(), 10);
    }
}
