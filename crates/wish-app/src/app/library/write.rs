//! The write queue: every database write, one at a time, in the order
//! made, then the reload (see `Library::write`).

use super::*;

impl Library {
    /// Runs `work` on the background executor with the store locked, then
    /// refreshes the mirror folder (when `changes_data`) and reloads. Back on
    /// the app thread it installs the reloaded state and calls `finish`.
    ///
    /// Writes run one at a time, in the order they were made: each waits for
    /// the one before it, not only for the store lock, which threads take in
    /// any order. So `+1` clicked twice in a row counts twice, and of two
    /// quick changes to one slider the later is the one kept.
    pub(super) fn write<T, R>(
        &mut self,
        cx: &mut Context<Self>,
        changes_data: bool,
        work: impl FnOnce(&mut Store, &GameData) -> Result<T> + Send + 'static,
        finish: impl FnOnce(&mut Self, T) -> R + 'static,
    ) -> Task<Result<R>>
    where
        T: Send + 'static,
        R: 'static,
    {
        let (Some(store), Some(data)) = (self.store.clone(), self.data.clone()) else {
            return Task::ready(Err(not_open()));
        };
        self.writes += 1;
        cx.notify();
        // The mirror is written a little after the last change, not by each
        // write (`Library::schedule_mirror`).
        let mirrored = changes_data && self.settings(cx).mirror_folder.is_some();
        let mirror_due = self.mirror_due.clone();
        let generations = self.generations.clone();
        let analyses = self.analyses.clone();
        let unfinished = self.unfinished.clone();
        unfinished.fetch_add(1, Ordering::SeqCst);

        let (ran, turn) = oneshot::channel();
        let before = self.last_write.replace(turn);

        // The write runs detached, so it finishes (and is counted out of
        // `writes`) even when the caller drops the returned task, say by
        // starting another flow. It is queued at once, not from the app
        // thread later, so quitting can wait for it.
        let (sent, outcome) = oneshot::channel();
        cx.background_spawn(async move {
            if let Some(before) = before {
                // Gone without a word only if it was cut off: go on.
                before.await.ok();
            }
            // A panic becomes an error: the app must not stay busy.
            // The write's transaction was rolled back as it unwound.
            let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let mut store = lock(&store);
                // Taken under the lock, so it orders the reloads.
                let generation = generations.fetch_add(1, Ordering::Relaxed) + 1;
                let value = work(&mut store, &data);
                if mirrored && value.is_ok() {
                    // Under the lock, like the mirror's own write.
                    mirror_due.store(true, Ordering::SeqCst);
                }
                // After a failed write too: it undoes what the page
                // showed at once.
                if value.is_err() {
                    // Prudent: everything is worked out again.
                    forget(&analyses);
                }
                let loaded = load(&mut store, &data, &analyses);
                (value, loaded, generation)
            }))
            .or_else(|panic| -> Result<_> {
                let error = anyhow!(t!("app-write-panicked", detail = panic_text(&*panic)));
                // Reload all the same: the page may show a change
                // the rolled-back write never made.
                let mut store = lock(&store);
                let generation = generations.fetch_add(1, Ordering::Relaxed) + 1;
                forget(&analyses);
                Ok((Err(error), load(&mut store, &data, &analyses), generation))
            });
            unfinished.fetch_sub(1, Ordering::SeqCst);
            ran.send(()).ok();
            sent.send(outcome).ok();
        })
        .detach();
        let (done, result) = oneshot::channel();
        cx.spawn(async move |this, cx| {
            let outcome = outcome
                .await
                .unwrap_or_else(|_| Err(anyhow!(t!("app-write-cut-off"))));
            let result = this.update(cx, |this, cx| {
                this.writes -= 1;
                let result = outcome.and_then(|(value, loaded, generation)| {
                    // A later write may have finished first; keep its state.
                    match loaded {
                        Ok(loaded) if generation > this.loaded_generation => {
                            this.loaded_generation = generation;
                            this.set_loaded(loaded);
                        }
                        Ok(_) => {}
                        Err(err) if value.is_ok() => return Err(err),
                        Err(_) => {}
                    }
                    if mirrored && value.is_ok() {
                        this.schedule_mirror(cx);
                    }
                    value.map(|value| finish(this, value))
                });
                cx.notify();
                result
            });
            // Nobody waiting for the result is fine.
            done.send(result.and_then(|result| result)).ok();
        })
        .detach();
        cx.spawn(async move |_, _| {
            result
                .await
                .unwrap_or_else(|_| Err(anyhow!(t!("app-write-window-closed"))))
        })
    }
}

/// What a panic said, for an error message.
fn panic_text(panic: &(dyn std::any::Any + Send)) -> String {
    (panic.downcast_ref::<&str>().map(|s| s.to_string()))
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| t!("app-write-panic"))
}

/// Drops every analysis kept, so the next reload works them all out again.
fn forget(analyses: &Mutex<Analyses>) {
    lock(analyses).clear();
}
