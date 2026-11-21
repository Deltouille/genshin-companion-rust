//! The app's look: neutral cool greys, one indigo accent for what is current
//! or primary, small corners (the theme's `radius` on controls, `radius.lg` on
//! surfaces) and the Inter typeface; the colour is kept for the game's data
//! (rarities, elements, charts). Its light and dark themes are in
//! `theme.json`, with the colour roles the kit's tokens don't have (the `app`
//! sections, read here as a [`Palette`]).
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

/// Inter 4.1 (SIL Open Font License, `fonts/Inter-OFL.txt`), in the weights
/// the pages use: the static fonts of its `Inter.ttc` collection, one file
/// each, since GPUI can't vary a variable font's weight.
const FONTS: [&[u8]; 5] = [
    include_bytes!("../fonts/Inter-Regular.ttf"),
    include_bytes!("../fonts/Inter-Medium.ttf"),
    include_bytes!("../fonts/Inter-SemiBold.ttf"),
    include_bytes!("../fonts/Inter-Bold.ttf"),
    include_bytes!("../fonts/Inter-Black.ttf"),
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
    /// A surface raised above a card: an item's name strip, a picker's list.
    pub raised: Hsla,
    /// Text on [`Palette::raised`].
    pub on_raised: Hsla,
    /// What is current or selected: the open group's icon, a chosen filter.
    pub highlight: Hsla,
    /// A 1★ to 5★ tile's two colours, dark then light.
    tiles: [(Hsla, Hsla); 5],
    /// Seven shades of the 5★, 4★ and 3★ chart colours, faintest first
    /// ([`rarity_ramp`]).
    ramps: [[Hsla; RAMP_STEPS]; 3],
}

/// How many shades `theme.json` gives each ramp.
const RAMP_STEPS: usize = 7;

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
        let ramp = |hue: &str| std::array::from_fn(|step| color(&format!("ramp.{hue}.{step}")));
        Palette {
            raised: color("raised"),
            on_raised: color("on_raised"),
            highlight: color("highlight"),
            tiles: [tile(1), tile(2), tile(3), tile(4), tile(5)],
            ramps: [ramp("gold"), ramp("purple"), ramp("blue")],
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

/// An item's name or count strip under its picture: the raised surface and
/// its text. Returns (text, fill).
pub fn tile_label(cx: &App) -> (Hsla, Hsla) {
    let palette = palette(cx);
    (palette.on_raised, palette.raised)
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

/// `steps` shades of a rarity's chart colour (5★ gold, 4★ purple, 3★ blue)
/// for an ordered set (constellations, levels), faintest first: the most
/// stands out the most from the card, darker in the light theme and lighter in
/// the dark one. Up to seven, spread over `theme.json`'s `ramp.*` shades:
/// one hue, evenly spaced in OKLCH lightness, the faintest at 2:1 against the
/// card, which the dataviz validator checks (`--ordinal`) for 7, 5 and 4
/// steps. Colour is never their only cue: a legend names each.
pub fn rarity_ramp(rarity: Rarity, steps: usize, cx: &App) -> Vec<Hsla> {
    let ramp = &palette(cx).ramps[match rarity {
        Rarity::Five => 0,
        Rarity::Four => 1,
        Rarity::Three => 2,
    }];
    let steps = steps.min(RAMP_STEPS);
    (0..steps)
        .map(|step| ramp[ramp_index(step, steps)])
        .collect()
}

/// Which of the seven shades step `step` of `steps` takes: spread evenly,
/// the first and the last always in (4 steps: 0, 2, 4, 6).
fn ramp_index(step: usize, steps: usize) -> usize {
    let last = RAMP_STEPS - 1;
    match steps {
        0 | 1 => last,
        _ => (step * last + (steps - 1) / 2) / (steps - 1),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_palettes_read_with_their_ramps() {
        let (light, dark) = &*PALETTES;
        for palette in [light, dark] {
            for ramp in &palette.ramps {
                // Faintest first: the shades all differ.
                assert!(ramp.windows(2).all(|pair| pair[0] != pair[1]));
            }
        }
    }

    #[test]
    fn a_ramp_spreads_its_steps_over_the_seven_shades() {
        let picks = |steps| (0..steps).map(|s| ramp_index(s, steps)).collect::<Vec<_>>();
        assert_eq!(picks(7), [0, 1, 2, 3, 4, 5, 6]);
        // The subsets the dataviz validator checked.
        assert_eq!(picks(5), [0, 2, 3, 5, 6]);
        assert_eq!(picks(4), [0, 2, 4, 6]);
        assert_eq!(picks(1), [6]);
    }
}
