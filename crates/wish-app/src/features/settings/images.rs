//! Settings → Images: the downloaded images (how many, how big, empty the
//! cache), and downloading them all for offline use, icons only or everything,
//! with its progress.

use crate::app::notices::report;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::progress::Progress;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{Context, Div, ParentElement as _, Styled as _, Window, div};
use wish_core::images::Collection;

use super::{SettingsPage, group, help, row};
use crate::app::library::DownloadState;
use wish_ui::{ButtonExt as _, button, confirm_destructive, format};

impl SettingsPage {
    pub(super) fn images_group(&mut self, cx: &mut Context<Self>) -> Div {
        let service = self.library.read(cx).image_service().read(cx);
        let (files, bytes) = service.images().usage();
        let download = service.download().cloned();
        let running = download.as_ref().is_some_and(|d| d.is_running());
        let missing = |collection| service.missing(collection);
        let (icons, everything) = (missing(Collection::Icons), missing(Collection::Everything));
        let theme = cx.theme();

        let cache = row(
            t!("settings-images-downloaded"),
            help(
                t!(
                    "settings-images-help",
                    count = files,
                    size = format::bytes(bytes)
                ),
                cx,
            ),
            button("empty-image-cache")
                .ghost()
                .label(t!("settings-images-empty"))
                .unavailable(files == 0 || running)
                .on_click(cx.listener(|this, _, window, cx| this.confirm_empty_cache(window, cx))),
        );

        let detail = match &download {
            Some(download) if download.is_running() => {
                let shown = if download.state == DownloadState::Stopping {
                    t!("settings-images-stopping")
                } else {
                    t!(
                        "settings-images-downloading",
                        done = download.done,
                        total = download.total
                    )
                };
                let share = download.done as f32 / download.total.max(1) as f32;
                div().v_flex().gap_2().child(help(shown, cx)).child(
                    Progress::new("image-download-progress")
                        .value(share * 100.)
                        .accessibility_label(t!("settings-images-progress")),
                )
            }
            other => {
                let outcome = other.as_ref().and_then(|download| {
                    let missing = download.not_found;
                    Some(match download.state {
                        DownloadState::Done if download.failed == 0 => {
                            (t!("settings-images-done", missing = missing), false)
                        }
                        DownloadState::Done => (
                            t!(
                                "settings-images-failed",
                                failed = download.failed,
                                missing = missing
                            ),
                            true,
                        ),
                        DownloadState::Stopped => (
                            t!(
                                "settings-images-stopped",
                                stored = download.stored,
                                total = download.total
                            ),
                            false,
                        ),
                        DownloadState::Unreachable => (t!("settings-images-unreachable"), true),
                        DownloadState::Running | DownloadState::Stopping => return None,
                    })
                });
                div()
                    .v_flex()
                    .gap_0p5()
                    .child(help(t!("settings-images-offline-help"), cx))
                    .children(outcome.map(|(text, problem)| {
                        help(text, cx).when(problem, |this| this.text_color(theme.danger))
                    }))
            }
        };
        let offer = |id: &'static str, label: String, (count, size): (usize, u64)| {
            let text = if count == 0 {
                t!("settings-images-offer-done", what = label)
            } else {
                t!(
                    "settings-images-offer-size",
                    what = label,
                    size = format::bytes(size)
                )
            };
            button(id).label(text).unavailable(count == 0)
        };
        let controls = if running {
            div().child(
                button("stop-image-download")
                    .label(t!("settings-images-stop"))
                    .unavailable(
                        download.as_ref().map(|d| d.state) == Some(DownloadState::Stopping),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        let service = this.library.read(cx).image_service().clone();
                        service.update(cx, |service, cx| service.stop_download(cx))
                    })),
            )
        } else {
            div()
                .h_flex()
                .gap_2()
                .child(
                    offer("download-icons", t!("settings-images-icons"), icons).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.download_all_images(Collection::Icons, cx)
                        }),
                    ),
                )
                .child(
                    offer(
                        "download-everything",
                        t!("settings-images-everything"),
                        everything,
                    )
                    .primary()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.download_all_images(Collection::Everything, cx)
                    })),
                )
        };
        let offline = row(t!("settings-images-offline"), detail, controls);

        group(t!("settings-group-images"), vec![cache, offline], cx)
    }

    fn download_all_images(&mut self, collection: Collection, cx: &mut Context<Self>) {
        let service = self.library.read(cx).image_service().clone();
        service.update(cx, |service, cx| service.download_all(collection, cx));
    }

    fn confirm_empty_cache(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        confirm_destructive(
            window,
            cx,
            t!("settings-images-empty-title"),
            t!("settings-images-empty-help"),
            t!("settings-images-empty-button"),
            move |_, cx| {
                page.update(cx, |page, cx| page.empty_cache(cx)).ok();
            },
        );
    }

    fn empty_cache(&mut self, cx: &mut Context<Self>) {
        let service = self.library.read(cx).image_service().clone();
        let task = service.update(cx, |service, cx| service.clear_cache(cx));
        report(task, t!("settings-images-empty-failed"), cx);
    }
}
