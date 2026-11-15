//! Settings → Mirror folder: choose it or stop mirroring.

use crate::app::notices::{Notice, notify, report};

use gpui_kit::{Context, PathPromptOptions, SharedString, Window};

use super::SettingsPage;

impl SettingsPage {
    pub(super) fn choose_mirror_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(SharedString::from("Mirror here")),
        });
        let flow = cx.spawn_in(window, async move |this, cx| {
            let folder = match picked.await {
                Ok(Ok(Some(mut folders))) => folders.pop(),
                Ok(Err(err)) => {
                    this.update_in(cx, |_, _, cx| {
                        notify(Notice::failure("Couldn’t open the folder picker", &err), cx);
                    })
                    .ok();
                    None
                }
                Ok(Ok(None)) | Err(_) => None,
            };
            if let Some(folder) = folder {
                this.update_in(cx, |this, _, cx| this.set_mirror_folder(Some(folder), cx))
                    .ok();
            }
        });
        self.session
            .update(cx, |session, _| session.start_flow(flow));
    }

    pub(super) fn set_mirror_folder(
        &mut self,
        folder: Option<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let task = self
            .library
            .update(cx, |library, cx| library.set_mirror_folder(folder, cx));
        report(task, "Couldn’t change the mirror folder", cx);
    }
}
