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

use super::{ImageDownloads, Library};

struct Harness {
    cx: HeadlessAppContext,
    library: Entity<Library>,
}

impl Harness {
    fn open(dir: &Path) -> Self {
        let mut cx = HeadlessAppContext::new(Arc::new(NoopTextSystem::new()));
        let paths = AppPaths::at(dir);
        let library = cx.update(|cx| cx.new(|cx| Library::open(paths, ImageDownloads::Off, cx)));
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
