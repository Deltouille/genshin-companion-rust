//! The HoYoLAB login the system keyring keeps when the player asks, and the
//! chronicle synced with it on its own while the app is open.
//!
//! The login ([`hoyolab::Cookie::login`]) is in the keyring only: read for a
//! sync, dropped (and wiped) when its requests end. `settings.json` keeps
//! what goes with it, never the login: the game UIDs behind it and when the
//! chronicle last synced ([`HoyolabLink`]).
//!
//! A sync on its own asks for the chronicle alone and writes it without a
//! preview ([`service::hoyolab::keep_chronicle`]): it only adds periods or
//! brings one under way up to date, into the accounts that already have the
//! UID. The characters stay with the sync the player starts, which shows what
//! would change. It runs when the app opens and every half hour after, once
//! [`SYNC_EVERY`] has passed since the last; it stops at a captcha (until
//! the app opens again), and forgets a login HoYoLAB no longer accepts.

use chrono::Utc;
use wish_core::service::hoyolab::{ChronicleSync, Wanted};
use wish_core::settings::HoyolabLink;

use super::*;
use crate::app::{Notice, notify, report};

/// How long after a sync the chronicle syncs again on its own.
const SYNC_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// How often the app checks whether a sync is due.
const CHECK_EVERY: Duration = Duration::from_secs(30 * 60);

/// Whether the system keyring keeps a HoYoLAB login.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeptLogin {
    /// The keyring is being asked.
    Checking,
    /// This system has no keyring the app can use: nothing is kept.
    Unavailable,
    /// The keyring keeps none.
    None,
    /// The keyring keeps one: the chronicle syncs on its own.
    Kept,
}

/// The state of the syncs on their own.
#[derive(Default)]
pub(super) struct ChronicleSyncs {
    /// One is running.
    running: bool,
    /// HoYoLAB asked for a captcha: no more until the app opens again.
    paused: bool,
    /// Checks every [`CHECK_EVERY`] whether a sync is due, while a login is
    /// kept.
    timer: Option<Task<()>>,
}

impl Library {
    /// Whether the system keyring keeps a HoYoLAB login.
    pub fn kept_login(&self) -> KeptLogin {
        self.kept_login
    }

    /// Whether the chronicle is syncing with the login kept.
    pub fn is_syncing_chronicle(&self) -> bool {
        self.chronicle_syncs.running
    }

    /// Asks the keyring, once the data folder is open, whether it keeps a
    /// login; if it does, the chronicle syncs when due.
    pub(super) fn ask_keyring(&mut self, cx: &mut Context<Self>) {
        let keyring = self.services.keyring.clone();
        cx.spawn(async move |this, cx| {
            let kept = cx
                .background_spawn(async move {
                    if !keyring.available() {
                        return KeptLogin::Unavailable;
                    }
                    match keyring.load() {
                        Ok(Some(_)) => KeptLogin::Kept,
                        // A keyring that fails to answer is said in Settings
                        // as having nothing: the player can keep one again.
                        Ok(None) | Err(_) => KeptLogin::None,
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                this.kept_login = kept;
                if kept == KeptLogin::Kept {
                    this.watch_chronicle(cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Syncs the chronicle now if due, then checks again every
    /// [`CHECK_EVERY`].
    fn watch_chronicle(&mut self, cx: &mut Context<Self>) {
        self.sync_chronicle_if_due(cx);
        self.chronicle_syncs.timer = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(CHECK_EVERY).await;
                if this
                    .update(cx, |this, cx| this.sync_chronicle_if_due(cx))
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    fn sync_chronicle_if_due(&mut self, cx: &mut Context<Self>) {
        let syncs = &self.chronicle_syncs;
        if self.kept_login != KeptLogin::Kept || syncs.running || syncs.paused {
            return;
        }
        let last = self.settings(cx).hoyolab.synced_at;
        let due = last.is_none_or(|last| {
            (Utc::now() - last)
                .to_std()
                .is_ok_and(|since| since >= SYNC_EVERY)
        });
        if due {
            self.sync_chronicle(false, cx).detach();
        }
    }

    /// Keeps `login` ([`hoyolab::Cookie::login`]: never the cookie token) in
    /// the system keyring, for the game accounts `uids` behind it, which a
    /// sync the player confirmed just read: the chronicle then syncs on its
    /// own.
    pub fn remember_login(
        &mut self,
        login: hoyolab::Cookie,
        uids: Vec<String>,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let keyring = self.services.keyring.clone();
        cx.spawn(async move |this, cx| {
            cx.background_spawn(async move { keyring.save(&login.login()) })
                .await?;
            this.update(cx, |this, cx| {
                this.kept_login = KeptLogin::Kept;
                let link = HoyolabLink {
                    uids,
                    synced_at: Some(Utc::now()),
                };
                this.save_link(link, cx);
                this.watch_chronicle(cx);
                notify(Notice::done(t!("app-login-kept")), cx);
                cx.notify();
            })?;
            Ok(())
        })
    }

    /// Forgets the login the keyring keeps: the chronicle no longer syncs on
    /// its own.
    pub fn forget_login(&mut self, cx: &mut Context<Self>) -> Task<Result<()>> {
        let keyring = self.services.keyring.clone();
        cx.spawn(async move |this, cx| {
            cx.background_spawn(async move { keyring.forget() }).await?;
            this.update(cx, |this, cx| this.login_forgotten(cx))?;
            Ok(())
        })
    }

    fn login_forgotten(&mut self, cx: &mut Context<Self>) {
        if self.kept_login == KeptLogin::Kept {
            self.kept_login = KeptLogin::None;
        }
        self.chronicle_syncs.timer = None;
        self.save_link(HoyolabLink::default(), cx);
        cx.notify();
    }

    /// Forgets the login when no account is left with one of the UIDs
    /// behind it: the cookie goes with its account.
    pub(super) fn forget_login_without_account(&mut self, cx: &mut Context<Self>) {
        let uids = &self.settings(cx).hoyolab.uids;
        let linked = (self.accounts.iter())
            .any(|a| a.account.uid.as_ref().is_some_and(|uid| uids.contains(uid)));
        if self.kept_login == KeptLogin::Kept && !uids.is_empty() && !linked {
            report(self.forget_login(cx), t!("app-forget-login-failed"), cx);
        }
    }

    fn save_link(&mut self, link: HoyolabLink, cx: &mut Context<Self>) {
        let saved = self.preferences.update(cx, |preferences, cx| {
            preferences.change(|settings| settings.hoyolab = link, cx)
        });
        report(saved, t!("app-settings-save-failed"), cx);
    }

    /// Syncs the chronicle with the login kept now: the chronicle alone,
    /// written without a preview into the accounts with its UIDs. `asked`:
    /// the player asked for it (Settings), so the result is said even when
    /// nothing changed or HoYoLAB can't be reached.
    pub fn sync_chronicle(
        &mut self,
        asked: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<ChronicleSync>> {
        if self.chronicle_syncs.running {
            return Task::ready(Err(anyhow!(t!("app-chronicle-syncing"))));
        }
        self.chronicle_syncs.running = true;
        cx.notify();
        let keyring = self.services.keyring.clone();
        let transport = self.services.hoyolab.clone();
        let delay = self.services.pacing.delay;
        cx.spawn(async move |this, cx| {
            let fetched = cx
                .background_spawn(async move {
                    let Some(login) = keyring.load()? else {
                        return Err(anyhow!(t!("app-no-login-kept")));
                    };
                    let cancelled = AtomicBool::new(false);
                    let fetched = service::hoyolab::fetch(
                        login,
                        &*transport,
                        delay,
                        Utc::now(),
                        Wanted::Chronicle,
                        &cancelled,
                    )?;
                    Ok(fetched)
                })
                .await;
            let result = match fetched {
                Ok(fetched) => {
                    let write = this.update(cx, |this, cx| {
                        this.write(
                            cx,
                            true,
                            move |store, _| Ok(service::hoyolab::keep_chronicle(store, fetched)?),
                            |_, sync| sync,
                        )
                    })?;
                    write.await
                }
                Err(error) => Err(error),
            };
            this.update(cx, |this, cx| this.chronicle_synced(&result, asked, cx))?;
            result
        })
    }

    /// What a sync of the chronicle did: when it synced, a word if it kept
    /// something, and what HoYoLAB refused.
    fn chronicle_synced(
        &mut self,
        result: &Result<ChronicleSync>,
        asked: bool,
        cx: &mut Context<Self>,
    ) {
        self.chronicle_syncs.running = false;
        cx.notify();
        let hoyolab = |error: &anyhow::Error| match error.downcast_ref::<wish_core::Error>() {
            Some(wish_core::Error::Hoyolab(error)) => Some(matches!(
                error,
                hoyolab::Error::InvalidCookie | hoyolab::Error::MissingCookie(_)
            )),
            _ => None,
        };
        match result {
            Ok(sync) => {
                let link = HoyolabLink {
                    uids: sync.uids.clone(),
                    synced_at: Some(Utc::now()),
                };
                self.save_link(link, cx);
                let kept = &sync.kept;
                let (added, updated) = (
                    kept.abyss.added + kept.theater.added + kept.diary.added,
                    kept.abyss.updated + kept.theater.updated + kept.diary.updated,
                );
                if added + updated > 0 {
                    notify(
                        Notice::done(t!("app-chronicle-kept", added = added, updated = updated)),
                        cx,
                    );
                } else if asked {
                    notify(Notice::done(t!("app-chronicle-up-to-date")), cx);
                }
            }
            // HoYoLAB no longer accepts it: forgotten, and said.
            Err(error) if hoyolab(error) == Some(true) => {
                report(self.forget_login(cx), t!("app-forget-login-failed"), cx);
                notify(Notice::failed(t!("app-login-refused")), cx);
            }
            Err(error) => {
                if matches!(
                    error.downcast_ref::<wish_core::Error>(),
                    Some(wish_core::Error::Hoyolab(hoyolab::Error::Captcha))
                ) {
                    self.chronicle_syncs.paused = true;
                    notify(Notice::failure(t!("app-chronicle-paused"), error), cx);
                } else if asked {
                    notify(Notice::failure(t!("app-chronicle-failed"), error), cx);
                }
                // Otherwise (no network, HoYoLAB busy): the next check tries
                // again, without a word.
            }
        }
    }
}
