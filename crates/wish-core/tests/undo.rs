//! Undoing imports, game ids recorded for migrated wishes, and upgrading a
//! database written by an earlier version.

use std::path::Path;

use wish_core::data::GameData;
use wish_core::import::{AccountBatch, ImportBatch, ImportSource, WishLink, paimon};
use wish_core::model::{GachaType, ItemType, Rarity, Source, Wish, WishId};
use wish_core::snapshot::Snapshot;
use wish_core::store::BackupReason;
use wish_core::{AppPaths, Error, Store};

const SAMPLE: &str = include_str!("fixtures/paimon-sample.json");

fn open_store(dir: &Path) -> Store {
    Store::open(AppPaths::at(dir)).unwrap()
}

fn sample_batch() -> ImportBatch {
    paimon::parse(SAMPLE.as_bytes(), GameData::embedded()).unwrap()
}

/// What a game import of UID 700000001 would bring: one new wish, and the
/// game's id for the first migrated one.
fn api_batch(store: &Store) -> ImportBatch {
    let main = &store.accounts().unwrap()[0];
    let first = store.wishes(main.id).unwrap().remove(0);
    let mut batch = ImportBatch::new(ImportSource::Api);
    batch.accounts.push(AccountBatch {
        label: main.label.clone(),
        uid: main.uid.clone(),
        wishes: vec![Wish {
            id: WishId::from_api("1700000000000000001").unwrap(),
            gacha_type: GachaType::CharacterEvent,
            time: "2024-05-01 10:00:00".parse().unwrap(),
            seq: 0,
            name: "Slingshot".to_owned(),
            item_type: ItemType::Weapon,
            rarity: Rarity::Three,
            item_key: Some("slingshot".to_owned()),
            manual_pity: None,
            source: Source::Api,
        }],
        links: vec![WishLink {
            wish_id: first.id,
            api_id: WishId::from_api("1600000000000000001").unwrap(),
        }],
        ..AccountBatch::default()
    });
    batch
}

#[test]
fn undo_removes_what_an_import_added_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());
    store.apply_import(sample_batch()).unwrap();
    let report = store.apply_import(api_batch(&store)).unwrap();
    assert_eq!((report.added(), report.accounts[0].linked), (1, 1));

    let known = store.known_api_ids().unwrap();
    assert_eq!(
        known["700000001"].len(),
        2,
        "the API wish and the recorded link"
    );

    // Newest first: the game import, then the two accounts of the paimon import.
    let imports = store.recent_imports(10).unwrap();
    assert_eq!(imports.len(), 3);
    assert_eq!(imports[0].source, ImportSource::Api);
    let paimon_main = imports
        .iter()
        .find(|i| i.account_label == "Main" && i.source == ImportSource::PaimonJson)
        .unwrap();
    let paimon_alt = imports
        .iter()
        .find(|i| i.account_label == "Account 2")
        .unwrap();

    // Undoing the paimon import of Main removes its 14 wishes, and the link
    // the game import recorded on one of them, but keeps the game's wish.
    let undo = store.undo_import(paimon_main.id).unwrap();
    assert_eq!((undo.removed_wishes, undo.removed_links), (14, 1));
    assert!(undo.backup.exists());
    let main = &store.accounts().unwrap()[0];
    let left = store.wishes(main.id).unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].source, Source::Api);
    assert!(store.links(main.id).unwrap().is_empty());

    // Account 2's manual counters go with its own import.
    let undo = store.undo_import(paimon_alt.id).unwrap();
    assert_eq!((undo.removed_wishes, undo.removed_counters), (3, 1));

    assert!(matches!(
        store.undo_import(paimon_alt.id),
        Err(Error::AlreadyUndone(_))
    ));
    assert!(matches!(
        store.undo_import(999),
        Err(Error::UnknownImport(999))
    ));
    let backups = store.backups().unwrap();
    assert!(
        backups
            .iter()
            .any(|b| b.reason == Some(BackupReason::BeforeUndo))
    );

    // The paimon wishes can be imported again.
    assert_eq!(store.apply_import(sample_batch()).unwrap().added(), 17);
}

#[test]
fn links_survive_a_snapshot_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(&dir.path().join("a"));
    store.apply_import(sample_batch()).unwrap();
    store.apply_import(api_batch(&store)).unwrap();
    let exported = Snapshot::from_store(&store).unwrap();
    assert_eq!(exported.accounts[0].links.len(), 1);

    let mut restored = open_store(&dir.path().join("b"));
    restored
        .apply_import(exported.clone().into_batch())
        .unwrap();
    assert_eq!(
        Snapshot::from_store(&restored).unwrap().accounts,
        exported.accounts
    );
}

#[test]
fn merging_into_an_account_of_another_uid_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());
    store.apply_import(sample_batch()).unwrap();
    let main = store.accounts().unwrap()[0].clone();

    let mut batch = ImportBatch::new(ImportSource::Api);
    batch.accounts.push(AccountBatch {
        label: "800000002".to_owned(),
        uid: Some("800000002".to_owned()),
        merge_into: Some(main.id),
        ..AccountBatch::default()
    });
    let err = store.apply_import(batch).unwrap_err();
    assert!(matches!(err, Error::AccountMismatch { .. }), "{err}");
}

#[test]
fn a_database_from_the_first_version_is_upgraded_after_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::at(dir.path());
    {
        // Schema version 1, as M1 wrote it, with one account.
        let conn = rusqlite::Connection::open(paths.database()).unwrap();
        conn.execute_batch(include_str!("../src/store/schema/001_initial.sql"))
            .unwrap();
        conn.execute(
            "INSERT INTO accounts (label, uid, server, created_at) VALUES ('Main', '700000001', 'europe', '2026-11-09T12:00:00+01:00')",
            [],
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
    }

    let mut store = Store::open(paths).unwrap();
    let backups = store.backups().unwrap();
    assert!(
        backups
            .iter()
            .any(|b| b.reason == Some(BackupReason::BeforeMigration))
    );
    assert_eq!(store.accounts().unwrap()[0].label, "Main");
    // The new tables and columns work.
    store.apply_import(sample_batch()).unwrap();
    store.apply_import(api_batch(&store)).unwrap();
    assert_eq!(store.recent_imports(10).unwrap().len(), 3);
}

#[test]
fn a_paimon_save_imported_after_the_game_adds_no_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());

    // The game's records of the sample's first two character wishes, stored first.
    let mut batch = ImportBatch::new(ImportSource::Api);
    let game_wish = |id: &str, time: &str, seq: u32| Wish {
        id: WishId::from_api(id).unwrap(),
        gacha_type: GachaType::CharacterEvent,
        time: time.parse().unwrap(),
        seq,
        name: if seq == 0 && time.ends_with("10:00:00") {
            "Slingshot"
        } else {
            "Debate Club"
        }
        .to_owned(),
        item_type: ItemType::Weapon,
        rarity: Rarity::Three,
        item_key: None,
        manual_pity: None,
        source: Source::Api,
    };
    batch.accounts.push(AccountBatch {
        label: "700000001".to_owned(),
        uid: Some("700000001".to_owned()),
        wishes: vec![
            game_wish("1704103200000000001", "2024-01-01 10:00:00", 0),
            // The 10-pull holds two Debate Clubs; the game import has one.
            game_wish("1704103500000000002", "2024-01-01 10:05:00", 0),
        ],
        ..AccountBatch::default()
    });
    store.apply_import(batch).unwrap();

    // The paimon save holds the same two wishes, and 12 more for Main.
    let report = store.apply_import(sample_batch()).unwrap();
    let main = report
        .accounts
        .iter()
        .find(|a| a.account.uid.as_deref() == Some("700000001"))
        .unwrap();
    let counts = main.pools[&wish_core::model::Pool::CharacterEvent];
    assert_eq!((counts.added, counts.already_present), (11, 2));
    assert_eq!(store.wishes(main.account.id).unwrap().len(), 14);
}

fn main_wish_count(store: &Store) -> usize {
    let main = &store.accounts().unwrap()[0];
    store.wishes(main.id).unwrap().len()
}

#[test]
fn restoring_a_backup_replaces_the_data_after_a_safety_backup() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());
    store.apply_import(sample_batch()).unwrap();
    let before = store.backup(BackupReason::Manual).unwrap();
    store.apply_import(api_batch(&store)).unwrap();
    assert_eq!(main_wish_count(&store), 15);

    let safety = store.restore(&before).unwrap();
    assert_eq!(main_wish_count(&store), 14);
    // The replaced state is kept, and can be restored in turn.
    let backups = store.backups().unwrap();
    assert!(
        backups
            .iter()
            .any(|b| b.path == safety && b.reason == Some(BackupReason::BeforeRestore))
    );
    store.restore(&safety).unwrap();
    assert_eq!(main_wish_count(&store), 15);

    let junk = dir.path().join("notes.db");
    std::fs::write(&junk, b"not a database").unwrap();
    assert!(matches!(store.restore(&junk), Err(Error::NotABackup(_))));
    assert_eq!(
        main_wish_count(&store),
        15,
        "a refused restore changes nothing"
    );
}

#[test]
fn a_backup_with_a_trigger_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());
    store.apply_import(sample_batch()).unwrap();
    let trapped = store.backup(BackupReason::Manual).unwrap();
    {
        let conn = rusqlite::Connection::open(&trapped).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER wipe AFTER INSERT ON wishes BEGIN DELETE FROM wishes; END;",
        )
        .unwrap();
    }
    assert!(matches!(store.restore(&trapped), Err(Error::NotABackup(_))));
    assert_eq!(
        main_wish_count(&store),
        14,
        "a refused restore changes nothing"
    );
}

#[test]
fn the_oldest_backup_can_be_restored_when_the_folder_is_full() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());
    store.apply_import(sample_batch()).unwrap();
    for _ in 0..wish_core::store::KEEP_BACKUPS {
        store.backup(BackupReason::Manual).unwrap();
    }
    store.apply_import(api_batch(&store)).unwrap();
    // Full: the import's own backup pushed out the first manual one.
    let oldest = store.backups().unwrap().pop().unwrap();
    assert_eq!(oldest.reason, Some(BackupReason::Manual));

    store.restore(&oldest.path).unwrap();
    assert_eq!(main_wish_count(&store), 14);
    assert_eq!(
        store.backups().unwrap().len(),
        wish_core::store::KEEP_BACKUPS
    );
}

#[test]
fn a_backup_from_the_first_version_is_migrated_when_restored() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old.db");
    {
        let conn = rusqlite::Connection::open(&old).unwrap();
        conn.execute_batch(include_str!("../src/store/schema/001_initial.sql"))
            .unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
    }
    let mut store = open_store(&dir.path().join("data"));
    store.apply_import(sample_batch()).unwrap();
    store.restore(&old).unwrap();
    assert!(store.accounts().unwrap().is_empty());
    // The schema is current again: links and undo work.
    store.apply_import(sample_batch()).unwrap();
    store.apply_import(api_batch(&store)).unwrap();
}

#[test]
fn wishes_counted_by_hand_add_up_and_can_be_removed() {
    use wish_core::engine;
    use wish_core::import::manual;
    use wish_core::model::{Pool, WishTime};

    let data = GameData::embedded();
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());
    store.apply_import(sample_batch()).unwrap();
    let alt = store
        .accounts()
        .unwrap()
        .into_iter()
        .find(|a| a.label == "Account 2")
        .unwrap();

    // Account 2 counts Standard by hand: +10, then a 5★.
    let time: WishTime = "2024-04-01 10:00:00".parse().unwrap();
    let ten = store
        .apply_import(manual::three_stars(Some(&alt), Pool::Standard, 10, time))
        .unwrap();
    assert_eq!(ten.added(), 10);
    let later: WishTime = "2024-04-01 10:05:00".parse().unwrap();
    let diluc = data.items.by_key("diluc").unwrap();
    store
        .apply_import(manual::item(
            Some(&alt),
            GachaType::Standard,
            diluc,
            None,
            later,
        ))
        .unwrap();

    let wishes = store.wishes(alt.id).unwrap();
    assert_eq!(wishes.len(), 3 + 11);
    let analysis = engine::analyze(wishes.clone(), alt.server, &[], data);
    let standard = analysis.pool(Pool::Standard).unwrap();
    assert_eq!(
        standard.pulls.last().unwrap().pity,
        Some(11),
        "10 three-stars, then the 5★"
    );

    // Only hand-entered wishes can be deleted.
    let migrated = wishes
        .iter()
        .find(|w| w.source == Source::Paimon)
        .unwrap()
        .id
        .clone();
    assert!(matches!(
        store.delete_manual_wishes(alt.id, &[migrated]),
        Err(Error::NotManual(_))
    ));
    let last = wishes.last().unwrap().id.clone();
    assert_eq!(store.delete_manual_wishes(alt.id, &[last]).unwrap(), 1);
    assert_eq!(store.wishes(alt.id).unwrap().len(), 13);
    assert!(
        store
            .backups()
            .unwrap()
            .iter()
            .any(|b| b.reason == Some(BackupReason::BeforeDelete))
    );

    // The +10 is an import, so it can be undone.
    let plus_ten = store
        .recent_imports(10)
        .unwrap()
        .into_iter()
        .find(|i| i.detail.as_deref() == Some("+10"))
        .unwrap();
    store.undo_import(plus_ten.id).unwrap();
    assert_eq!(store.wishes(alt.id).unwrap().len(), 3);
}

#[test]
fn counting_by_hand_from_scratch_creates_one_account() {
    use wish_core::import::manual;
    use wish_core::model::{Pool, WishTime};

    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());
    let time: WishTime = "2024-04-01 10:00:00".parse().unwrap();
    store
        .apply_import(manual::three_stars(None, Pool::CharacterEvent, 1, time))
        .unwrap();
    store
        .apply_import(manual::three_stars(None, Pool::WeaponEvent, 10, time))
        .unwrap();

    let accounts = store.accounts().unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].label, manual::NEW_ACCOUNT_LABEL);
    assert_eq!(store.wishes(accounts[0].id).unwrap().len(), 11);
}

#[test]
fn counting_by_hand_backs_up_once_until_something_else_writes() {
    use wish_core::import::manual;
    use wish_core::model::{Pool, WishTime};

    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());
    store.apply_import(sample_batch()).unwrap();
    let account = store.accounts().unwrap().into_iter().next().unwrap();
    let time: WishTime = "2024-04-01 10:00:00".parse().unwrap();
    let counting_backups = |store: &Store| {
        store
            .backups()
            .unwrap()
            .iter()
            .filter(|b| b.reason == Some(BackupReason::BeforeCounting))
            .count()
    };

    // A burst of clicks shares one backup...
    for _ in 0..5 {
        store
            .apply_import(manual::three_stars(Some(&account), Pool::Standard, 1, time))
            .unwrap();
    }
    assert_eq!(counting_backups(&store), 1);

    // ...until another kind of write backs up, after which counting backs up again.
    store.apply_import(api_batch(&store)).unwrap();
    store
        .apply_import(manual::three_stars(Some(&account), Pool::Standard, 1, time))
        .unwrap();
    assert_eq!(counting_backups(&store), 2);
}

#[test]
fn now_on_a_server_is_its_wall_clock() {
    use wish_core::model::{Server, WishTime};
    let america = WishTime::now_on(Server::America).naive();
    let asia = WishTime::now_on(Server::Asia).naive();
    let gap = (asia - america).num_hours();
    assert!(
        (12..=13).contains(&gap),
        "Asia is 13 hours ahead of America ({gap})"
    );
}
