//! The window's model and navigation, run headless (nothing is drawn):
//! opening a data folder, importing a save the way the import dialog does,
//! and following the selected account.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::{AppContext as _, Entity, HeadlessAppContext, NoopTextSystem, Task};
use wish_app::app::library::{LoadState, MirrorStatus};
use wish_app::{ArchivePage, Feature, ImageDownloads, Library, OverviewTab, Page, Shell, WishPage};
use wish_core::AppPaths;
use wish_core::model::{AccountId, Pool};

const SAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../wish-core/tests/fixtures/paimon-sample.json"
);

struct App {
    cx: HeadlessAppContext,
    library: Entity<Library>,
    shell: Entity<Shell>,
}

impl App {
    fn open(data_dir: &Path) -> Self {
        // Not the platform's text system: building the macOS platform
        // panics off the main thread, where tests run. Nothing is drawn
        // here, so text measured roughly is enough.
        let mut cx = HeadlessAppContext::new(Arc::new(NoopTextSystem::new()));
        cx.update(wish_app::init);
        let paths = AppPaths::at(data_dir);
        let (library, shell) = cx.update(|cx| {
            let library = cx.new(|cx| Library::open(paths, ImageDownloads::Off, cx));
            let (_, shell) =
                gpui_kit::open_window(wish_app::window_options(cx), cx, |window, cx| {
                    cx.new(|cx| Shell::new(library.clone(), window, cx))
                })
                .expect("window opens headless");
            (library, shell)
        });
        cx.run_until_parked();
        Self { cx, library, shell }
    }

    /// Runs a task to completion and returns its output.
    fn finish<T: 'static>(&mut self, task: Task<T>) -> T {
        let slot = Rc::new(RefCell::new(None));
        let out = slot.clone();
        self.cx
            .foreground_executor()
            .spawn(async move { *out.borrow_mut() = Some(task.await) })
            .detach();
        self.cx.run_until_parked();
        slot.borrow_mut().take().expect("the task finished")
    }

    /// Reads, previews and applies a file, as the import dialog does.
    fn import(&mut self, file: &str) -> wish_core::import::ImportReport {
        let library = self.library.clone();
        let prepare = self.cx.update(|cx| {
            library.update(cx, |library, cx| {
                library.prepare_import(file.into(), None, cx)
            })
        });
        let prepared = self.finish(prepare).expect("the file reads");
        assert!(prepared.paimon_check.as_ref().is_some_and(|c| c.matches()));
        let apply = self.cx.update(|cx| {
            library.update(cx, |library, cx| library.apply_import(prepared.batch, cx))
        });
        self.finish(apply).expect("the import applies")
    }

    /// Clicks a feature group's header; returns the page shown and whether
    /// the group is folded.
    fn click_group(&mut self, feature: Feature) -> (Page, bool) {
        let shell = self.shell.clone();
        self.cx
            .update(|cx| shell.update(cx, |shell, cx| shell.open_or_fold(feature, cx)));
        self.cx.run_until_parked();
        self.cx
            .update(|cx| (shell.read(cx).page(), shell.read(cx).is_folded(feature)))
    }

    fn navigate(&mut self, page: Page) -> Page {
        let shell = self.shell.clone();
        self.cx
            .update(|cx| shell.update(cx, |shell, cx| shell.navigate(page, cx)));
        self.cx.run_until_parked();
        self.cx.update(|cx| shell.read(cx).page())
    }

    /// The account the pages show: the sidebar's.
    fn account(&mut self) -> Option<AccountId> {
        let shell = self.shell.clone();
        self.cx
            .update(|cx| shell.read(cx).session().read(cx).account())
    }

    /// The plan of the account shown.
    fn plan(&mut self) -> Vec<wish_core::planner::PlanEntry> {
        let (library, account) = (self.library.clone(), self.account());
        self.cx.update(|cx| library.read(cx).plan(account).to_vec())
    }
}

#[test]
fn a_new_data_folder_opens_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::open(dir.path());

    let (ready, accounts) = app.cx.update(|cx| {
        let library = app.library.read(cx);
        (
            matches!(library.state(), LoadState::Ready),
            library.accounts().len(),
        )
    });
    assert!(ready);
    assert_eq!(accounts, 0);
    assert_eq!(
        app.navigate(Page::Wishes(WishPage::History(Pool::CharacterEvent))),
        Page::Overview(OverviewTab::Wishes)
    );
    assert_eq!(app.navigate(Page::Settings), Page::Settings);
    assert_eq!(app.navigate(Page::Imports), Page::Imports);
}

#[test]
fn importing_a_save_shows_its_accounts() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::open(dir.path());

    let report = app.import(SAMPLE);
    assert_eq!(report.added(), 17);

    let (accounts, first) = app.cx.update(|cx| {
        let library = app.library.read(cx);
        (library.accounts().len(), library.accounts()[0].account.id)
    });
    assert_eq!(accounts, 2);
    // The shell follows the data: the first account is selected.
    assert_eq!(
        app.cx.update(|cx| app.shell.read(cx).account(cx)),
        Some(first)
    );

    // Main has character and weapon wishes, but no Chronicled ones: the
    // first banner type with wishes opens instead.
    assert_eq!(
        app.navigate(Page::Wishes(WishPage::History(Pool::CharacterEvent))),
        Page::Wishes(WishPage::History(Pool::CharacterEvent))
    );
    assert_eq!(
        app.navigate(Page::Wishes(WishPage::History(Pool::Chronicled))),
        Page::Wishes(WishPage::History(Pool::CharacterEvent))
    );

    // Importing the same file again changes nothing.
    assert_eq!(app.import(SAMPLE).added(), 0);
}

#[test]
fn the_mirror_folder_gets_a_snapshot_right_away_and_after_imports() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = dir.path().join("Sync");
    let mut app = App::open(&dir.path().join("data"));

    let library = app.library.clone();
    let folder = mirror.clone();
    let set = app.cx.update(|cx| {
        library.update(cx, |library, cx| {
            library.set_mirror_folder(Some(folder), cx)
        })
    });
    app.finish(set).unwrap();
    let file = mirror.join(wish_core::snapshot::MIRROR_FILE);
    assert!(file.exists());
    let written = app.cx.update(|cx| library.read(cx).mirror().cloned());
    assert!(matches!(written, Some(MirrorStatus::Written(_))));

    app.import(SAMPLE);
    let snapshot =
        wish_core::snapshot::Snapshot::from_json(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(snapshot.accounts.len(), 2);

    // The setting survives a restart.
    drop(app);
    let reopened =
        wish_core::settings::Settings::load(&AppPaths::at(dir.path().join("data"))).unwrap();
    assert_eq!(reopened.mirror_folder, Some(mirror));
}

#[test]
fn undoing_an_import_hides_an_account_it_emptied() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::open(dir.path());
    app.import(SAMPLE);

    let library = app.library.clone();
    let alt = app.cx.update(|cx| {
        let imports = library.read(cx).imports();
        assert_eq!(imports.len(), 2);
        imports
            .iter()
            .find(|i| i.account_label == "Account 2")
            .unwrap()
            .id
    });
    let undo = app
        .cx
        .update(|cx| library.update(cx, |library, cx| library.undo_import(alt, cx)));
    let report = app.finish(undo).unwrap();
    assert_eq!(report.removed_wishes, 3);

    app.cx.update(|cx| {
        let library = library.read(cx);
        let labels: Vec<_> = library
            .accounts()
            .iter()
            .map(|a| a.account.label.as_str())
            .collect();
        assert_eq!(labels, ["Main"]);
        let undone = library.imports().iter().find(|i| i.id == alt).unwrap();
        assert!(undone.undone_at.is_some());
    });
    // The shell still shows an account that exists.
    let shown = app.cx.update(|cx| app.shell.read(cx).account(cx));
    let main = app
        .cx
        .update(|cx| library.read(cx).accounts()[0].account.id);
    assert_eq!(shown, Some(main));
}

#[test]
fn counting_by_hand_starts_from_nothing_and_keeps_every_click() {
    use wish_core::import::manual;
    use wish_core::model::{Server, WishTime};

    let dir = tempfile::tempdir().unwrap();
    let mut app = App::open(dir.path());
    let library = app.library.clone();
    let on = app
        .cx
        .update(|cx| library.update(cx, |library, cx| library.set_manual_counting(true, cx)));
    app.finish(on).unwrap();
    let saved = wish_core::settings::Settings::load(&AppPaths::at(dir.path())).unwrap();
    assert!(saved.manual_counting);

    // Every banner page opens while counting, even with no wishes yet.
    assert_eq!(
        app.navigate(Page::Wishes(WishPage::History(Pool::Standard))),
        Page::Wishes(WishPage::History(Pool::Standard))
    );

    // Two clicks before the first is saved: both count, in one new account.
    let time = WishTime::now_on(Server::Asia);
    let (first, second) = app.cx.update(|cx| {
        library.update(cx, |library, cx| {
            (
                library.apply_import(manual::three_stars(None, Pool::Standard, 1, time), cx),
                library.apply_import(manual::three_stars(None, Pool::Standard, 10, time), cx),
            )
        })
    });
    app.finish(first).unwrap();
    app.finish(second).unwrap();

    // A write whose caller went away still finishes.
    drop(app.cx.update(|cx| {
        library.update(cx, |library, cx| {
            library.apply_import(manual::three_stars(None, Pool::Standard, 1, time), cx)
        })
    }));
    app.cx.run_until_parked();

    let (accounts, wishes, busy) = app.cx.update(|cx| {
        let library = library.read(cx);
        let wishes = library.accounts().first().map_or(0, |summary| {
            summary
                .analysis
                .pool(Pool::Standard)
                .map_or(0, |pool| pool.pulls.len())
        });
        (library.accounts().len(), wishes, library.is_busy())
    });
    assert_eq!((accounts, wishes, busy), (1, 12, false));
    assert_eq!(
        app.cx.update(|cx| app.shell.read(cx).page()),
        Page::Wishes(WishPage::History(Pool::Standard))
    );
}

#[test]
fn a_feature_header_opens_its_feature_then_folds_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::open(dir.path());
    let characters = Page::Archive(ArchivePage::Characters);

    // Away from the feature, the header opens it.
    app.navigate(Page::Settings);
    assert_eq!(app.click_group(Feature::Archive), (characters, false));
    // On one of its pages, it folds and unfolds the group.
    assert_eq!(app.click_group(Feature::Archive), (characters, true));
    assert_eq!(app.click_group(Feature::Archive), (characters, false));
    // Opening a page of a folded group unfolds it.
    app.click_group(Feature::Archive);
    app.navigate(characters);
    assert_eq!(app.click_group(Feature::Archive), (characters, true));
    app.navigate(Page::Settings);
    app.navigate(characters);
    let folded = app
        .cx
        .update(|cx| app.shell.read(cx).is_folded(Feature::Archive));
    assert!(!folded);
}

#[test]
fn the_overview_opens_first_and_keeps_its_tab() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::open(dir.path());
    let shell = app.shell.clone();
    assert_eq!(
        app.cx.update(|cx| shell.read(cx).page()),
        Page::Overview(OverviewTab::Wishes)
    );

    // The sidebar's entry opens the tab shown last.
    app.navigate(Page::Overview(OverviewTab::Weapons));
    app.navigate(Page::Settings);
    app.cx
        .update(|cx| shell.update(cx, |shell, cx| shell.open_overview(cx)));
    assert_eq!(
        app.cx.update(|cx| shell.read(cx).page()),
        Page::Overview(OverviewTab::Weapons)
    );

    // The overview is in no group. Without wishes, the wish counter's header
    // opens it; with wishes, the first banner type.
    assert_eq!(
        app.click_group(Feature::Wishes).0,
        Page::Overview(OverviewTab::Wishes)
    );
    app.import(SAMPLE);
    assert_eq!(
        app.click_group(Feature::Wishes).0,
        Page::Wishes(WishPage::History(Pool::CharacterEvent))
    );
}

#[test]
fn the_archive_opens_from_its_group_and_on_a_character() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::open(dir.path());
    let characters = Page::Archive(ArchivePage::Characters);

    assert_eq!(app.click_group(Feature::Archive), (characters, false));
    // A character's page belongs to the same entry of the sidebar.
    let shell = app.shell.clone();
    app.cx
        .update(|cx| shell.update(cx, |shell, cx| shell.open_character("ganyu", cx)));
    app.cx.run_until_parked();
    assert_eq!(app.cx.update(|cx| shell.read(cx).page()), characters);
    // The sidebar entry brings the list back.
    assert_eq!(app.navigate(characters), characters);
}

#[test]
fn a_weapon_opens_in_the_archive_and_joins_the_plan() {
    use wish_core::planner::{PlanTarget, WeaponGoal};

    let dir = tempfile::tempdir().unwrap();
    let mut app = App::open(dir.path());
    let weapons = Page::Archive(ArchivePage::Weapons);
    let shell = app.shell.clone();
    app.cx
        .update(|cx| shell.update(cx, |shell, cx| shell.open_weapon("amosbow", cx)));
    app.cx.run_until_parked();
    assert_eq!(app.cx.update(|cx| shell.read(cx).page()), weapons);
    assert_eq!(app.navigate(weapons), weapons);

    // Added to the plan as its page does: from 1 to the top of its rarity.
    let library = app.library.clone();
    let target = PlanTarget::Weapon {
        key: "amosbow".to_owned(),
        goal: WeaponGoal::full(5),
    };
    let account = app.account();
    let task = app.cx.update(|cx| {
        library.update(cx, |library, cx| {
            library.add_plan_entry(account, target.clone(), cx)
        })
    });
    app.finish(task).expect("the weapon is added");
    let plan = app.plan();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].target, target);
    // The planner page draws it.
    assert_eq!(app.navigate(Page::Planner), Page::Planner);
}

#[test]
fn plan_entries_move_and_keep_their_order() {
    use wish_core::planner::{CharacterGoal, PlanTarget};

    let dir = tempfile::tempdir().unwrap();
    let mut app = App::open(dir.path());
    let library = app.library.clone();
    // No account yet: the first entry makes one, "Main", which the
    // sidebar then shows.
    assert_eq!(app.account(), None);
    for key in ["ganyu", "albedo", "furina"] {
        let target = PlanTarget::Character {
            key: key.to_owned(),
            goal: CharacterGoal::FULL,
        };
        let account = app.account();
        let task = app.cx.update(|cx| {
            library.update(cx, |library, cx| {
                library.add_plan_entry(account, target, cx)
            })
        });
        app.finish(task).expect("the entry is added");
    }
    let accounts = app.cx.update(|cx| {
        let library = library.read(cx);
        let accounts = library.accounts().iter();
        accounts
            .map(|a| a.account.label.clone())
            .collect::<Vec<_>>()
    });
    assert_eq!(accounts, ["Main"], "one account, made by the first entry");
    assert!(app.account().is_some());
    let order = |app: &mut App| -> Vec<i64> { app.plan().iter().map(|e| e.id).collect() };
    let [ganyu, albedo, furina] = order(&mut app)[..] else {
        panic!("three entries");
    };

    // Furina dropped on Ganyu takes her place: first to get the inventory.
    let account = app.account();
    let task = app.cx.update(|cx| {
        library.update(cx, |library, cx| {
            library.move_plan_entry(account, furina, ganyu, cx)
        })
    });
    app.finish(task).expect("the order is saved");
    assert_eq!(order(&mut app), [furina, ganyu, albedo]);

    // Ganyu dropped on Albedo, further down, goes after her.
    let task = app.cx.update(|cx| {
        library.update(cx, |library, cx| {
            library.move_plan_entry(account, ganyu, albedo, cx)
        })
    });
    app.finish(task).expect("the order is saved");
    assert_eq!(order(&mut app), [furina, albedo, ganyu]);

    // Kept when the app opens again.
    drop(app);
    let mut app = App::open(dir.path());
    assert_eq!(order(&mut app), [furina, albedo, ganyu]);
}
