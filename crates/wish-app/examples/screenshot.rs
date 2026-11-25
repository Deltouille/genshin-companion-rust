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
    ArchivePage, ChroniclePage, ImportFromGame, Library, OverviewTab, Page, Services, Shell,
    WishPage,
};
use wish_core::AppPaths;
use wish_core::collection::{OwnedCharacter, OwnedWeapon};
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
    // SCREENSHOT_LANGUAGE=fr shoots the pages in French; English otherwise,
    // whatever the data folder's settings say, so captures compare.
    let language = std::env::var("SCREENSHOT_LANGUAGE")
        .ok()
        .and_then(|code| wish_core::i18n::Language::from_locale(&code))
        .unwrap_or(wish_core::i18n::Language::English);
    wish_core::i18n::set_language(language);
    cx.update(wish_app::init);

    let paths = AppPaths::at(data_dir);
    let manual_counting = wish_core::settings::Settings::load(&paths)?.manual_counting;
    // The real network for the images; a sample HoYoLAB for the sync's
    // preview (never the real one: no cookie here).
    // Never the player's keyring: one in memory, which keeps a sample login
    // (Settings → HoYoLAB shows it kept).
    let sample_login =
        wish_core::hoyolab::Cookie::parse("ltuid_v2=1; ltoken_v2=sample; ltmid_v2=m")
            .expect("a sample cookie");
    let services = Services {
        keyring: Arc::new(wish_core::keyring::MemoryKeyring::with(&sample_login)),
        hoyolab: Arc::new(SampleHoyolab),
        // At once: the sample HoYoLAB minds no pace.
        pacing: wish_core::import::api::Pacing {
            delay: Duration::ZERO,
            ..wish_core::import::api::Pacing::default()
        },
        ..Services::network()
    };
    let library = cx.update(|cx| cx.new(|cx| Library::open(paths, services, cx)));
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
        ("overview", Page::Overview(OverviewTab::Wishes)),
        (
            "overview-characters",
            Page::Overview(OverviewTab::Characters),
        ),
        ("overview-weapons", Page::Overview(OverviewTab::Weapons)),
        (
            "history-character",
            Page::Wishes(WishPage::History(Pool::CharacterEvent)),
        ),
        (
            "history-standard",
            Page::Wishes(WishPage::History(Pool::Standard)),
        ),
        ("resources", Page::Archive(ArchivePage::Resources)),
        ("chronicle-abyss", Page::Chronicle(ChroniclePage::Abyss)),
        ("chronicle-theater", Page::Chronicle(ChroniclePage::Theater)),
        ("chronicle-diary", Page::Chronicle(ChroniclePage::Diary)),
        ("imports", Page::Imports),
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
            if cx.update(|cx| library.read(cx).images(cx).pending()) == 0 {
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

    // A few characters the account owns (added once to a data folder that
    // has none), Ganyu among them, a favourite: the archive and the
    // overview's Characters tab show them.
    let account = cx.update(|cx| shell.read(cx).session().read(cx).account());
    if cx.update(|cx| library.read(cx).owned(account).characters.is_empty()) {
        let owned = [
            ("ganyu", (80, 6), 2, [6, 9, 9], true),
            ("furina", (90, 6), 0, [1, 9, 9], true),
            ("bennett", (80, 5), 6, [1, 8, 9], false),
            ("xiangling", (70, 4), 6, [1, 6, 8], false),
            ("amber", (20, 0), 1, [1, 1, 1], false),
        ];
        for (key, (level, phase), constellation, talents, favourite) in owned {
            let character = OwnedCharacter {
                level: Level::new(level, phase).expect("a level of its phase"),
                constellation,
                talents,
                favourite,
                ..OwnedCharacter::new(key)
            };
            cx.update(|cx| {
                library.update(cx, |l, cx| {
                    l.save_owned_character(account, character, cx).detach()
                })
            });
        }
        for _ in 0..40 {
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    // A few weapons too (added once): Ganyu's Amos' Bow, two Favonius Swords
    // (one held by Bennett), Xiangling's The Catch at R5, a spare catalyst.
    if cx.update(|cx| library.read(cx).owned(account).weapons.is_empty()) {
        let weapons = [
            ("amosbow", (90, 6), 1, Some("ganyu"), true),
            ("favoniussword", (80, 6), 3, Some("bennett"), false),
            ("favoniussword", (1, 0), 2, None, false),
            ("thecatch", (70, 5), 5, Some("xiangling"), false),
            ("sacrificialfragments", (20, 0), 1, None, false),
        ];
        for (key, (level, phase), refinement, holder, favourite) in weapons {
            let weapon = OwnedWeapon {
                level: Level::new(level, phase).expect("a level of its phase"),
                refinement,
                holder: holder.map(str::to_owned),
                favourite,
                ..OwnedWeapon::new(key)
            };
            cx.update(|cx| {
                library.update(cx, |l, cx| l.add_owned_weapon(account, weapon, cx).detach())
            });
            for _ in 0..10 {
                cx.run_until_parked();
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    // A chronicle (kept once in a data folder that has none): six Abyss
    // periods, five Theater periods, five Diary months, as a sync would
    // keep them, into the account shown, under its UID.
    if cx.update(|cx| library.read(cx).chronicle(account).is_empty()) {
        use wish_core::chronicle::Record;
        use wish_core::model::Server;
        let uid = cx.update(|cx| {
            let library = library.read(cx);
            account
                .and_then(|id| library.account(id))
                .and_then(|summary| summary.account.uid.clone())
        });
        let mut records = Vec::new();
        for k in 0..6 {
            records.extend(Record::abyss(
                sample_chronicle::abyss(k),
                sample_chronicle::fetched(k),
            )?);
        }
        for k in 0..5 {
            let fetched = sample_chronicle::fetched(k);
            let one = serde_json::json!({ "data": [sample_chronicle::theater(k)] });
            records.extend(Record::theater(one, fetched)?);
        }
        for k in 0..5 {
            let (info, pages) = sample_chronicle::diary(k);
            // Kept from the month after it, or November's under way.
            let current = sample_chronicle::diary(k.saturating_sub(1)).0["data_month"]
                .as_u64()
                .and_then(|m| u32::try_from(m).ok())
                .unwrap_or(11);
            let fetched = sample_chronicle::fetched(k);
            records.push(Record::diary(
                info,
                pages,
                current,
                Server::Europe,
                fetched,
            )?);
        }
        let fetched = wish_app::app::library::HoyolabAccount {
            account: wish_core::hoyolab::GameAccount {
                uid: uid.unwrap_or_else(|| "712345678".to_owned()),
                region: "os_euro".to_owned(),
                nickname: "Traveler".to_owned(),
                level: 60,
                game_biz: "hk4e_global".to_owned(),
            },
            details: Vec::new(),
            records,
            choices: Vec::new(),
            suggested: 0,
        };
        let task =
            cx.update(|cx| library.update(cx, |l, cx| l.apply_hoyolab_sync(fetched, account, cx)));
        for _ in 0..100 {
            cx.run_until_parked();
            if cx.update(|cx| !library.read(cx).chronicle(account).is_empty()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        drop(task);
    }

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
        let account = cx.update(|cx| shell.read(cx).session().read(cx).account());
        if cx.update(|cx| library.read(cx).plan(account).is_empty()) {
            // A leak: the first series of each role (after the placeholder),
            // a local specialty the game data doesn't have yet, and a weekly
            // boss material not known yet ("?").
            let data = PlannerData::embedded();
            let first = |role: &Role| (*role, CustomItem::Material(data.families(*role)[1][0]));
            let mut materials: CustomMaterials = Role::CHARACTER.iter().map(first).collect();
            materials.insert(
                Role::LocalSpecialty,
                CustomItem::Named("Moonfall Lily".to_owned()),
            );
            materials.insert(
                Role::WeeklyMaterial,
                CustomItem::Material(data.families(Role::WeeklyMaterial)[0][0]),
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
                    materials: Role::WEAPON.iter().map(first).collect(),
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
                cx.update(|cx| {
                    library.update(cx, |l, cx| l.add_plan_entry(account, target, cx).detach())
                });
                // The write runs in the background; wait for its reload.
                for _ in 0..100 {
                    cx.run_until_parked();
                    if cx.update(|cx| library.read(cx).plan(account).len()) > n {
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
                let task = cx
                    .update(|cx| library.update(cx, |l, cx| l.set_owned(account, item, count, cx)));
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
            let plan = library.read(cx).plan(account);
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
            let plan = library.read(cx).plan(account);
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
        // Choosing a role's material: its placeholder "?" first.
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| {
                shell.open_custom_picker(Role::TalentBook, window, cx)
            })
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("planner-custom-pick-{mode_name}"))?;
        cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        // The inventory dialog, on a talent book (its whole series, the
        // guides crafted from the teachings), on Hero's Wit (the three books,
        // each worth its EXP), then on Mora (one material, its line under the
        // name long enough to wrap in French, and the ley line button).
        for (id, name) in [
            (104314, "inventory"),
            (104003, "exp"),
            (wish_core::planner::MORA, "mora"),
        ] {
            cx.update_window(window, |_, window, cx| {
                shell.update(cx, |shell, cx| shell.open_inventory(id, window, cx))
            })?;
            for _ in 0..3 {
                std::thread::sleep(Duration::from_millis(400));
                cx.advance_clock(Duration::from_millis(400));
                cx.update_window(window, |_, window, _| window.refresh())?;
                cx.run_until_parked();
            }
            shoot(&mut cx, format!("planner-{name}-{mode_name}"))?;
            cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
            cx.advance_clock(Duration::from_secs(1));
            cx.run_until_parked();
        }
        // "Add a character": the cards, what the account owns first.
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| shell.open_planner_add(false, window, cx))
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("planner-add-{mode_name}"))?;
        cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        // The resin estimate's options.
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| shell.open_resin_options(window, cx))
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("planner-resin-{mode_name}"))?;
        cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        // The second entry's move menu, which has every move (a tall window
        // shows the cards).
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| shell.show_plan_moves(1, true, window, cx))
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("planner-moves-{mode_name}"))?;
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| shell.show_plan_moves(1, false, window, cx))
        })?;
        cx.run_until_parked();
        // The Battle Pass drawer, on a pass at level 23 with the Gnostic Hymn,
        // counted in "Still needed", its Philosophies boxes picked and the
        // others left to the plan (entered once in a data folder that has
        // none).
        if cx.update(|cx| library.read(cx).battle_pass(account).is_none()) {
            use wish_core::planner::{BattlePass, BattlePassData, BoxKind, PlannerData};
            let data = BattlePassData::embedded();
            let now = chrono::Local::now().fixed_offset();
            let freedom = PlannerData::embedded().material_named("Philosophies of Freedom");
            let pass = BattlePass {
                premium: true,
                counted: true,
                ..BattlePass::new(data, now)
            }
            .at_level(23, data, now);
            let pass = BattlePass {
                picks: pass.picks.with(BoxKind::Philosophies, freedom),
                ..pass
            };
            cx.update(|cx| {
                library.update(cx, |l, cx| l.set_battle_pass(account, pass, cx).detach())
            });
            for _ in 0..100 {
                cx.run_until_parked();
                if cx.update(|cx| library.read(cx).battle_pass(account).is_some()) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| shell.show_battle_pass(true, window, cx))
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("planner-pass-{mode_name}"))?;
        // Its list of reward plans, open.
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| {
                shell.show_battle_pass_plans(true, window, cx)
            })
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("planner-pass-plans-{mode_name}"))?;
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| {
                shell.show_battle_pass_plans(false, window, cx);
                shell.show_battle_pass(false, window, cx);
            })
        })?;
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

        // Settings → Accounts: a new account, then the first one.
        let first = cx.update(|cx| library.read(cx).accounts().first().map(|a| a.account.id));
        for (name, account) in [("new", None), ("edit", first)] {
            cx.update_window(window, |_, window, cx| {
                shell.update(cx, |shell, cx| shell.open_account(account, window, cx))
            })?;
            for _ in 0..3 {
                std::thread::sleep(Duration::from_millis(300));
                cx.advance_clock(Duration::from_millis(300));
                cx.update_window(window, |_, window, _| window.refresh())?;
                cx.run_until_parked();
            }
            shoot(&mut cx, format!("settings-account-{name}-{mode_name}"))?;
            cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
            cx.advance_clock(Duration::from_secs(1));
            cx.run_until_parked();
        }

        // The character archive: the list, then one character's three tabs.
        cx.update(|cx| {
            shell.update(cx, |shell, cx| {
                shell.navigate(Page::Archive(ArchivePage::Characters), cx)
            })
        });
        settle(&mut cx)?;
        shoot(&mut cx, format!("characters-{mode_name}"))?;
        // The filters, in their sheet.
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| shell.open_character_filters(window, cx))
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("characters-filters-{mode_name}"))?;
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();

        // One character's page (its sections stack: a tall window shows more).
        cx.update(|cx| shell.update(cx, |shell, cx| shell.open_character("ganyu", cx)));
        settle(&mut cx)?;
        shoot(&mut cx, format!("character-{mode_name}"))?;

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
        // SCREENSHOT_WEAPON=coolsteel shoots another weapon's page (one the
        // account has many copies of, after a GOOD import).
        let weapon = std::env::var("SCREENSHOT_WEAPON").unwrap_or_else(|_| "amosbow".to_owned());
        cx.update(|cx| shell.update(cx, |shell, cx| shell.open_weapon(&weapon, cx)));
        settle(&mut cx)?;
        shoot(&mut cx, format!("weapon-{mode_name}"))?;

        // The "Import from the game" dialog, opened as the menu would.
        cx.update(|cx| {
            shell.update(cx, |shell, cx| {
                shell.navigate(Page::Overview(OverviewTab::Wishes), cx)
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

        // The HoYoLAB sync's first step, over the Import page.
        cx.update(|cx| shell.update(cx, |shell, cx| shell.navigate(Page::Imports, cx)));
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| shell.open_hoyolab_sync(window, cx))
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("hoyolab-{mode_name}"))?;
        cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        // Its preview, from the sample HoYoLAB: a change of each kind and the
        // notes (a level whose phase isn't sent, a character the data lacks).
        cx.update_window(window, |_, window, cx| {
            let cookie = wish_core::hoyolab::Cookie::parse("ltuid_v2=1; ltoken_v2=sample")
                .expect("a sample cookie");
            shell.update(cx, |shell, cx| {
                shell.open_hoyolab_preview(cookie, window, cx)
            })
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("hoyolab-preview-{mode_name}"))?;
        cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        // The GOOD import's first step, then its preview of a sample file:
        // fodder grouped, materials, and the notes.
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| {
                shell.open_good_import(window, cx);
            })
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("good-{mode_name}"))?;
        cx.update_window(window, |_, window, cx| window.close_dialog(cx))?;
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| {
                shell.open_good_preview(SAMPLE_GOOD.to_owned(), window, cx)
            })
        })?;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            cx.advance_clock(Duration::from_millis(400));
            cx.update_window(window, |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        shoot(&mut cx, format!("good-preview-{mode_name}"))?;
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

/// A GOOD file as the screenshot tool shows it: a few characters (Skirk
/// above 90, a Manekina the data lacks), their weapons, fodder by the dozen,
/// a bow the data lacks, some materials and a steak; never a real export.
const SAMPLE_GOOD: &str = r#"{
    "format": "GOOD", "version": 3, "source": "Irminsul",
    "characters": [
        {"key": "Ganyu", "level": 80, "constellation": 1, "ascension": 6,
         "talent": {"auto": 9, "skill": 8, "burst": 10}},
        {"key": "Bennett", "level": 90, "constellation": 6, "ascension": 6,
         "talent": {"auto": 1, "skill": 9, "burst": 10}},
        {"key": "Xingqiu", "level": 80, "constellation": 6, "ascension": 5,
         "talent": {"auto": 1, "skill": 8, "burst": 9}},
        {"key": "Skirk", "level": 100, "constellation": 0, "ascension": 6,
         "talent": {"auto": 10, "skill": 10, "burst": 10}},
        {"key": "Manekina", "level": 20, "constellation": 0, "ascension": 0,
         "talent": {"auto": 1, "skill": 1, "burst": 1}}
    ],
    "weapons": [
        {"key": "AmosBow", "level": 90, "ascension": 6, "refinement": 1, "location": "Ganyu"},
        {"key": "SkywardBlade", "level": 90, "ascension": 6, "refinement": 1, "location": "Bennett"},
        {"key": "SacrificialSword", "level": 80, "ascension": 6, "refinement": 5, "location": "Xingqiu"},
        {"key": "Azurelight", "level": 90, "ascension": 6, "refinement": 1, "location": "Skirk"},
        {"key": "CoolSteel", "level": 1, "ascension": 0, "refinement": 1, "location": ""},
        {"key": "CoolSteel", "level": 1, "ascension": 0, "refinement": 1, "location": ""},
        {"key": "CoolSteel", "level": 1, "ascension": 0, "refinement": 1, "location": ""},
        {"key": "CoolSteel", "level": 1, "ascension": 0, "refinement": 1, "location": ""},
        {"key": "CoolSteel", "level": 1, "ascension": 0, "refinement": 1, "location": ""},
        {"key": "HarbingerOfDawn", "level": 1, "ascension": 0, "refinement": 1, "location": ""},
        {"key": "HarbingerOfDawn", "level": 1, "ascension": 0, "refinement": 1, "location": ""},
        {"key": "HarbingerOfDawn", "level": 1, "ascension": 0, "refinement": 1, "location": ""},
        {"key": "ANewBow", "level": 1, "ascension": 0, "refinement": 1, "location": ""}
    ],
    "materials": {
        "HerosWit": 121, "AdventurersExperience": 340, "MysticEnhancementOre": 2000,
        "TeachingsOfFreedom": 12, "GuideToFreedom": 30, "PhilosophiesOfFreedom": 2,
        "CrownOfInsight": 3, "DeliciousSteak": 5
    }
}"#;

/// HoYoLAB as the screenshot tool shows it: one Genshin account with Ganyu
/// at level 80 (HoYoLAB doesn't say whether she ascended), Bennett, and a
/// character the game data doesn't have.
struct SampleHoyolab;

impl wish_core::hoyolab::Transport for SampleHoyolab {
    fn send(&self, request: &wish_core::hoyolab::Request) -> wish_core::hoyolab::Result<String> {
        let amos = serde_json::json!({ "id": 15502, "name": "Amos' Bow", "rarity": 5,
            "level": 80, "affix_level": 1, "promote_level": 6, "type": 12 });
        let favonius = serde_json::json!({ "id": 11401, "name": "Favonius Sword", "rarity": 4,
            "level": 90, "affix_level": 3, "promote_level": 6, "type": 1 });
        let base =
            |id: u32, name: &str, element: &str, rarity: u8, level: u8, constellation: u8| {
                serde_json::json!({ "id": id, "name": name, "element": element, "rarity": rarity,
                "level": level, "actived_constellation_num": constellation })
            };
        let skills = |names: [&str; 3], levels: [u8; 3]| -> serde_json::Value {
            (names.iter().zip(levels).enumerate())
                .map(|(ix, (name, level))| {
                    serde_json::json!({ "skill_id": ix + 1, "skill_type": 1, "name": name,
                        "level": level })
                })
                .collect()
        };
        let sword = serde_json::json!({ "id": 11301, "name": "Harbinger of Dawn", "rarity": 3,
            "level": 1, "affix_level": 1, "promote_level": 0, "type": 1 });
        let data = if request.url.ends_with("getUserGameRolesByCookie") {
            serde_json::json!({ "list": [{ "game_biz": "hk4e_global", "region": "os_euro",
                "game_uid": "712345678", "nickname": "Traveler", "level": 60 }] })
        } else if request.url.ends_with("character/list") {
            let list = [
                (10000037, "Ganyu", "Cryo", 5, 80, 1, amos),
                (10000032, "Bennett", "Pyro", 4, 90, 6, favonius),
                (10000999, "Someone New", "Anemo", 5, 90, 0, sword),
            ]
            .map(
                |(id, name, element, rarity, level, constellation, weapon)| {
                    let mut character = base(id, name, element, rarity, level, constellation);
                    character["weapon"] = weapon;
                    character
                },
            );
            serde_json::json!({ "list": list })
        } else if request.url.ends_with("character/detail") {
            serde_json::json!({ "list": [
                { "base": base(10000037, "Ganyu", "Cryo", 5, 80, 1), "weapon": amos,
                  "constellations": [], "skills": skills(
                      ["Liutian Archery", "Trail of the Qilin", "Celestial Shower"], [6, 9, 9]) },
                { "base": base(10000032, "Bennett", "Pyro", 4, 90, 6), "weapon": favonius,
                  "constellations": [], "skills": skills(
                      ["Strike of Fortune", "Passion Overload", "Fantastic Voyage"], [6, 10, 10]) },
                { "base": base(10000999, "Someone New", "Anemo", 5, 90, 0),
                  "weapon": sword, "constellations": [], "skills": [] },
            ] })
        } else if request.url.contains("spiralAbyss") {
            sample_chronicle::abyss(u32::from(request.url.contains("schedule_type=2")))
        } else if request.url.contains("role_combat") {
            serde_json::json!({ "is_unlock": true, "data": [
                sample_chronicle::theater(0), sample_chronicle::theater(1)] })
        } else if request.url.contains("ysledgeros") {
            // November 2026 is the month under way.
            let param =
                |name: &str| url_param(&request.url, name).and_then(|v| v.parse::<u32>().ok());
            let k = (11 + 12 - param("month").unwrap_or(11)) % 12;
            let (info, pages) = sample_chronicle::diary(k);
            if request.url.contains("month_info") {
                info
            } else {
                let page = param("current_page").unwrap_or(1) as usize;
                (pages.get(page - 1).cloned()).unwrap_or_else(
                    || serde_json::json!({ "data_month": info["data_month"], "list": [] }),
                )
            }
        } else {
            return Err(wish_core::hoyolab::Error::Network(format!(
                "the sample HoYoLAB has no answer for {}",
                request.url
            )));
        };
        Ok(serde_json::json!({ "retcode": 0, "message": "OK", "data": data }).to_string())
    }
}

/// The value of `name` in `url`'s query.
fn url_param(url: &str, name: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query
        .split('&')
        .find_map(|pair| pair.split_once('=').filter(|(n, _)| *n == name))
        .map(|(_, value)| value.to_owned())
}

/// A sample chronicle, as HoYoLAB would show it: Spiral Abyss and
/// Imaginarium Theater periods, a month apart, and Traveler's Diary months.
/// `k` is how many periods before the one under way (0); the one under way
/// is late November 2026.
mod sample_chronicle {
    use chrono::{DateTime, Datelike as _, Duration, NaiveDate, TimeZone as _, Utc};
    use serde_json::{Value, json};
    use wish_core::planner::PlannerData;

    /// The teams the sample fields, by the planner's keys.
    const TEAMS: [[&str; 4]; 6] = [
        ["raidenshogun", "xiangling", "xingqiu", "bennett"],
        ["neuvillette", "furina", "kaedeharakazuha", "zhongli"],
        ["nahida", "yelan", "xingqiu", "kukishinobu"],
        [
            "kamisatoayaka",
            "shenhe",
            "kaedeharakazuha",
            "sangonomiyakokomi",
        ],
        ["hutao", "yelan", "xingqiu", "zhongli"],
        ["ganyu", "xiangling", "bennett", "sangonomiyakokomi"],
    ];

    /// The month `k` months before November 2026, and its first day.
    fn month(k: u32) -> NaiveDate {
        let index = 2026 * 12 + 10 - i32::try_from(k).unwrap_or(0);
        NaiveDate::from_ymd_opt(index.div_euclid(12), index.rem_euclid(12) as u32 + 1, 1)
            .expect("a month")
    }

    fn seconds(day: NaiveDate, hour: u32) -> i64 {
        Utc.from_utc_datetime(&day.and_hms_opt(hour, 0, 0).expect("an hour"))
            .timestamp()
    }

    fn avatar(key: &str, level: u8) -> Value {
        let planner = PlannerData::embedded();
        let character = planner.character(key);
        json!({
            "id": character.map_or(0, |c| c.id),
            "element": character.map_or("", |c| c.element.as_str()),
            "level": level,
            "rarity": character.map_or(4, |c| c.rarity),
        })
    }

    /// When period `k` was fetched: under way for the latest, three days
    /// after its end for the others.
    pub fn fetched(k: u32) -> DateTime<Utc> {
        if k == 0 {
            Utc.with_ymd_and_hms(2026, 11, 25, 10, 0, 0)
                .single()
                .expect("a time")
        } else {
            let end = month(k - 1).with_day(16).expect("the 16th");
            Utc.from_utc_datetime(&end.and_hms_opt(12, 0, 0).expect("noon")) + Duration::days(3)
        }
    }

    /// Spiral Abyss period `k`: the 16th to the 16th.
    pub fn abyss(k: u32) -> Value {
        let start = month(k).with_day(16).expect("the 16th");
        let end = month(k.saturating_sub(1)).with_day(16).expect("the 16th");
        let end = if k == 0 {
            start + Duration::days(30)
        } else {
            end
        };
        let stars: [u32; 6] = [33, 36, 35, 31, 36, 34];
        let total = stars[k as usize % stars.len()];
        let floors: Vec<Value> = (9..=12)
            .map(|floor: u32| {
                let chambers: Vec<Value> = (1..=3)
                    .map(|chamber: u32| {
                        // The stars missing fall in floor 12, then 11.
                        let missing = 36 - total;
                        let place = (12 - floor) * 3 + (3 - chamber);
                        let star = if place < missing.min(6) { 2 } else { 3 };
                        let team = |n: usize| {
                            let team = TEAMS[(n + k as usize + floor as usize) % TEAMS.len()];
                            team.iter().map(|key| avatar(key, 90)).collect::<Vec<_>>()
                        };
                        let day = start + Duration::days(i64::from(floor - 8));
                        json!({ "index": chamber, "star": star, "max_star": 3, "battles": [
                            { "index": 1, "timestamp": (seconds(day, 18) + i64::from(chamber) * 600).to_string(),
                              "avatars": team(chamber as usize) },
                            { "index": 2, "timestamp": (seconds(day, 18) + i64::from(chamber) * 600 + 240).to_string(),
                              "avatars": team(chamber as usize + 3) },
                        ]})
                    })
                    .collect();
                let star: u64 = chambers.iter().map(|c| c["star"].as_u64().unwrap_or(0)).sum();
                json!({ "index": floor, "is_unlock": true, "star": star, "max_star": 9,
                        "levels": chambers })
            })
            .collect();
        let rank = |key: &str, value: u64| {
            let a = avatar(key, 90);
            json!([{ "avatar_id": a["id"], "value": value, "rarity": a["rarity"] }])
        };
        json!({
            "schedule_id": 110 - k,
            "start_time": seconds(start, 3).to_string(),
            "end_time": (seconds(end, 3) - 1).to_string(),
            "total_battle_times": 12 + (36 - total) / 2,
            "total_win_times": 12,
            "max_floor": "12-3",
            "total_star": total,
            "is_unlock": true,
            "reveal_rank": rank(TEAMS[k as usize % 6][0], 8),
            "damage_rank": rank("raidenshogun", 187_420 + u64::from(k) * 9_311),
            "defeat_rank": rank("neuvillette", 96),
            "take_damage_rank": rank("zhongli", 61_250),
            "normal_skill_rank": rank("bennett", 74),
            "energy_skill_rank": rank("xiangling", 31),
            "floors": floors,
        })
    }

    /// Imaginarium Theater period `k`: a calendar month.
    pub fn theater(k: u32) -> Value {
        let start = month(k);
        let end = month(k.saturating_sub(1));
        let end = if k == 0 {
            NaiveDate::from_ymd_opt(2026, 12, 1).expect("a day")
        } else {
            end
        };
        let furthest: [u8; 5] = [8, 10, 10, 8, 10];
        let reached = furthest[k as usize % furthest.len()];
        let acts: Vec<Value> = (1..=reached)
            .map(|act| {
                let team = TEAMS[(act as usize + k as usize) % TEAMS.len()];
                let mut cast: Vec<Value> = team.iter().map(|key| {
                    let mut a = avatar(key, 90);
                    a["avatar_id"] = a["id"].clone();
                    a["avatar_type"] = json!(1);
                    a
                }).collect();
                if act.is_multiple_of(4) {
                    cast[3]["avatar_type"] = json!(2);
                }
                if act.is_multiple_of(5) {
                    cast[2]["avatar_type"] = json!(3);
                }
                json!({ "round_id": act, "is_get_medal": !act.is_multiple_of(3),
                        "finish_time": seconds(start + Duration::days(i64::from(act)), 20).to_string(),
                        "avatars": cast })
            })
            .collect();
        let best = |key: &str, value: &str| {
            let a = avatar(key, 90);
            json!({ "avatar_id": a["id"], "value": value, "rarity": a["rarity"] })
        };
        json!({
            "schedule": { "schedule_id": 70 - k, "start_time": seconds(start, 4).to_string(),
                          "end_time": (seconds(end, 4) - 1).to_string(), "schedule_type": 1 },
            "stat": { "difficulty_id": if k.is_multiple_of(2) { 4 } else { 3 }, "max_round_id": reached,
                      "get_medal_round_list": (1..=reached).map(|a| u8::from(!a.is_multiple_of(3))).collect::<Vec<_>>(),
                      "medal_num": (1..=reached).filter(|a| !a.is_multiple_of(3)).count(),
                      "coin_num": 38 + k, "avatar_bonus_num": 9, "rent_cnt": 4 },
            "detail": { "rounds_data": acts, "fight_statisic": {
                "max_damage_avatar": best("neuvillette", "412870"),
                "max_defeat_avatar": best("raidenshogun", "57"),
                "max_take_damage_avatar": best("zhongli", "88203"),
                "total_use_time": 2460 + k * 130 } },
            "has_data": true, "has_detail_data": true,
        })
    }

    /// The Traveler's Diary of the month `k` months before November 2026:
    /// its totals, and its log's pages (up to the 25th for November).
    pub fn diary(k: u32) -> (Value, Vec<Value>) {
        let first = month(k);
        let days = if k == 0 {
            25
        } else {
            (month(k.saturating_sub(1)) - first).num_days() as u32
        };
        let mut log = Vec::new();
        for day in (1..=days).rev() {
            let date = first.with_day(day).expect("a day");
            let at = |hour: u32| format!("{} {hour:02}:0{}:00", date.format("%Y-%m-%d"), day % 10);
            log.push(json!({ "action_id": 1, "action": "Daily Commission Rewards", "time": at(5), "num": 60 }));
            if day.is_multiple_of(6) {
                log.push(json!({ "action_id": 2, "action": "Event Rewards", "time": at(20), "num": 420 }));
            }
            if day.is_multiple_of(9) {
                log.push(json!({ "action_id": 3, "action": "Achievement Rewards", "time": at(21), "num": 10 }));
            }
            if day == 17 {
                log.push(json!({ "action_id": 4, "action": "Spiral Abyss Rewards", "time": at(19), "num": 600 }));
            }
            if day == 2 {
                log.push(json!({ "action_id": 5, "action": "Imaginarium Theater Rewards", "time": at(19), "num": 800 }));
            }
            if day == 1 {
                log.push(
                    json!({ "action_id": 6, "action": "Mail Rewards", "time": at(4), "num": 300 }),
                );
            }
        }
        let sum = |pred: &dyn Fn(&str) -> bool| -> u64 {
            (log.iter())
                .filter(|e| pred(e["action"].as_str().unwrap_or("")))
                .map(|e| e["num"].as_u64().unwrap_or(0))
                .sum()
        };
        let daily = sum(&|a| a == "Daily Commission Rewards");
        let events = sum(&|a| a == "Event Rewards");
        let abyss = sum(&|a| a.contains("Abyss") || a.contains("Theater"));
        let mail = sum(&|a| a == "Mail Rewards");
        let other = sum(&|a| a == "Achievement Rewards");
        let total = daily + events + abyss + mail + other;
        let number = first.month();
        let info = json!({
            "data_month": number,
            "optional_month": [month(2).month(), month(1).month(), month(0).month()],
            "month_data": {
                "current_primogems": total, "current_mora": 2_740_000 + u64::from(k) * 81_500,
                "group_by": [
                    { "action_id": 1, "action": "Daily Activity", "num": daily },
                    { "action_id": 2, "action": "Events", "num": events },
                    { "action_id": 3, "action": "Spiral Abyss", "num": abyss },
                    { "action_id": 4, "action": "Mail", "num": mail },
                    { "action_id": 5, "action": "Other", "num": other },
                ],
            },
        });
        let pages = log
            .chunks(100)
            .map(|list| json!({ "data_month": number, "list": list }))
            .collect();
        (info, pages)
    }
}
