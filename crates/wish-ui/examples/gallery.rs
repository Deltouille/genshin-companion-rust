//! Renders every shared component offscreen, in the light and the dark
//! theme, into PNG files: a way to review the design system on its own.
//!
//!     cargo run -p wish-ui --example gallery -- <out-dir>

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::{ActiveTheme as _, Icon, Theme, ThemeMode};
use gpui_kit::{
    AppContext as _, Context, HeadlessAppContext, IntoElement, ParentElement as _, Render,
    Styled as _, Window, div, px, rems, size,
};
use wish_core::model::{Pool, Rarity};
use wish_ui::{
    PageFrame, Pager, Segmented, badge, big_number, button, card, card_title, filter_chip,
    icon_button, item_tile, pool_icon, stat_label, stat_tile, theme,
};

struct Gallery;

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme();
        let (gold, purple) = (
            theme::rarity(Rarity::Five, cx),
            theme::rarity(Rarity::Four, cx),
        );
        let section = |title: &'static str| div().v_flex().gap_3().child(card_title(title, cx));

        let tiles = card(cx)
            .p_4()
            .gap_2()
            .w_96()
            .child(card_title("stat_tile + big_number", cx))
            .child(stat_tile(
                "Lifetime wishes",
                "2,400 primogems",
                big_number("15", colors.foreground),
                cx,
            ))
            .child(stat_tile(
                "5★ pity",
                "Guaranteed at 90",
                div()
                    .h_flex()
                    .gap_3()
                    .child(badge("Guaranteed", theme::soft(gold), cx))
                    .child(big_number("74", gold)),
                cx,
            ))
            .child(stat_tile(
                "4★ pity",
                "Guaranteed at 10",
                big_number("3", purple),
                cx,
            ));

        let badges = section("badge").child(
            div()
                .h_flex()
                .gap_2()
                .child(badge("Won", theme::soft(colors.green), cx))
                .child(badge("Lost", theme::soft(colors.red), cx))
                .child(badge("Guaranteed", theme::soft(gold), cx))
                .child(badge("50/50", (colors.muted_foreground, colors.muted), cx)),
        );
        let tiles_row = section("item_tile (rarity gradient, no icon)").child(
            div()
                .h_flex()
                .gap_2()
                .child(item_tile(Rarity::Five, None, rems(1.75), cx))
                .child(item_tile(Rarity::Four, None, rems(1.75), cx))
                .child(item_tile(Rarity::Three, None, rems(1.75), cx)),
        );
        let icons = section("pool_icon").child(
            div().h_flex().gap_3().children(
                Pool::ALL
                    .iter()
                    .map(|pool| Icon::new(pool_icon(*pool)).text_color(colors.primary)),
            ),
        );
        let buttons = section("button, icon_button").child(
            div()
                .h_flex()
                .gap_2()
                .child(button("primary").primary().label("Primary"))
                .child(button("default").label("Default"))
                .child(button("ghost").ghost().label("Ghost"))
                .child(button("danger").danger().label("Delete"))
                .child(icon_button("open", IconName::ChevronRight, "Open").ghost()),
        );
        let choices = section("Segmented, filter_chip").child(
            div()
                .h_flex()
                .gap_4()
                .child(
                    Segmented::new("view")
                        .option("Wishes")
                        .option("Banners")
                        .selected(0),
                )
                .child(filter_chip("five", "5★", gold, true, cx))
                .child(filter_chip("four", "4★", purple, true, cx))
                .child(filter_chip("three", "3★", colors.blue, false, cx)),
        );
        let pager = section("Pager").child(Pager::new("pager").page(4).pages(17));
        let labels = section("stat_label").child(stat_label("5★ pity", cx));

        let body = div().h_flex().items_start().gap_6().child(tiles).child(
            card(cx)
                .p_4()
                .gap_6()
                .flex_1()
                .child(badges)
                .child(tiles_row)
                .child(icons)
                .child(buttons)
                .child(choices)
                .child(pager)
                .child(labels),
        );
        div()
            .size_full()
            .bg(colors.background)
            .text_color(colors.foreground)
            .child(
                PageFrame::new("Components", body)
                    .subtitle("Everything in wish-ui, in the current theme"),
            )
    }
}

fn main() -> Result<()> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [out_dir] = args.as_slice() else {
        bail!("usage: gallery <out-dir>");
    };
    std::fs::create_dir_all(out_dir)?;

    let platform = gpui_kit::platform::current_platform(true);
    let mut cx = HeadlessAppContext::with_platform(
        platform.text_system(),
        Arc::new(wish_ui::UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        wish_ui::init(cx);
    });
    let (window, _) = cx.update(|cx| {
        let options = gpui_kit::WindowOptions {
            window_bounds: Some(gpui_kit::WindowBounds::Windowed(
                gpui_kit::Bounds::centered(None, size(px(1280.), px(820.)), cx),
            )),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| Gallery))
    })?;

    for (name, mode) in [("light", ThemeMode::Light), ("dark", ThemeMode::Dark)] {
        cx.update(|cx| Theme::change(mode, None, cx));
        // Nothing else changes, so ask for a new frame in the new theme.
        cx.update_window(window, |_, window, _| window.refresh())?;
        cx.run_until_parked();
        let image = cx
            .capture_screenshot(window)
            .context("rendering the gallery")?;
        let path = out_dir.join(format!("gallery-{name}.png"));
        image
            .save(&path)
            .with_context(|| format!("writing {}", path.display()))?;
        println!("{}", path.display());
    }
    Ok(())
}
