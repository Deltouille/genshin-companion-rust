//! The app's look, after the game's menus (without their assets): ink-blue
//! slate panels on a plain page, parchment text and pills, gold for ornaments
//! and what is selected; in light, a parchment page beside a slate sidebar.
//! Its light and dark themes are in `theme.json`, with the colour roles the
//! kit's tokens don't have (the `app` sections, read here as a [`Palette`]).
//!
//! Every raw colour lives in `theme.json`. Rarity colours follow the game's
//! convention (5★ gold, 4★ purple); they are always paired with the written
//! rarity ("5★"), never the only cue.

use std::borrow::Cow;
use std::sync::LazyLock;

use gpui_kit::component::{ActiveTheme as _, Theme, ThemeRegistry, try_parse_color};
use gpui_kit::{App, Background, Hsla, Window, linear_color_stop, linear_gradient};
use wish_core::model::Rarity;

const THEMES: &str = include_str!("theme.json");
const LIGHT: &str = "Genshin Companion Light";
const DARK: &str = "Genshin Companion Dark";

/// Google Sans Flex (SIL Open Font License, `fonts/GoogleSansFlex-OFL.txt`),
/// in the weights the pages use: static instances of its variable font (opsz
/// 18, wdth 100, ROND 0), which GPUI can't vary itself.
const FONTS: [&[u8]; 5] = [
    include_bytes!("../fonts/GoogleSansFlex-Regular.ttf"),
    include_bytes!("../fonts/GoogleSansFlex-Medium.ttf"),
    include_bytes!("../fonts/GoogleSansFlex-SemiBold.ttf"),
    include_bytes!("../fonts/GoogleSansFlex-Bold.ttf"),
    include_bytes!("../fonts/GoogleSansFlex-Black.ttf"),
];

/// Registers the app's typeface and themes, and switches to the theme
/// matching the system.
pub fn install(cx: &mut App) {
    cx.text_system()
        .add_fonts(FONTS.iter().map(|font| Cow::Borrowed(*font)).collect())
        .expect("the bundled fonts load");
    let registry = ThemeRegistry::global_mut(cx);
    registry
        .load_themes_from_str(THEMES)
        .expect("the bundled theme file parses");
    let themes = registry.themes();
    let (light, dark) = (themes[LIGHT].clone(), themes[DARK].clone());
    let theme = Theme::global_mut(cx);
    theme.light_theme = light;
    theme.dark_theme = dark;
    Theme::change(cx.window_appearance(), None, cx);
}

/// Follows the system's light or dark appearance while the window is open.
pub fn follow_system(window: &mut Window) -> gpui_kit::Subscription {
    window.observe_window_appearance(|window, cx| Theme::sync_system_appearance(Some(window), cx))
}

/// The app's own colour roles, beside the kit's tokens.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// The game's cream: an item's name strip, the filled pills.
    pub parchment: Hsla,
    /// Text on parchment.
    pub ink: Hsla,
    /// Ornaments (the diamonds) and what is selected.
    pub gold: Hsla,
    /// A 1★ to 5★ tile's two colours, dark then light.
    tiles: [(Hsla, Hsla); 5],
}

static PALETTES: LazyLock<(Palette, Palette)> = LazyLock::new(|| {
    let file: serde_json::Value = serde_json::from_str(THEMES).expect("theme.json parses");
    let palette = |name: &str| {
        let theme = file["themes"]
            .as_array()
            .and_then(|themes| themes.iter().find(|t| t["name"] == name))
            .unwrap_or_else(|| panic!("theme.json has the theme {name}"));
        let color = |key: &str| {
            let value = theme["app"][key]
                .as_str()
                .unwrap_or_else(|| panic!("{name} has app.{key}"));
            try_parse_color(value).unwrap_or_else(|_| panic!("app.{key} of {name} is a colour"))
        };
        let tile = |stars: u8| {
            (
                color(&format!("tile.{stars}")),
                color(&format!("tile.{stars}.light")),
            )
        };
        Palette {
            parchment: color("parchment"),
            ink: color("ink"),
            gold: color("gold"),
            tiles: [tile(1), tile(2), tile(3), tile(4), tile(5)],
        }
    };
    (palette(LIGHT), palette(DARK))
});

/// The app's own colour roles in the current theme.
pub fn palette(cx: &App) -> &'static Palette {
    if cx.theme().is_dark() {
        &PALETTES.1
    } else {
        &PALETTES.0
    }
}

/// An item's name or count strip under its picture: ink on parchment, in
/// both themes, as on the game's item cards. Returns (text, fill).
pub fn tile_label(cx: &App) -> (Hsla, Hsla) {
    let palette = palette(cx);
    (palette.ink, palette.parchment)
}

/// Text and number colour of a rarity: 5★ gold, 4★ purple, 3★ muted.
pub fn rarity(rarity: Rarity, cx: &App) -> Hsla {
    let theme = cx.theme();
    match rarity {
        Rarity::Five => theme.yellow,
        Rarity::Four => theme.magenta,
        Rarity::Three => theme.muted_foreground,
    }
}

/// A rarity's colour as a chart series: 5★ gold (`chart_4`), 4★ purple
/// (`chart_3`), 3★ blue (`chart_2`, also the colour of all wishes together).
/// In dark mode they are a step darker than [`rarity`], so the series sit in
/// one lightness band and none outshines the others.
pub fn rarity_chart(rarity: Rarity, cx: &App) -> Hsla {
    let theme = cx.theme();
    match rarity {
        Rarity::Five => theme.chart_4,
        Rarity::Four => theme.chart_3,
        Rarity::Three => theme.chart_2,
    }
}

/// The background of a wished item's icon: its rarity's tile (3★ blue).
pub fn rarity_tile(rarity: Rarity, cx: &App) -> Background {
    stars_tile(
        match rarity {
            Rarity::Five => 5,
            Rarity::Four => 4,
            Rarity::Three => 3,
        },
        cx,
    )
}

/// A soft badge colour pair (text, fill) for a rate-up result.
pub fn soft(color: Hsla) -> (Hsla, Hsla) {
    (color, color.opacity(0.16))
}

/// The colour of a 1★ to 5★ thing as text (its stars, a filter): gold,
/// purple, blue, green, grey, each readable on the theme's surfaces.
pub fn stars_color(stars: u8, cx: &App) -> Hsla {
    let theme = cx.theme();
    match stars {
        5 => theme.yellow,
        4 => theme.magenta,
        3 => theme.blue,
        2 => theme.green,
        _ => theme.muted_foreground,
    }
}

/// The background of a 1★ to 5★ thing (a material, a character card), as on
/// the game's item cards: its rarity colour, darker at the top left, lighter
/// toward the bottom right. The same in both themes.
pub fn stars_tile(stars: u8, cx: &App) -> Background {
    let (dark, light) = tile_colors(stars, cx);
    linear_gradient(
        150.,
        linear_color_stop(dark, 0.),
        linear_color_stop(light, 1.),
    )
}

fn tile_colors(stars: u8, cx: &App) -> (Hsla, Hsla) {
    let index = usize::from(stars.clamp(1, 5) - 1);
    palette(cx).tiles[index]
}
