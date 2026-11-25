# Frozen witnesses

Files the app wrote outside its database, as players have them on disk. `tests/formats.rs` checks that each one
still reads, and that the app still writes the current one.

- `snapshot-v1.json`: a snapshot (and mirror file) of version 1, written by Genshin Companion 0.5.0 itself.
- `snapshot-v2.json`: version 2, written by 0.6.0: the same accounts, with characters, weapons, plans (every kind
  of entry, "?" placeholders included) and inventories.
- `snapshot-v3.json`: version 3, written by the code that introduced it, before its release raised the version
  number (so its `app_version` says 0.10.0): version 2's accounts, with a chronicle (a Spiral Abyss period, an
  Imaginarium Theater period, a Traveler's Diary month with its log), HoYoLAB's answers kept whole.
- `settings-v0.6.json`: `settings.json` as 0.6.0 writes it, every field set.

The data is synthetic (the UID is `paimon-sample.json`'s). **Never edit or regenerate these files**: a change of
format adds a new witness, written by the version that introduces it, and keeps the old ones.
