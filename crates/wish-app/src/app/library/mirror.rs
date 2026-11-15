//! The mirror folder: a snapshot written there after every change.

use super::*;

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
}
