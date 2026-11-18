//! The write queue's promises (`Library::write`), which keep the player's
//! changes: a write that fails or panics brings back what the store holds,
//! the lock a panic poisoned is taken back, writes run in the order they were
//! made and the newest state wins, a write finishes when its caller lets go,
//! and quitting knows which writes are still under way. Run headless, on a
//! scratch folder, without downloads.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use anyhow::{Result, anyhow};
use gpui_kit::{AppContext as _, Entity, HeadlessAppContext, NoopTextSystem, Task};
use wish_core::AppPaths;
use wish_core::planner::{CharacterGoal, PlanTarget};

use super::{Library, Services};

struct Harness {
    cx: HeadlessAppContext,
    library: Entity<Library>,
}

impl Harness {
    fn open(dir: &Path) -> Self {
        let mut cx = HeadlessAppContext::new(Arc::new(NoopTextSystem::new()));
        let paths = AppPaths::at(dir);
        let library = cx.update(|cx| cx.new(|cx| Library::open(paths, Services::offline(), cx)));
        cx.run_until_parked();
        Self { cx, library }
    }

    fn update<R>(
        &mut self,
        f: impl FnOnce(&mut Library, &mut gpui_kit::Context<Library>) -> R,
    ) -> R {
        let library = self.library.clone();
        self.cx.update(|cx| library.update(cx, f))
    }

    fn read<R>(&mut self, f: impl FnOnce(&Library) -> R) -> R {
        let library = self.library.clone();
        self.cx.update(|cx| f(library.read(cx)))
    }

    /// Runs everything queued, then gives what `task` returned.
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

    /// A write that adds an account labelled `label`.
    fn add_account(&mut self, label: &str) -> Task<Result<()>> {
        let label = label.to_owned();
        self.update(move |library, cx| {
            library.write(
                cx,
                false,
                move |store, _| {
                    store.add_account(&label, None, None)?;
                    Ok(())
                },
                |_, ()| (),
            )
        })
    }

    fn labels(&mut self) -> Vec<String> {
        self.read(|library| {
            (library.accounts().iter())
                .map(|summary| summary.account.label.clone())
                .collect()
        })
    }

    /// The writes whose work on the store hasn't ended: what quitting waits for.
    fn unfinished(&mut self) -> usize {
        self.read(|library| library.unfinished.load(Ordering::SeqCst))
    }
}

fn character(key: &str) -> PlanTarget {
    PlanTarget::Character {
        key: key.to_owned(),
        goal: CharacterGoal::FULL,
    }
}

#[test]
fn a_panic_becomes_the_write_s_error_and_the_next_write_still_works() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Harness::open(dir.path());
    let task = app.update(|library, cx| {
        library.write(
            cx,
            false,
            |store, _| -> Result<()> {
                store.add_account("Before the panic", None, None)?;
                panic!("a bug in a write");
            },
            |_, ()| (),
        )
    });
    let error = app.finish(task).unwrap_err();
    assert!(
        format!("{error:#}").contains("a bug in a write"),
        "{error:#}"
    );
    // The state was read again (the account added before the panic shows),
    // and the app is not left busy.
    assert_eq!(app.labels(), ["Before the panic"]);
    assert!(!app.read(Library::is_busy));
    assert_eq!(app.unfinished(), 0);

    // The lock the panic poisoned is taken back.
    let task = app.add_account("After");
    app.finish(task).unwrap();
    assert_eq!(app.labels(), ["Before the panic", "After"]);
}

#[test]
fn a_failed_write_brings_back_what_the_store_holds() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Harness::open(dir.path());
    for key in ["ganyu", "bennett"] {
        let task = app.update(|library, cx| library.add_plan_entry(None, character(key), cx));
        app.finish(task).unwrap();
    }
    let account = app.read(|library| library.accounts()[0].account.id);
    let order = |app: &mut Harness| -> Vec<PlanTarget> {
        app.read(|library| {
            (library.plan(Some(account)).iter())
                .map(|e| e.target.clone())
                .collect()
        })
    };
    let stored = order(&mut app);
    assert_eq!(stored, [character("ganyu"), character("bennett")]);

    // A page shows a change at once (as dragging a card does), then the
    // write behind it fails.
    let task = app.update(|library, cx| {
        library.holdings.get_mut(&account).unwrap().plan.reverse();
        library.write(
            cx,
            true,
            |_, _| -> Result<()> { Err(anyhow!("the disk is full")) },
            |_, ()| (),
        )
    });
    assert_ne!(order(&mut app), stored);
    let error = app.finish(task).unwrap_err();
    assert_eq!(error.to_string(), "the disk is full");
    assert_eq!(order(&mut app), stored);
    assert!(!app.read(Library::is_busy));
}

#[test]
fn writes_run_in_order_and_the_newest_state_wins() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Harness::open(dir.path());
    // Made in a row without waiting, like clicks: the scheduler (random
    // order, as in tests) may finish them in any order.
    let labels: Vec<String> = (1..=24).map(|n| format!("Account {n}")).collect();
    let tasks: Vec<_> = labels.iter().map(|label| app.add_account(label)).collect();
    assert_eq!(app.unfinished(), labels.len());
    for task in tasks {
        app.finish(task).unwrap();
    }
    // Each ran, in the order made, and what shows is the last one's state.
    assert_eq!(app.labels(), labels);
    assert!(!app.read(Library::is_busy));
    assert_eq!(app.unfinished(), 0);
}

#[test]
fn a_write_finishes_when_its_caller_lets_go() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Harness::open(dir.path());
    let task = app.add_account("Kept");
    drop(task);
    app.cx.run_until_parked();
    assert_eq!(app.labels(), ["Kept"]);
    assert!(!app.read(Library::is_busy));
}

#[test]
fn quitting_knows_every_write_under_way_whatever_its_end() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Harness::open(dir.path());
    let done = app.add_account("Done");
    let failed = app.update(|library, cx| {
        library.write(
            cx,
            false,
            |_, _| -> Result<()> { Err(anyhow!("no")) },
            |_, ()| (),
        )
    });
    let panicked = app.update(|library, cx| {
        library.write(
            cx,
            false,
            |_, _| -> Result<()> { panic!("oops") },
            |_, ()| (),
        )
    });
    // The quit handler waits while this is above 0.
    assert_eq!(app.unfinished(), 3);
    app.cx.run_until_parked();
    assert_eq!(app.unfinished(), 0);
    assert!(app.finish(done).is_ok());
    assert!(app.finish(failed).is_err());
    assert!(app.finish(panicked).is_err());
}

/// Each account's analysis (pity, 50/50, the history's numbers) is kept from
/// one reload to the next: a write works out again only those it changed.
#[test]
fn an_account_is_edited_whole_or_not_at_all_then_removed() {
    use wish_core::model::Server;

    let dir = tempfile::tempdir().unwrap();
    let mut app = Harness::open(dir.path());
    let add = |app: &mut Harness, label: &str, uid: Option<&str>| {
        let (label, uid) = (label.to_owned(), uid.map(str::to_owned));
        let task = app.update(|library, cx| library.add_account(label, uid, None, cx));
        app.finish(task).unwrap().id
    };
    let main = add(&mut app, "Main", None);
    add(&mut app, "Alt", Some("800000001"));

    // A taken UID: the name doesn't change either.
    let identity = Some((Some("800000001".to_owned()), None));
    let task =
        app.update(|library, cx| library.edit_account(main, "Renamed".to_owned(), identity, cx));
    assert!(app.finish(task).is_err());
    assert_eq!(app.labels(), ["Main", "Alt"]);

    // A free one: both, and the server it says.
    let identity = Some((Some("700000001".to_owned()), Some(Server::Asia)));
    let task =
        app.update(|library, cx| library.edit_account(main, "Renamed".to_owned(), identity, cx));
    app.finish(task).unwrap();
    assert_eq!(app.labels(), ["Renamed", "Alt"]);
    let account = app.read(|library| library.account(main).unwrap().account.clone());
    assert_eq!(account.uid.as_deref(), Some("700000001"));
    assert_eq!(account.server, Some(Server::Europe));

    // Removed: gone from the list.
    let task = app.update(|library, cx| library.remove_account(main, cx));
    app.finish(task).unwrap();
    assert_eq!(app.labels(), ["Alt"]);
}

#[test]
fn a_write_works_out_again_only_the_analyses_it_changed() {
    use wish_core::data::GameData;
    use wish_core::engine::Analysis;
    use wish_core::import::{manual, paimon};
    use wish_core::model::{AccountId, Pool};

    const SAMPLE: &str = include_str!("../../../../wish-core/tests/fixtures/paimon-sample.json");
    let dir = tempfile::tempdir().unwrap();
    let mut app = Harness::open(dir.path());
    let batch = paimon::parse(SAMPLE.as_bytes(), GameData::embedded()).unwrap();
    let task = app.update(|library, cx| library.apply_import(batch, cx));
    app.finish(task).unwrap();
    let analyses = |app: &mut Harness| -> Vec<(AccountId, Arc<Analysis>)> {
        app.read(|library| {
            (library.accounts().iter())
                .map(|summary| (summary.account.id, summary.analysis.clone()))
                .collect()
        })
    };
    let kept = |before: &[(AccountId, Arc<Analysis>)], after: &[(AccountId, Arc<Analysis>)]| {
        (before.iter().zip(after))
            .map(|((_, a), (_, b))| Arc::ptr_eq(a, b))
            .collect::<Vec<bool>>()
    };
    let before = analyses(&mut app);
    assert_eq!(before.len(), 2);

    // The plan: no analysis worked out again.
    let task = app.update(|library, cx| library.add_plan_entry(None, character("ganyu"), cx));
    app.finish(task).unwrap();
    let after = analyses(&mut app);
    assert_eq!(kept(&before, &after), [true, true]);

    // A wish counted by hand: its account's only, with the wish in it.
    let standard = |analysis: &Analysis| {
        (analysis.pools.iter())
            .find(|pool| pool.pool == Pool::Standard)
            .map_or(0, |pool| pool.pulls.len())
    };
    let account = app.read(|library| library.accounts()[0].account.clone());
    let time = "2024-06-01 12:00:00".parse().unwrap();
    let counted = manual::three_stars(Some(&account), Pool::Standard, 1, time);
    let task = app.update(|library, cx| library.apply_import(counted, cx));
    app.finish(task).unwrap();
    let counted = analyses(&mut app);
    assert_eq!(kept(&after, &counted), [false, true]);
    assert_eq!(standard(&counted[0].1), standard(&after[0].1) + 1);

    // A failed write: every one, to be safe.
    let task = app.update(|library, cx| {
        library.write(
            cx,
            false,
            |_, _| -> Result<()> { Err(anyhow!("no")) },
            |_, ()| (),
        )
    });
    app.finish(task).unwrap_err();
    assert_eq!(kept(&counted, &analyses(&mut app)), [false, false]);
}

/// The mirror is written once the changes stop for a few seconds, and when
/// the app quits; never by each write.
mod mirror {
    use std::time::Duration;

    use wish_core::snapshot::{MIRROR_FILE, Snapshot};

    use super::super::mirror::MIRROR_DELAY;
    use super::*;

    /// A library mirroring into `<dir>/Sync`, and the accounts the mirror
    /// file holds.
    fn mirrored(dir: &Path) -> (Harness, impl Fn() -> usize) {
        let mut app = Harness::open(&dir.join("data"));
        let folder = dir.join("Sync");
        let set = app.update(|library, cx| library.set_mirror_folder(Some(folder.clone()), cx));
        app.finish(set).unwrap();
        let file = folder.join(MIRROR_FILE);
        let accounts = move || {
            let json = std::fs::read(&file).unwrap();
            Snapshot::from_json(&json).unwrap().accounts.len()
        };
        (app, accounts)
    }

    /// A change of the data, as the app makes it.
    fn add(app: &mut Harness, label: &str) {
        let label = label.to_owned();
        let task = app.update(|library, cx| library.add_account(label, None, None, cx));
        app.finish(task).unwrap();
    }

    fn wait(app: &mut Harness, time: Duration) {
        app.cx.advance_clock(time);
        app.cx.run_until_parked();
    }

    #[test]
    fn each_change_starts_the_wait_again() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, accounts) = mirrored(dir.path());
        assert_eq!(accounts(), 0);

        let second = Duration::from_secs(1);
        add(&mut app, "First");
        wait(&mut app, MIRROR_DELAY - second);
        assert_eq!(accounts(), 0, "not yet");
        // Another change before the wait ends: it starts again.
        add(&mut app, "Second");
        wait(&mut app, MIRROR_DELAY - second);
        assert_eq!(accounts(), 0, "still not");
        wait(&mut app, second);
        assert_eq!(accounts(), 2, "both, in one write");
        assert!(matches!(
            app.read(|library| library.mirror().cloned()),
            Some(super::super::MirrorStatus::Written(_))
        ));
    }

    #[test]
    fn quitting_writes_the_mirror_a_change_still_waits_for() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, accounts) = mirrored(dir.path());
        add(&mut app, "Just now");
        assert_eq!(accounts(), 0);
        app.cx.update(|cx| cx.shutdown());
        assert_eq!(accounts(), 1);
    }

    #[test]
    fn a_backup_by_hand_or_a_failed_write_is_no_change_to_mirror() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, accounts) = mirrored(dir.path());
        let task = app.update(|library, cx| library.back_up(cx));
        app.finish(task).unwrap();
        // A change of the data that fails: rolled back, nothing to mirror.
        let task = app.update(|library, cx| {
            library.write(
                cx,
                true,
                |_, _| -> Result<()> { Err(anyhow!("no")) },
                |_, ()| (),
            )
        });
        app.finish(task).unwrap_err();
        wait(&mut app, MIRROR_DELAY * 2);
        assert!(app.read(|library| library.mirror().cloned()).is_some());
        assert_eq!(accounts(), 0);
    }
}

/// The two flows that bring data from the network, end to end through the
/// library, the core's services and the store, on fake transports.
mod flows {
    use std::sync::Mutex;

    use serde_json::{Value, json};
    use wish_core::hoyolab;
    use wish_core::import::api::{self, GachaLink};
    use wish_core::model::AccountId;

    use super::super::FetchControl;
    use super::*;

    /// HoYoLAB, answering by the end of the URL.
    struct FakeHoyolab {
        answers: Vec<(&'static str, Value)>,
        /// The URLs asked, in order.
        asked: Mutex<Vec<String>>,
    }

    impl hoyolab::Transport for FakeHoyolab {
        fn send(&self, request: &hoyolab::Request) -> hoyolab::Result<String> {
            self.asked.lock().unwrap().push(request.url.clone());
            (self.answers.iter())
                .find(|(end, _)| request.url.ends_with(end))
                .map(|(_, data)| json!({ "retcode": 0, "message": "OK", "data": data }).to_string())
                .ok_or_else(|| hoyolab::Error::Network(format!("no answer for {}", request.url)))
        }
    }

    /// One Genshin account, UID 712345678, with Ganyu holding Amos' Bow.
    fn hoyolab() -> FakeHoyolab {
        let weapon = json!({ "id": 15502, "name": "Amos' Bow", "rarity": 5, "level": 90,
                             "affix_level": 1, "promote_level": 6, "type": 12 });
        FakeHoyolab {
            answers: vec![
                (
                    "getUserGameRolesByCookie",
                    json!({ "list": [{ "game_biz": "hk4e_global", "region": "os_euro",
                        "game_uid": "712345678", "nickname": "Traveler", "level": 60 }] }),
                ),
                (
                    "character/list",
                    json!({ "list": [{ "id": 10000037, "name": "Ganyu", "element": "Cryo",
                        "rarity": 5, "level": 90, "actived_constellation_num": 1,
                        "weapon": weapon }] }),
                ),
                (
                    "character/detail",
                    json!({ "list": [{
                        "base": { "id": 10000037, "name": "Ganyu", "element": "Cryo",
                                  "rarity": 5, "level": 90, "actived_constellation_num": 1 },
                        "weapon": weapon,
                        "constellations": [{ "id": 371, "pos": 1, "name": "Dew-Drinker",
                                             "effect": "…", "is_actived": true }],
                        "skills": [
                            { "skill_id": 10371, "skill_type": 1, "name": "Liutian Archery", "level": 6 },
                            { "skill_id": 10372, "skill_type": 1, "name": "Trail of the Qilin", "level": 9 },
                            { "skill_id": 10375, "skill_type": 1, "name": "Celestial Shower", "level": 9 }
                        ]
                    }] }),
                ),
            ],
            asked: Mutex::new(Vec::new()),
        }
    }

    fn cookie() -> hoyolab::Cookie {
        hoyolab::Cookie::parse("ltuid_v2=123; ltoken_v2=v2_token; ltmid_v2=mid").unwrap()
    }

    fn open_with(dir: &Path, services: Services) -> Harness {
        let mut cx = HeadlessAppContext::new(Arc::new(NoopTextSystem::new()));
        let paths = AppPaths::at(dir);
        let library = cx.update(|cx| cx.new(|cx| Library::open(paths, services, cx)));
        cx.run_until_parked();
        Harness { cx, library }
    }

    fn preview(app: &mut Harness, shown: Option<AccountId>) -> Vec<super::super::HoyolabAccount> {
        let control = Arc::new(FetchControl::default());
        let task =
            app.update(|library, cx| library.fetch_from_hoyolab(cookie(), shown, control, cx));
        app.finish(task).unwrap()
    }

    #[test]
    fn a_hoyolab_sync_goes_where_the_player_chose_and_gives_it_the_uid() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Arc::new(hoyolab());
        let services = Services {
            hoyolab: fake.clone(),
            ..Services::offline()
        };
        let mut app = open_with(dir.path(), services);
        let task = app.add_account("Main");
        app.finish(task).unwrap();
        let main = app.read(|library| library.accounts()[0].account.id);

        // Offered: a new account, or "Main", which has no UID and is shown.
        let fetched = preview(&mut app, Some(main));
        let [account] = fetched.as_slice() else {
            panic!("one Genshin account");
        };
        let targets: Vec<Option<AccountId>> = (account.choices.iter())
            .map(|choice| choice.target.as_ref().map(|a| a.id))
            .collect();
        assert_eq!(targets, [None, Some(main)]);
        assert_eq!(account.suggested, 1);
        assert!(account.choices[1].plan.changes() > 0);
        assert_eq!(fake.asked.lock().unwrap().len(), 3);

        let sync = account.clone();
        let task = app.update(|library, cx| library.apply_hoyolab_sync(sync, Some(main), cx));
        let (into, plan) = app.finish(task).unwrap();
        assert_eq!(into, main);
        assert!(plan.changes() > 0);
        app.read(|library| {
            let summary = &library.accounts()[0];
            assert_eq!(summary.account.uid.as_deref(), Some("712345678"));
            let owned = library.owned(Some(main));
            let ganyu = owned.character("ganyu").unwrap();
            assert_eq!((ganyu.level.level, ganyu.constellation), (90, 1));
            assert_eq!(ganyu.talents, [6, 9, 9]);
            let amos = owned.copies("amosbow").next().unwrap();
            assert_eq!(amos.holder.as_deref(), Some("ganyu"));
        });

        // Next time, only the account with that UID, and nothing to change.
        let again = preview(&mut app, None);
        assert_eq!(again[0].choices.len(), 1);
        assert_eq!(again[0].choices[0].plan.changes(), 0);
    }

    #[test]
    fn a_hoyolab_refusal_reaches_the_player_in_hoyolab_s_words() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = open_with(dir.path(), Services::offline());
        let control = Arc::new(FetchControl::default());
        let task =
            app.update(|library, cx| library.fetch_from_hoyolab(cookie(), None, control, cx));
        let error = app.finish(task).unwrap_err();
        assert!(
            format!("{error:#}").contains("Couldn't reach HoYoLAB"),
            "{error:#}"
        );
    }

    /// The game's wish history API: the rows of each banner type, newest
    /// first, on one page.
    struct FakeGame {
        rows: Vec<Value>,
        asked: Mutex<u32>,
    }

    impl api::Transport for FakeGame {
        fn get(&self, url: &str) -> Result<String, String> {
            *self.asked.lock().unwrap() += 1;
            let param = |name: &str| {
                (url.split(['?', '&']))
                    .find_map(|pair| pair.strip_prefix(&format!("{name}=")))
                    .unwrap_or_default()
                    .to_owned()
            };
            let (gacha_type, end_id) = (param("gacha_type"), param("end_id"));
            let list: Vec<&Value> = (self.rows.iter())
                .filter(|row| row["gacha_type"] == gacha_type.as_str() && end_id == "0")
                .collect();
            Ok(json!({ "retcode": 0, "message": "OK",
                "data": { "page": "1", "size": "20", "region": "os_euro", "list": list } })
            .to_string())
        }
    }

    fn row(id: &str, time: &str, name: &str, item_type: &str, rank: &str) -> Value {
        json!({ "uid": "712345678", "gacha_type": "301", "item_id": "", "count": "1",
                "time": time, "name": name, "lang": "en-us", "item_type": item_type,
                "rank_type": rank, "id": id })
    }

    const LINK: &str = "https://webstatic-sea.hoyoverse.com/genshin/event/e20190909gacha-v3/index.html\
        ?win_mode=fullscreen&authkey_ver=1&sign_type=2&auth_appid=webview_gacha&init_type=301\
        &gacha_id=abc&lang=fr&region=os_euro&authkey=ab%2Bcd%2F%3D&game_biz=hk4e_global#/log";

    fn fetch(app: &mut Harness) -> wish_core::import::api_plan::ApiPlan {
        let link = GachaLink::parse(LINK).unwrap();
        let control = Arc::new(FetchControl::default());
        let task = app.update(|library, cx| library.fetch_from_game(link, control, cx));
        app.finish(task).unwrap()
    }

    #[test]
    fn an_import_from_the_game_fetches_plans_writes_and_stops_at_what_is_stored() {
        let dir = tempfile::tempdir().unwrap();
        let game = Arc::new(FakeGame {
            rows: vec![
                row(
                    "1712345678000000002",
                    "2024-06-02 10:00:01",
                    "Xiangling",
                    "Character",
                    "4",
                ),
                row(
                    "1712345678000000001",
                    "2024-06-02 10:00:00",
                    "Slingshot",
                    "Weapon",
                    "3",
                ),
            ],
            asked: Mutex::new(0),
        });
        let services = Services {
            game: game.clone(),
            ..Services::offline()
        };
        let mut app = open_with(dir.path(), services);

        let plan = fetch(&mut app);
        assert_eq!(plan.uid.as_deref(), Some("712345678"));
        let target = &plan.targets[plan.suggested];
        assert_eq!(target.new_total(), 2);
        let batch = target.batch.clone();
        let task = app.update(|library, cx| library.apply_import(batch, cx));
        app.finish(task).unwrap();
        app.read(|library| {
            let summary = &library.accounts()[0];
            assert_eq!(summary.account.uid.as_deref(), Some("712345678"));
            let wishes: usize = summary.analysis.pools.iter().map(|p| p.pulls.len()).sum();
            assert_eq!(wishes, 2);
        });

        // Fetched again: the stored wishes stop it, nothing new.
        let again = fetch(&mut app);
        assert_eq!(again.targets[again.suggested].new_total(), 0);
        assert!(
            *game.asked.lock().unwrap() >= 10,
            "every banner type asked twice"
        );
    }
}
