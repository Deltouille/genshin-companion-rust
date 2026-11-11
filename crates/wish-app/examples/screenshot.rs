//! Renders the main window offscreen, page by page, into PNG files: a way to
//! review the UI without a desktop session (nothing appears on screen).
//!
//!     cargo run -p wish-app --example screenshot -- <data-dir> <out-dir>
//!
//! `<data-dir>` is an app data folder, e.g. one filled with
//! `wishctl --data-dir <data-dir> import-paimon <file>`. Each page is rendered
//! in the light and the dark theme.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use gpui_kit::component::{Theme, ThemeMode, WindowExt as _};
use gpui_kit::{AppContext as _, HeadlessAppContext};
use wish_app::app::{Notice, notify};
use wish_app::{
    ArchivePage, CharacterTab, ImageDownloads, ImportFromGame, Library, Page, Shell, WishPage,
};
use wish_core::AppPaths;
use wish_core::model::Pool;
use wish_core::planner::{
    CharacterGoal, CustomItem, CustomMaterials, Level, PlanTarget, PlannerData, Role, WeaponGoal,
};

fn main() -> Result<()> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [data_dir, out_dir] = args.as_slice() else {
        bail!("usage: screenshot <data-dir> <out-dir>");
    };
    std::fs::create_dir_all(out_dir)?;

    let platform = gpui_kit::platform::current_platform(true);
    let mut cx = HeadlessAppContext::with_platform(
        platform.text_system(),
        Arc::new(wish_app::AppAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(wish_app::init);

    let paths = AppPaths::at(data_dir);
    let manual_counting = wish_core::settings::Settings::load(&paths)?.manual_counting;
    let library = cx.update(|cx| cx.new(|cx| Library::open(paths, ImageDownloads::On, cx)));
    let (window, shell) = cx.update(|cx| {
        let library = library.clone();
        let mut options = wish_app::window_options(cx);
        // SCREENSHOT_HEIGHT=1600 renders a taller window, to review a whole page;
        // SCREENSHOT_WIDTH=2000 a wider one (1280 by default).
        let env = |name: &str| std::env::var(name).ok().and_then(|v| v.parse::<f32>().ok());
        let (width, height) = (env("SCREENSHOT_WIDTH"), env("SCREENSHOT_HEIGHT"));
        if width.is_some() || height.is_some() {
            options.window_bounds = Some(gpui_kit::WindowBounds::Windowed(
                // At the origin rather than centred, which would clamp it to the display.
                gpui_kit::Bounds::new(
                    gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(0.)),
                    gpui_kit::size(
                        gpui_kit::px(width.unwrap_or(1280.)),
                        gpui_kit::px(height.unwrap_or(820.)),
                    ),
                ),
            ));
        }
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Shell::new(library, window, cx))
        })
    })?;
    cx.run_until_parked();

    let pages = [
        ("overview", Page::Wishes(WishPage::Overview)),
        (
            "history-character",
            Page::Wishes(WishPage::History(Pool::CharacterEvent)),
        ),
        (
            "history-standard",
            Page::Wishes(WishPage::History(Pool::Standard)),
        ),
        ("settings", Page::Settings),
        ("credits", Page::Credits),
    ];
    // SCREENSHOT_ONLY=overview,banners shoots only the captures whose names
    // start with one of these.
    let only: Option<Vec<String>> = std::env::var("SCREENSHOT_ONLY")
        .ok()
        .map(|only| only.split(',').map(str::to_owned).collect());
    let shoot = |cx: &mut HeadlessAppContext, name: String| -> Result<()> {
        if let Some(only) = &only
            && !only.iter().any(|prefix| name.starts_with(prefix.as_str()))
        {
            return Ok(());
        }
        // The images this page asks for download on first use: wait for
        // them (a minute at most), then let the window load them.
        cx.update_window(window, |_, window, _| window.refresh())?;
        cx.run_until_parked();
        let mut downloaded = false;
        for _ in 0..600 {
            cx.run_until_parked();
            if cx.update(|cx| library.read(cx).images().pending()) == 0 {
                break;
            }
            downloaded = true;
            std::thread::sleep(Duration::from_millis(100));
        }
        if downloaded {
            for _ in 0..2 {
                std::thread::sleep(Duration::from_millis(300));
                cx.update_window(window, |_, window, _| window.refresh())?;
                cx.run_until_parked();
            }
        }
        let image = cx
            .capture_screenshot(window)
            .context("rendering the window")?;
        let path = out_dir.join(format!("{name}.png"));
        image
            .save(&path)
            .with_context(|| format!("writing {}", path.display()))?;
        println!("{}", path.display());
        Ok(())
    };

    for (mode_name, mode) in [("light", ThemeMode::Light), ("dark", ThemeMode::Dark)] {
        cx.update(|cx| Theme::change(mode, None, cx));
        for (name, page) in pages {
            cx.update(|cx| shell.update(cx, |shell, cx| shell.navigate(page, cx)));
            cx.run_until_parked();
            shoot(&mut cx, format!("{name}-{mode_name}"))?;
        }

        cx.update(|cx| {
            shell.update(cx, |shell, cx| {
                shell.navigate(Page::Wishes(WishPage::History(Pool::CharacterEvent)), cx);
                shell.show_banners(true, cx);
            })
        });
        cx.run_until_parked();
        for _ in 0..2 {
            // Images load in the background; draw again once they have.
            std::thread::sleep(Duration::from_millis(300));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("banners-character-{mode_name}"))?;
        cx.update(|cx| shell.update(cx, |shell, cx| shell.show_banners(false, cx)));

        let settle = |cx: &mut HeadlessAppContext| -> Result<()> {
            for _ in 0..2 {
                // Images load in the background; draw again once they have.
                std::thread::sleep(Duration::from_millis(300));
                cx.update_window(window, |_, window, _| window.refresh())?;
                cx.run_until_parked();
            }
            Ok(())
        };
        // The planner, with a character, a custom character and a few
        // materials in the inventory (added once to a data folder whose plan
        // is empty).
        if cx.update(|cx| library.read(cx).plan().is_empty()) {
            // A leak: the first series of each role, a local specialty the
            // game data doesn't have yet.
            let data = PlannerData::embedded();
            let mut materials: CustomMaterials = Role::CHARACTER
                .iter()
                .map(|role| (*role, CustomItem::Material(data.families(*role)[0][0])))
                .collect();
            materials.insert(
                Role::LocalSpecialty,
                CustomItem::Named("Moonfall Lily".to_owned()),
            );
            let example = [
                PlanTarget::Character {
                    key: "ganyu".to_owned(),
                    goal: CharacterGoal {
                        from: Level::new(70, 4).expect("a level of phase 4"),
                        to: Level::MAX,
                        talents: [(6, 9), (8, 10), (8, 10)],
                    },
                },
                PlanTarget::CustomCharacter {
                    name: "Leaked character".to_owned(),
                    stars: 5,
                    materials,
                    goal: CharacterGoal::FULL,
                },
                PlanTarget::CustomWeapon {
                    name: "Leaked sword".to_owned(),
                    stars: 5,
                    materials: Role::WEAPON
                        .iter()
                        .map(|role| (*role, CustomItem::Material(data.families(*role)[0][0])))
                        .collect(),
                    goal: WeaponGoal::full(5),
                },
                PlanTarget::Weapon {
                    key: "amosbow".to_owned(),
                    goal: WeaponGoal {
                        from: Level::new(40, 1).expect("a level of phase 1"),
                        to: Level::MAX,
                    },
                },
            ];
            for (n, target) in example.into_iter().enumerate() {
                cx.update(|cx| library.update(cx, |l, cx| l.add_plan_entry(target, cx).detach()));
                // The write runs in the background; wait for its reload.
                for _ in 0..100 {
                    cx.run_until_parked();
                    if cx.update(|cx| library.read(cx).plan().len()) > n {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
            // Mora, Hero's Wit, Hoarfrost Core (Ganyu has them all), and some
            // of the leak's local specialty.
            let owned = [
                (CustomItem::Material(202), 1_200_000),
                (CustomItem::Material(104003), 120),
                (CustomItem::Material(113010), 40),
                // Teachings of Diligence, crafted up into Ganyu's guides.
                (CustomItem::Material(104313), 60),
                (CustomItem::Named("Moonfall Lily".to_owned()), 168),
            ];
            for (item, count) in owned {
                let task = cx.update(|cx| library.update(cx, |l, cx| l.set_owned(item, count, cx)));
                task.detach();
            }
            for _ in 0..40 {
                cx.run_until_parked();
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        cx.update(|cx| shell.update(cx, |shell, cx| shell.navigate(Page::Planner, cx)));
        settle(&mut cx)?;
        shoot(&mut cx, format!("planner-{mode_name}"))?;
        let custom = cx.update(|cx| {
            let plan = library.read(cx).plan();
            plan.iter()
                .find(|entry| matches!(entry.target, PlanTarget::CustomCharacter { .. }))
                .map(|entry| entry.id)
        });
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| shell.open_custom_entry(custom, window, cx))
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("planner-custom-{mode_name}"))?;
        cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        // The same dialog on the custom weapon.
        let custom_weapon = cx.update(|cx| {
            let plan = library.read(cx).plan();
            plan.iter()
                .find(|entry| matches!(entry.target, PlanTarget::CustomWeapon { .. }))
                .map(|entry| entry.id)
        });
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| {
                shell.open_custom_entry(custom_weapon, window, cx)
            })
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("planner-custom-weapon-{mode_name}"))?;
        cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        // A new custom entry: a character or a weapon.
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| shell.open_custom_entry(None, window, cx))
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("planner-custom-new-{mode_name}"))?;
        cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        // The inventory dialog, on a talent book: its whole series.
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| shell.open_inventory(104314, window, cx))
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("planner-inventory-{mode_name}"))?;
        cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();

        // The open-source licences, over the Credits page.
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| shell.open_licenses(window, cx))
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("credits-licenses-{mode_name}"))?;
        cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();

        // The character archive: the list, then one character's three tabs.
        cx.update(|cx| {
            shell.update(cx, |shell, cx| {
                shell.navigate(Page::Archive(ArchivePage::Characters), cx)
            })
        });
        settle(&mut cx)?;
        shoot(&mut cx, format!("characters-{mode_name}"))?;
        for (name, tab) in [
            ("profile", CharacterTab::Profile),
            ("talents", CharacterTab::Talents),
            ("constellations", CharacterTab::Constellations),
        ] {
            cx.update(|cx| {
                shell.update(cx, |shell, cx| {
                    shell.open_character("ganyu", cx);
                    shell.show_character_tab(tab, cx);
                })
            });
            settle(&mut cx)?;
            shoot(&mut cx, format!("character-{name}-{mode_name}"))?;
        }

        // Messages (snackbars), over the character's page: work done, then
        // work that failed.
        if only.as_ref().is_none_or(|only| {
            only.iter()
                .any(|prefix| "notifications".starts_with(prefix.as_str()))
        }) {
            cx.update(|cx| {
                notify(Notice::done("Ganyu added to the planner."), cx);
                notify(
                    Notice::failed("Couldn’t export: permission denied (os error 13)"),
                    cx,
                );
            });
            for _ in 0..3 {
                std::thread::sleep(Duration::from_millis(300));
                cx.advance_clock(Duration::from_millis(300));
                cx.update_window(window, |_, window, _| window.refresh())?;
                cx.run_until_parked();
            }
            shoot(&mut cx, format!("notifications-{mode_name}"))?;
            // Done goes by itself; Failed stays until closed.
            cx.advance_clock(Duration::from_secs(5));
            cx.run_until_parked();
            let left = cx.update(|cx| shell.read(cx).notices().read(cx).messages());
            println!("still shown after 5 s: {left:?}");
            // A message stays above a dialog.
            cx.update_window(window, |_, window, cx| {
                shell.update(cx, |shell, cx| shell.open_licenses(window, cx))
            })?;
            for _ in 0..3 {
                std::thread::sleep(Duration::from_millis(300));
                cx.advance_clock(Duration::from_millis(300));
                cx.update_window(window, |_, window, _| window.refresh())?;
                cx.run_until_parked();
            }
            shoot(&mut cx, format!("notifications-dialog-{mode_name}"))?;
            cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
            cx.update(|cx| {
                let notices = shell.read(cx).notices().clone();
                notices.update(cx, |notices, cx| notices.clear(cx));
            });
            cx.advance_clock(Duration::from_secs(1));
            cx.run_until_parked();
        }

        // The weapon archive: the list, then one weapon's page.
        cx.update(|cx| {
            shell.update(cx, |shell, cx| {
                shell.navigate(Page::Archive(ArchivePage::Weapons), cx)
            })
        });
        settle(&mut cx)?;
        shoot(&mut cx, format!("weapons-{mode_name}"))?;
        cx.update(|cx| shell.update(cx, |shell, cx| shell.open_weapon("amosbow", cx)));
        settle(&mut cx)?;
        shoot(&mut cx, format!("weapon-{mode_name}"))?;

        // The "Import from the game" dialog, opened as the menu would.
        cx.update(|cx| {
            shell.update(cx, |shell, cx| {
                shell.navigate(Page::Wishes(WishPage::Overview), cx)
            })
        });
        cx.update_window(window, |_, window, cx| {
            window.dispatch_action(Box::new(ImportFromGame), cx)
        })?;
        // Overlay entrances animate on the wall clock: let one finish.
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            // A capture rasterizes the last drawn frame; ask for a new one.
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("game-import-{mode_name}"))?;
        cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        // With counting by hand on: the "Add a 4★ or 5★" dialog.
        if manual_counting {
            cx.update(|cx| {
                shell.update(cx, |shell, cx| {
                    shell.navigate(Page::Wishes(WishPage::History(Pool::CharacterEvent)), cx)
                })
            });
            cx.run_until_parked();
            cx.update_window(window, |_, window, cx| {
                shell.update(cx, |shell, cx| {
                    shell.open_add_item(Pool::CharacterEvent, window, cx)
                })
            })?;
            for _ in 0..3 {
                std::thread::sleep(Duration::from_millis(400));
                cx.advance_clock(Duration::from_millis(400));
                cx.update_window(window, |_, window, _| window.refresh())?;
                cx.run_until_parked();
            }
            shoot(&mut cx, format!("add-item-{mode_name}"))?;
            cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
            cx.advance_clock(Duration::from_secs(1));
            cx.run_until_parked();
        }
    }
    Ok(())
}
