//! The mirror folder: a snapshot of everything, written there a few seconds
//! after the last change (`MIRROR_DELAY`), and when the app quits. The
//! changes of those seconds (a slider dragged, a few clicks) make one write,
//! not one each, and a syncing tool sees the file change once. The database
//! is written at once, in a transaction: the mirror is a second copy.

use super::*;

/// How long the mirror waits after a change; each change starts it again.
pub(super) const MIRROR_DELAY: Duration = Duration::from_secs(3);

impl Library {
    /// Saves the mirror folder setting and, for a folder, writes the
    /// snapshot right away so the folder is never stale.
    pub fn set_mirror_folder(
        &mut self,
        folder: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let saved = self.preferences.update(cx, |preferences, cx| {
            preferences.change(|settings| settings.mirror_folder = folder.clone(), cx)
        });
        let mirrored = self.write(
            cx,
            false,
            move |store, _| Ok(folder.map(|folder| write_mirror(store, &folder))),
            |this, mirror| this.mirror = mirror,
        );
        cx.background_spawn(async move {
            saved.await?;
            mirrored.await
        })
    }

    /// Writes the mirror `MIRROR_DELAY` after the last change: a change
    /// before then starts the wait again (the timer before it is dropped).
    pub(super) fn schedule_mirror(&mut self, cx: &mut Context<Self>) {
        self.mirror_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(MIRROR_DELAY).await;
            this.update(cx, |this, cx| this.flush_mirror(cx)).ok();
        }));
    }

    /// Writes the mirror, in the background, if a change awaits it.
    fn flush_mirror(&mut self, cx: &mut Context<Self>) {
        let (Some(store), Some(folder)) =
            (self.store.clone(), self.settings(cx).mirror_folder.clone())
        else {
            return;
        };
        let due = self.mirror_due.clone();
        cx.spawn(async move |this, cx| {
            let written = cx
                .background_spawn(async move { write_mirror_if_due(&store, &folder, &due) })
                .await;
            if let Some(status) = written {
                this.update(cx, |this, cx| {
                    this.mirror = Some(status);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
}

/// Writes the mirror if a change awaits it. The store stays locked while it
/// writes: a change can't slip in between, and quitting, which takes the
/// lock too, waits for a write under way.
pub(super) fn write_mirror_if_due(
    store: &Mutex<Store>,
    folder: &Path,
    due: &AtomicBool,
) -> Option<MirrorStatus> {
    let store = lock(store);
    due.swap(false, Ordering::SeqCst)
        .then(|| write_mirror(&store, folder))
}
