//! The game's day, which decides the domains open for "Farm today": it
//! starts at 04:00 in the account's server time.

use std::time::Duration;

use chrono::{FixedOffset, Local, Utc};
use gpui_kit::{App, Context, Task};
use wish_core::model::Server;
use wish_core::planner::{game_day, next_game_day};

use super::PlannerView;
use super::model::GameDay;

impl PlannerView {
    /// The game's day for the account shown: in its server's time, else in
    /// the player's.
    pub(super) fn game_day(&self, cx: &App) -> GameDay {
        let server = (self.account(cx))
            .and_then(|id| self.library.read(cx).account(id))
            .and_then(|summary| summary.account.server);
        GameDay {
            day: game_day(now_on(server)),
            server,
        }
    }

    /// Shows the new game day's domains when it starts. It looks again every
    /// ten minutes at most: a computer put to sleep doesn't count the time
    /// asleep in its timers.
    pub(super) fn watch_the_day(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(wait) = this.update(cx, |this, cx| {
                    if this.game_day(cx) != this.day {
                        this.rebuild(cx);
                        cx.notify();
                    }
                    let now = now_on(this.day.server);
                    (next_game_day(now) - now).to_std().unwrap_or_default()
                }) else {
                    break;
                };
                let wait = (wait + Duration::from_secs(1)).min(Duration::from_secs(600));
                cx.background_executor().timer(wait).await;
            }
        })
    }
}

/// Now, in the server's time, else in the player's.
pub(super) fn now_on(server: Option<Server>) -> chrono::DateTime<FixedOffset> {
    match server.and_then(|server| FixedOffset::east_opt(server.utc_offset_hours() * 3600)) {
        Some(offset) => Utc::now().with_timezone(&offset),
        None => Local::now().fixed_offset(),
    }
}
