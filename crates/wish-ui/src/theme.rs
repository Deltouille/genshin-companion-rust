//! The app's look: its own light and dark Material 3 themes (`theme.json`, the
//! colours of the Les Archives de Teyvat blog), and the colour roles the pages
//! use on top of the theme's tokens.
//!
//! Every raw colour lives in `theme.json`. Rarity colours follow the game's
//! convention (5★ gold, 4★ purple); they are always paired with the written
//! rarity ("5★"), never the only cue.

use std::borrow::Cow;

use gpui_kit::component::{ActiveTheme as _, Theme, ThemeRegistry};
use gpui_kit::{App, Background, Hsla, Window, linear_color_stop, linear_gradient};
use wish_core::model::Rarity;

const THEMES: &str = include_str!("theme.json");
const LIGHT: &str = "Genshin Companion Light";
const DARK: &str = "Genshin Companion Dark";

/// Google Sans Flex, Material 3 Expressive's typeface (SIL Open Font License,
/// `fonts/GoogleSansFlex-OFL.txt`), in the weights the pages use: static
/// instances of its variable font (opsz 18, wdth 100, ROND 0), which GPUI
/// can't vary itself.
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

/// The background of an item icon, as on paimon.moe: the rarity's bright
/// colour fading to a darker shade of it (3★ sits on the muted surface).
pub fn rarity_tile(rarity: Rarity, cx: &App) -> Background {
    let theme = cx.theme();
    let from = match rarity {
        Rarity::Five if theme.is_dark() => theme.yellow,
        Rarity::Five => theme.yellow_light,
        Rarity::Four if theme.is_dark() => theme.magenta,
        Rarity::Four => theme.magenta_light,
        Rarity::Three => return theme.muted.into(),
    };
    let to = Hsla {
        l: from.l * 0.45,
        ..from
    };
    linear_gradient(180., linear_color_stop(from, 0.), linear_color_stop(to, 1.))
}

/// A soft badge colour pair (text, fill) for a rate-up result.
pub fn soft(color: Hsla) -> (Hsla, Hsla) {
    (color, color.opacity(0.16))
}

/// The game's colour of a 1★ to 5★ thing: grey, green, blue, purple, gold.
pub fn stars_color(stars: u8, cx: &App) -> Hsla {
    let theme = cx.theme();
    let dark = theme.is_dark();
    match stars {
        5 if dark => theme.yellow,
        5 => theme.yellow_light,
        4 if dark => theme.magenta,
        4 => theme.magenta_light,
        3 if dark => theme.blue,
        3 => theme.blue_light,
        2 if dark => theme.green,
        2 => theme.green_light,
        _ => theme.muted_foreground,
    }
}

/// The background of a 1★ to 5★ thing (a material, a character card): the
/// game's rarity colours, grey, green, blue, purple and gold, each fading to a
/// darker shade of itself.
pub fn stars_tile(stars: u8, cx: &App) -> Background {
    let from = stars_color(stars, cx);
    let to = Hsla {
        l: from.l * 0.45,
        ..from
    };
    linear_gradient(180., linear_color_stop(from, 0.), linear_color_stop(to, 1.))
}
