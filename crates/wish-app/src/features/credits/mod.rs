//! Credits: paimon.moe, which the wish counter remakes, and genshin-db, the
//! planner's data (each with its MIT notice); HoYoverse, who own the game's art
//! and names; the other parts used; then the licences of every open-source
//! component the app is built with.

use std::sync::LazyLock;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::link::Link;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _};
use gpui_kit::{
    App, FontWeight, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _,
    UniformListScrollHandle, Window, div, rems, uniform_list,
};

use wish_ui::{PageFrame, button, card, dialog_close_footer};

/// The licence of every crate the app is built with, then those of the
/// Lucide icons and the Google Sans Flex font (`tools/third-party-licenses.sh`). Every
/// release archive holds the same file.
const THIRD_PARTY_LICENSES: &str = include_str!("../../../licenses/THIRD-PARTY-LICENSES.txt");

/// Its lines, the long ones wrapped: the dialog draws only those in view,
/// which needs them all one line high.
static LICENSE_LINES: LazyLock<Vec<SharedString>> = LazyLock::new(|| {
    (THIRD_PARTY_LICENSES.lines())
        .flat_map(|line| wrap(line, 96))
        .map(SharedString::from)
        .collect()
});

/// paimon.moe's licence, which its notice must travel with (MIT).
const PAIMON_MOE_LICENSE: &str = include_str!("../../../licenses/paimon-moe-MIT.txt");
/// genshin-db's licence, for the planner's data (MIT).
const GENSHIN_DB_LICENSE: &str = include_str!("../../../licenses/genshin-db-MIT.txt");

/// The Credits page. It has no state of its own, so it is a plain component
/// (`RenderOnce`), built afresh each time it is shown.
#[derive(IntoElement)]
pub struct Credits;

impl RenderOnce for Credits {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let license = |text: &str| {
            div()
                .p_4()
                .rounded(theme.radius)
                .bg(theme.muted)
                .text_xs()
                .font_family(theme.mono_font_family.clone())
                .text_color(theme.muted_foreground)
                .child(text.trim().to_owned())
        };

        let paimon = section(
            "paimon.moe",
            "Genshin Companion's wish counter is a desktop remake of paimon.moe’s, by Made Baruna. \
             Its banner and item data and its pity and rate-up rules come from the paimon.moe \
             repository, used under the MIT License below. The item icons and banner art are \
             downloaded from paimon.moe when first shown.",
            Some(("paimon-moe", "https://github.com/MadeBaruna/paimon-moe")),
            cx,
        )
        .child(license(PAIMON_MOE_LICENSE));

        let genshin_db = section(
            "genshin-db",
            "The character archive (profiles, stats, talents, constellations) and the planner's \
             data (ascension and talent costs, farming days) come from genshin-db, by theBowja, \
             used under the MIT License below.",
            Some(("genshin-db", "https://github.com/theBowja/genshin-db")),
            cx,
        )
        .child(license(GENSHIN_DB_LICENSE));

        let hoyoverse = section(
            "HoYoverse",
            "Genshin Impact, its characters, weapons and banner art, and all other game content \
             and materials are trademarks and copyrights of HoYoverse. Genshin Companion is a fan \
             project, not affiliated with or endorsed by HoYoverse. It ships none of the game's \
             images: they are downloaded when first shown, and kept on this computer.",
            None,
            cx,
        );

        let others = card(cx).p_4().gap_3().children(
            [
                (
                    "Project Amber",
                    "Character, weapon, material, element, talent and constellation icons and the \
                     wish art of the archive and the planner, downloaded from its copy of the \
                     game's images when first shown.",
                    "https://gi.yatta.moe",
                ),
                (
                    "AnimeGameData",
                    "The EXP needed per character and weapon level, from the game's own data.",
                    "https://gitlab.com/Dimbreath/AnimeGameData",
                ),
                (
                    "UIGF",
                    "Item ids from UIGF’s public dictionary, for exports other trackers read.",
                    "https://uigf.org",
                ),
                (
                    "Google Sans Flex",
                    "Typeface by the Google Sans Flex Authors, under the SIL Open Font License 1.1.",
                    "https://github.com/googlefonts/googlesans-flex",
                ),
                (
                    "Lucide",
                    "Icons by Lucide Icons and Contributors, under the ISC License.",
                    "https://lucide.dev",
                ),
                (
                    "GPUI Kit",
                    "The interface toolkit, under the Apache License 2.0.",
                    "https://gpui-kit.com",
                ),
            ]
            .map(|(name, what, url)| {
                div()
                    .v_flex()
                    .gap_0p5()
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(name))
                            .child(link(name, url)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(what),
                    )
            }),
        );

        // The licences named in the file's overview, with how many crates use each.
        let used = (THIRD_PARTY_LICENSES.lines())
            .skip_while(|line| !line.starts_with("- "))
            .take_while(|line| line.starts_with("- "))
            .map(|line| line.trim_start_matches("- ").to_owned())
            .collect::<Vec<_>>()
            .join(", ");
        let components = card(cx)
            .p_4()
            .gap_3()
            .child(div().child(
                "Genshin Companion is built with open-source Rust crates, each under its own \
                 licence, and embeds the Lucide icons and the Google Sans Flex font. Their notices and \
                 licence texts are here, and in THIRD-PARTY-LICENSES.txt next to the app in \
                 its download.",
            ))
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(used),
            )
            .child(
                // A row, so the button keeps its own width.
                div().h_flex().child(
                    button("third-party-licenses")
                        .label("Show the licences…")
                        .on_click(|_, window, cx| open_licenses(window, cx)),
                ),
            );

        let body = div()
            .v_flex()
            .gap_6()
            .max_w(rems(48.))
            .child(paimon)
            .child(genshin_db)
            .child(hoyoverse)
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(group_title("Also used", cx))
                    .child(others),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(group_title("Open-source components", cx))
                    .child(components),
            );
        PageFrame::new("Credits", body)
    }
}

/// The full licence texts, in a dialog. Some 15,000 lines: a uniform list
/// lays out only those in view.
pub(crate) fn open_licenses(window: &mut Window, cx: &mut App) {
    let scroll = UniformListScrollHandle::new();
    window.open_dialog(cx, move |dialog, window, cx| {
        let theme = cx.theme();
        let (mono, color) = (theme.mono_font_family.clone(), theme.foreground);
        let rem = window.rem_size();
        let lines = uniform_list(
            "third-party-licenses",
            LICENSE_LINES.len(),
            move |range, _, _| {
                range
                    .map(|ix| {
                        div()
                            .h(rem * 1.25)
                            .text_xs()
                            .font_family(mono.clone())
                            .text_color(color)
                            .whitespace_nowrap()
                            .child(LICENSE_LINES[ix].clone())
                    })
                    .collect()
            },
        )
        .track_scroll(&scroll)
        .size_full();
        dialog
            .title("Open-source licences")
            .w(rem * 52.)
            .child(
                div()
                    .relative()
                    .h(rem * 32.)
                    .p_3()
                    .rounded(theme.radius)
                    .bg(theme.muted)
                    .child(lines)
                    .vertical_scrollbar(&scroll),
            )
            .footer(dialog_close_footer("Close"))
    });
}

/// `line` cut at spaces into lines of `width` characters at most (a word
/// longer than that stays whole).
fn wrap(line: &str, width: usize) -> Vec<String> {
    if line.chars().count() <= width {
        return vec![line.to_owned()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in line.split(' ') {
        let len = current.chars().count();
        if len > 0 && len + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    lines.push(current);
    lines
}

/// A titled card: what is credited, and a link to it when there is one.
fn section(
    title: &'static str,
    text: &'static str,
    link_to: Option<(&'static str, &'static str)>,
    cx: &App,
) -> gpui_kit::Div {
    card(cx)
        .p_4()
        .gap_3()
        .child(
            div()
                .h_flex()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .child(title),
                )
                .children(link_to.map(|(id, url)| link(id, url))),
        )
        .child(div().child(text))
}

fn group_title(title: &'static str, cx: &App) -> impl IntoElement {
    div()
        .px_1()
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(title)
}

/// A link to a web page (external, so a link rather than a button).
fn link(id: &str, url: &'static str) -> Link {
    Link::new(SharedString::from(format!("credit-{id}")))
        .href(url)
        .text_sm()
        .child(url.trim_start_matches("https://"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_lines_wrap_at_spaces() {
        assert_eq!(wrap("short", 10), ["short"]);
        assert_eq!(wrap("one two three four", 9), ["one two", "three", "four"]);
        assert_eq!(
            wrap("averyveryverylongword x", 5),
            ["averyveryverylongword", "x"]
        );
    }

    #[test]
    fn the_licences_name_every_part_embedded() {
        for part in [
            "MIT License",
            "Apache License",
            "Lucide icons",
            "Google Sans Flex font",
        ] {
            assert!(THIRD_PARTY_LICENSES.contains(part), "{part}");
        }
        assert!(
            LICENSE_LINES
                .iter()
                .all(|line| line.chars().count() <= 96 || !line.contains(' '))
        );
    }
}
