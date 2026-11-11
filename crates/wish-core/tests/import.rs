//! End-to-end checks of the M1 data path: paimon.moe save → store → snapshot.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::Value;
use wish_core::data::GameData;
use wish_core::import::{ImportBatch, paimon};
use wish_core::model::{GachaType, ItemType, ManualCounters, Pool, Rarity, Server, Source};
use wish_core::snapshot::Snapshot;
use wish_core::{AppPaths, Error, Store};

const SAMPLE: &str = include_str!("fixtures/paimon-sample.json");

fn sample_batch() -> ImportBatch {
    paimon::parse(SAMPLE.as_bytes(), GameData::embedded())
        .unwrap()
        .with_original("paimon-sample.json", SAMPLE.as_bytes().to_vec())
}

fn open_store(dir: &Path) -> Store {
    Store::open(AppPaths::at(dir)).unwrap()
}

#[test]
fn paimon_save_is_read_completely() {
    let batch = sample_batch();
    assert!(batch.warnings.is_empty(), "{:?}", batch.warnings);
    assert_eq!(batch.accounts.len(), 2);

    let main = &batch.accounts[0];
    assert_eq!(main.label, "Main");
    assert_eq!(main.uid.as_deref(), Some("700000001"));
    assert_eq!(main.server, Some(Server::Europe));
    assert_eq!(main.wishes.len(), 14);
    // Stored counters match the pull list, so they are not kept.
    assert!(main.manual_counters.is_empty());

    // A 10-pull keeps its order, including the same 3★ twice at the same time.
    let ten_pull: Vec<_> = main
        .wishes
        .iter()
        .filter(|w| w.time.to_string() == "2024-01-01 10:05:00")
        .collect();
    assert_eq!(ten_pull.len(), 10);
    assert_eq!(
        ten_pull.iter().map(|w| w.seq).collect::<Vec<_>>(),
        (0..10).collect::<Vec<_>>()
    );
    assert_eq!(ten_pull[0].name, "Debate Club");
    assert_eq!(ten_pull[1].name, "Debate Club");
    assert_ne!(ten_pull[0].id, ten_pull[1].id);
    assert_eq!(ten_pull[9].name, "Bennett");
    assert_eq!(ten_pull[9].rarity, Rarity::Four);

    let venti = main.wishes.iter().find(|w| w.name == "Venti").unwrap();
    assert_eq!(venti.gacha_type, GachaType::CharacterEvent2);
    assert_eq!(venti.pool(), Pool::CharacterEvent);
    assert_eq!(venti.item_key.as_deref(), Some("venti"));
    assert_eq!(
        venti.manual_pity, None,
        "pity of imported pulls is derived, not kept"
    );
    assert_eq!(venti.source, Source::Paimon);

    let alt = &batch.accounts[1];
    assert_eq!(alt.label, "Account 2");
    assert_eq!(alt.uid, None);
    assert_eq!(alt.server, Some(Server::Asia));
    assert_eq!(alt.wishes[0].item_type, ItemType::Unknown);
    assert_eq!(alt.wishes[0].rarity, Rarity::Three);
    assert_eq!(alt.wishes[1].seq, 1);
    assert_eq!(alt.wishes[2].name, "Diluc");
    assert_eq!(
        alt.wishes[2].manual_pity,
        Some(76),
        "hand-typed pity is source data"
    );
    assert_eq!(
        alt.manual_counters,
        [ManualCounters {
            pool: Pool::Standard,
            total: 80,
            pity5: 3,
            pity4: 5
        }]
    );
}

#[test]
fn importing_twice_adds_nothing_and_backs_up_first() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());

    let first = store.apply_import(sample_batch()).unwrap();
    assert_eq!(first.added(), 17);
    assert!(
        first.backup.is_none(),
        "nothing to back up in an empty database"
    );
    let archived = first.archived.unwrap();
    assert_eq!(fs::read_to_string(&archived).unwrap(), SAMPLE);
    assert!(first.accounts.iter().all(|a| a.created));

    let second = store.apply_import(sample_batch()).unwrap();
    assert_eq!(second.added(), 0);
    assert!(second.warnings.is_empty(), "{:?}", second.warnings);
    assert!(second.accounts.iter().all(|a| !a.created));
    let present: u32 = second
        .accounts
        .iter()
        .flat_map(|a| a.pools.values())
        .map(|c| c.already_present)
        .sum();
    assert_eq!(present, 17);

    let backup = second.backup.unwrap();
    assert!(backup.exists());
    assert_eq!(
        second.archived.unwrap(),
        archived,
        "same file, same archived copy"
    );
    assert_eq!(store.accounts().unwrap().len(), 2);
    assert_eq!(store.wish_count().unwrap(), 17);
}

#[test]
fn snapshot_round_trip_is_lossless() {
    let dir = tempfile::tempdir().unwrap();
    let mut original = open_store(&dir.path().join("a"));
    original.apply_import(sample_batch()).unwrap();

    let exported = Snapshot::from_store(&original).unwrap();
    let file = dir.path().join("mirror/snapshot.json");
    exported.write_to(&file).unwrap();

    let read_back = Snapshot::from_json(&fs::read(&file).unwrap()).unwrap();
    assert_eq!(read_back, exported);

    let mut restored = open_store(&dir.path().join("b"));
    restored.apply_import(read_back.into_batch()).unwrap();
    let re_exported = Snapshot::from_store(&restored).unwrap();
    assert_eq!(re_exported.accounts, exported.accounts);

    // Merging the snapshot back into its source changes nothing.
    let merged = original.apply_import(exported.into_batch()).unwrap();
    assert_eq!(merged.added(), 0);
    assert!(merged.warnings.is_empty(), "{:?}", merged.warnings);
}

#[test]
fn unknown_items_fail_the_whole_import() {
    let save = SAMPLE.replace("\"venti\"", "\"brand_new_character\"");
    let err = paimon::parse(save.as_bytes(), GameData::embedded()).unwrap_err();
    match err {
        Error::UnknownItems(items) => assert_eq!(items, ["character `brand_new_character`"]),
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn a_changed_record_keeps_the_stored_version_and_warns() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());
    store.apply_import(sample_batch()).unwrap();

    // Same position (so same id), different item.
    let edited = SAMPLE.replacen("\"id\": \"slingshot\"", "\"id\": \"harbinger_of_dawn\"", 1);
    let batch = paimon::parse(edited.as_bytes(), GameData::embedded()).unwrap();
    let report = store.apply_import(batch).unwrap();

    assert_eq!(report.added(), 0);
    assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
    assert!(report.warnings[0].contains("Main, Character Event: 1 wish(es) differ"));

    let main = &store.accounts().unwrap()[0];
    let first = store.wishes(main.id).unwrap().remove(0);
    assert_eq!(first.name, "Slingshot");
}

#[test]
fn legacy_numeric_times_are_refused_not_guessed() {
    let save = SAMPLE.replacen("\"2024-01-01 10:00:00\"", "1704103200", 1);
    let err = paimon::parse(save.as_bytes(), GameData::embedded()).unwrap_err();
    assert!(err.to_string().contains("old numeric format"), "{err}");
}

#[test]
fn prepare_recognises_files_by_content() {
    use wish_core::import::{ImportSource, prepare};

    let data = GameData::embedded();
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());

    let paimon = prepare("save.json", SAMPLE.as_bytes().to_vec(), data, &store, None).unwrap();
    assert_eq!(paimon.batch.source, ImportSource::PaimonJson);
    assert!(paimon.paimon_check.unwrap().matches());

    store.apply_import(sample_batch()).unwrap();
    let snapshot = Snapshot::from_store(&store).unwrap().to_json();
    let prepared = prepare("backup.json", snapshot.clone(), data, &store, None).unwrap();
    assert_eq!(prepared.batch.source, ImportSource::Snapshot);
    assert_eq!(prepared.batch.wish_count(), 17);
    assert!(prepared.paimon_check.is_none());

    let uigf = wish_core::uigf::export(&store, data).unwrap();
    let prepared = prepare("uigf.json", uigf.json, data, &store, None).unwrap();
    assert_eq!(prepared.batch.source, ImportSource::Uigf);

    let main = store.accounts().unwrap()[0].clone();
    let analysis =
        wish_core::engine::analyze(store.wishes(main.id).unwrap(), main.server, &[], data);
    let xlsx = wish_core::excel::export(&main, &analysis, data).unwrap();
    let prepared = prepare("history.xlsx", xlsx, data, &store, Some(main.id)).unwrap();
    assert_eq!(prepared.batch.source, ImportSource::PaimonExcel);

    for junk in [&b"not json"[..], br#"{"hello": 1}"#, b"[1, 2]"] {
        let err = prepare("junk", junk.to_vec(), data, &store, None).unwrap_err();
        assert!(matches!(err, Error::UnrecognizedFile), "{err}");
    }

    // A damaged snapshot reports its own problem, not "unrecognised file".
    let damaged =
        String::from_utf8(snapshot)
            .unwrap()
            .replacen("\"rarity\": 3", "\"rarity\": 9", 1);
    let err = prepare("backup.json", damaged.into_bytes(), data, &store, None).unwrap_err();
    assert!(matches!(err, Error::Json(_)), "{err}");
}

/// UIGF out, UIGF back into the same store (nothing new), and into an
/// empty one (everything, with made-up ids kept apart from game ids).
#[test]
fn uigf_round_trips_without_duplicates() {
    use wish_core::import::prepare;

    let data = GameData::embedded();
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(&dir.path().join("a"));
    store.apply_import(sample_batch()).unwrap();

    let export = wish_core::uigf::export(&store, data).unwrap();
    assert_eq!(export.accounts, 1, "Account 2 has no UID");
    assert_eq!(export.without_uid, ["Account 2"]);
    assert_eq!(export.wishes, 14);
    assert_eq!(
        export.made_up_ids, 14,
        "migrated wishes have no game id yet"
    );
    let document: Value = serde_json::from_slice(&export.json).unwrap();
    let first = &document["hk4e"][0]["list"][0];
    assert_eq!(first["item_id"], "15304", "Slingshot's HoYoverse id");
    assert!(wish_core::uigf::is_made_up_id(
        first["id"].as_str().unwrap()
    ));
    assert_eq!(document["hk4e"][0]["timezone"], 1, "Europe");

    // Back into the same store: every wish is recognised.
    let again = prepare("uigf.json", export.json.clone(), data, &store, None).unwrap();
    let report = store.apply_import(again.batch).unwrap();
    assert_eq!(report.added(), 0);
    assert_eq!(
        report.accounts[0].linked, 0,
        "made-up ids are never recorded as game ids"
    );

    // Into an empty store: all of Main's wishes, as migrated wishes.
    let mut fresh = open_store(&dir.path().join("b"));
    let into_fresh = prepare("uigf.json", export.json, data, &fresh, None).unwrap();
    let report = fresh.apply_import(into_fresh.batch).unwrap();
    assert_eq!(report.added(), 14);
    let account = &fresh.accounts().unwrap()[0];
    assert_eq!(account.uid.as_deref(), Some("700000001"));
    assert_eq!(account.server, Some(Server::Europe));
    assert!(
        fresh
            .wishes(account.id)
            .unwrap()
            .iter()
            .all(|w| !w.id.is_api())
    );
}

#[test]
fn mirror_writes_one_snapshot_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(&dir.path().join("data"));
    store.apply_import(sample_batch()).unwrap();

    let mirror = dir.path().join("Sync");
    let path = wish_core::snapshot::write_mirror(&store, &mirror).unwrap();
    wish_core::snapshot::write_mirror(&store, &mirror).unwrap();
    assert_eq!(fs::read_dir(&mirror).unwrap().count(), 1);
    let snapshot = Snapshot::from_json(&fs::read(path).unwrap()).unwrap();
    assert_eq!(snapshot.accounts.len(), 2);
}

#[test]
fn other_json_is_not_mistaken_for_a_paimon_save() {
    let err = paimon::parse(br#"{"format": "something else"}"#, GameData::embedded()).unwrap_err();
    assert!(matches!(err, Error::NotPaimonExport), "{err}");
    let err = Snapshot::from_json(SAMPLE.as_bytes()).unwrap_err();
    assert!(matches!(err, Error::NotSnapshot { .. }), "{err}");
}

#[test]
fn database_from_a_newer_build_is_left_untouched() {
    let dir = tempfile::tempdir().unwrap();
    drop(open_store(dir.path()));
    let db = AppPaths::at(dir.path()).database();
    rusqlite::Connection::open(&db)
        .unwrap()
        .pragma_update(None, "user_version", 99)
        .unwrap();
    let before = fs::read(&db).unwrap();

    let err = Store::open(AppPaths::at(dir.path())).err().unwrap();
    assert!(
        matches!(err, Error::SchemaTooNew { found: 99, .. }),
        "{err}"
    );
    assert_eq!(fs::read(&db).unwrap(), before);
}

/// Runs the same checks on real exports dropped into `tests/fixtures/private/`
/// (git-ignored, since they contain UIDs). Passes trivially when there are none.
#[test]
fn private_exports_import_losslessly() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/private");
    let Ok(entries) = fs::read_dir(&dir) else {
        eprintln!("no private fixtures in {}", dir.display());
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let bytes = fs::read(&path).unwrap();
        let expected = pulls_per_pool(&bytes);

        let batch = paimon::parse(&bytes, GameData::embedded())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let store_dir = tempfile::tempdir().unwrap();
        let mut store = open_store(store_dir.path());
        let report = store.apply_import(batch).unwrap();

        let mut stored: BTreeMap<String, usize> = BTreeMap::new();
        for account in store.accounts().unwrap() {
            for wish in store.wishes(account.id).unwrap() {
                *stored.entry(wish.pool().to_string()).or_default() += 1;
            }
        }
        assert_eq!(stored, expected, "{}: wishes per pool", path.display());

        let snapshot = Snapshot::from_store(&store).unwrap();
        let mut restored = open_store(&store_dir.path().join("restored"));
        restored
            .apply_import(snapshot.clone().into_batch())
            .unwrap();
        assert_eq!(
            Snapshot::from_store(&restored).unwrap().accounts,
            snapshot.accounts,
            "{}: snapshot round trip",
            path.display()
        );
        eprintln!(
            "{}: {} wishes OK, warnings: {:?}",
            path.display(),
            report.added(),
            report.warnings
        );
    }
}

/// Counts the pulls of every `wish-counter-<pool>` entry straight from the JSON.
fn pulls_per_pool(bytes: &[u8]) -> BTreeMap<String, usize> {
    let save: serde_json::Map<String, Value> = serde_json::from_slice(bytes).unwrap();
    let mut counts = BTreeMap::new();
    for (key, value) in &save {
        let Some(pool) = Pool::ALL
            .iter()
            .find(|pool| key.ends_with(&format!("wish-counter-{pool}")))
        else {
            continue;
        };
        let pulls = value["pulls"].as_array().map_or(0, Vec::len);
        if pulls > 0 {
            *counts.entry(pool.to_string()).or_default() += pulls;
        }
    }
    counts
}
