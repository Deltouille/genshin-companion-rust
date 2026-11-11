//! Settings → Images: the downloaded images (how many, how big, empty the
//! cache), and downloading them all for offline use, icons only or everything,
//! with its progress.

use crate::app::notices::{Notice, notify};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{Context, Div, ParentElement as _, Styled as _, Window, div};
use wish_core::images::Collection;

use super::{SettingsPage, group, help, row};
use crate::app::library::DownloadState;
use wish_ui::{ButtonExt as _, button, dialog_footer, format};

impl SettingsPage {
    pub(super) fn images_group(&mut self, cx: &mut Context<Self>) -> Div {
        let library = self.library.read(cx);
        let (files, bytes) = library.images().usage();
        let download = library.image_download().cloned();
        let running = download.as_ref().is_some_and(|d| d.is_running());
        let missing = |collection| library.missing_images(collection);
        let (icons, everything) = (missing(Collection::Icons), missing(Collection::Everything));
        let theme = cx.theme();

        let cache = row(
            "Downloaded images",
            help(
                format!(
                    "{} {}, {}. Pages download the images they show, once, from paimon.moe \
                     and Project Amber; they stay on this computer.",
                    format::count(files as u64),
                    format::plural(files as u32, "image", "images"),
                    format::bytes(bytes)
                ),
                cx,
            ),
            button("empty-image-cache")
                .ghost()
                .label("Empty…")
                .unavailable(files == 0 || running)
                .on_click(cx.listener(|this, _, window, cx| this.confirm_empty_cache(window, cx))),
        );

        let detail = match &download {
            Some(download) if download.is_running() => {
                let shown = if download.state == DownloadState::Stopping {
                    "Stopping…".to_owned()
                } else {
                    format!(
                        "Downloading {} of {}…",
                        format::count(download.done as u64),
                        format::count(download.total as u64)
                    )
                };
                let share = download.done as f32 / download.total.max(1) as f32;
                div().v_flex().gap_2().child(help(shown, cx)).child(
                    Progress::new("image-download-progress")
                        .value(share * 100.)
                        .accessibility_label("Images downloaded"),
                )
            }
            other => {
                let outcome = other.as_ref().and_then(|download| {
                    let count = |n: usize| format::count(n as u64);
                    let lacking = match download.not_found {
                        0 => String::new(),
                        n => format!(" ({} not on paimon.moe or Project Amber)", count(n)),
                    };
                    Some(match download.state {
                        DownloadState::Done if download.failed == 0 => {
                            (format!("Every image is downloaded{lacking}."), false)
                        }
                        DownloadState::Done => (
                            format!(
                                "{} couldn’t be downloaded{lacking}: try again later.",
                                count(download.failed)
                            ),
                            true,
                        ),
                        DownloadState::Stopped => (
                            format!(
                                "Stopped: {} of {} downloaded.",
                                count(download.stored),
                                count(download.total)
                            ),
                            false,
                        ),
                        DownloadState::Unreachable => (
                            "Couldn’t reach paimon.moe or Project Amber. Check the connection, \
                             then try again."
                                .to_owned(),
                            true,
                        ),
                        DownloadState::Running | DownloadState::Stopping => return None,
                    })
                });
                div()
                    .v_flex()
                    .gap_0p5()
                    .child(help(
                        "Download them now, so that every page has its pictures without a \
                         connection. Icons cover the wish counter and the planner; everything \
                         adds the banner art and the characters' wish art.",
                        cx,
                    ))
                    .children(outcome.map(|(text, problem)| {
                        help(text, cx).when(problem, |this| this.text_color(theme.danger))
                    }))
            }
        };
        let offer = |id: &'static str, label: &str, (count, size): (usize, u64)| {
            let text = if count == 0 {
                format!("{label}: done")
            } else {
                format!("{label} (~{})", format::bytes(size))
            };
            button(id).label(text).unavailable(count == 0)
        };
        let controls = if running {
            div().child(
                button("stop-image-download")
                    .label("Stop")
                    .unavailable(
                        download.as_ref().map(|d| d.state) == Some(DownloadState::Stopping),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.library
                            .update(cx, |library, cx| library.stop_image_download(cx))
                    })),
            )
        } else {
            div()
                .h_flex()
                .gap_2()
                .child(offer("download-icons", "Icons", icons).on_click(
                    cx.listener(|this, _, _, cx| this.download_all_images(Collection::Icons, cx)),
                ))
                .child(
                    offer("download-everything", "Everything", everything)
                        .primary()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.download_all_images(Collection::Everything, cx)
                        })),
                )
        };
        let offline = row("For offline use", detail, controls);

        group("Images", vec![cache, offline], cx)
    }

    fn download_all_images(&mut self, collection: Collection, cx: &mut Context<Self>) {
        self.library.update(cx, |library, cx| {
            library.download_all_images(collection, cx)
        });
    }

    fn confirm_empty_cache(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let page = page.clone();
            alert
                .title("Empty the downloaded images?")
                .description(
                    "They are downloaded again when pages show them. None of your data is \
                     touched.",
                )
                .footer(dialog_footer(button("confirm").danger().label("Empty")))
                .on_ok(move |_, window, cx| {
                    page.update(cx, |page, cx| page.empty_cache(window, cx))
                        .ok();
                    true
                })
        });
    }

    fn empty_cache(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let task = self
            .library
            .update(cx, |library, cx| library.clear_image_cache(cx));
        cx.spawn_in(window, async move |this, cx| {
            if let Err(err) = task.await {
                this.update_in(cx, |_, _, cx| {
                    let message = format!("Couldn’t empty the downloaded images: {err:#}");
                    notify(Notice::failed(message), cx);
                })
                .ok();
            }
        })
        .detach();
    }
}
