//! Settings → Mirror folder: choose it or stop mirroring.

use crate::app::notices::{Notice, notify};

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
                        let message = format!("Couldn’t open the folder picker: {err:#}");
                        notify(Notice::failed(message), cx);
                    })
                    .ok();
                    None
                }
                Ok(Ok(None)) | Err(_) => None,
            };
            if let Some(folder) = folder {
                this.update_in(cx, |this, window, cx| {
                    this.set_mirror_folder(Some(folder), window, cx)
                })
                .ok();
            }
        });
        self.session
            .update(cx, |session, _| session.start_flow(flow));
    }

    pub(super) fn set_mirror_folder(
        &mut self,
        folder: Option<std::path::PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task = self
            .library
            .update(cx, |library, cx| library.set_mirror_folder(folder, cx));
        let flow = cx.spawn_in(window, async move |this, cx| {
            if let Err(err) = task.await {
                this.update_in(cx, |_, _, cx| {
                    let message = format!("Couldn’t change the mirror folder: {err:#}");
                    notify(Notice::failed(message), cx);
                })
                .ok();
            }
        });
        self.session
            .update(cx, |session, _| session.start_flow(flow));
    }
}
