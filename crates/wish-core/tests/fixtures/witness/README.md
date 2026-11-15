# Frozen witnesses

Files the app wrote outside its database, as players have them on disk. `tests/formats.rs` checks that each one
still reads, and that the app still writes the current one.

- `snapshot-v1.json`: a snapshot (and mirror file) of version 1, written by Genshin Companion 0.5.0 itself.
- `snapshot-v2.json`: version 2, written by 0.6.0: the same accounts, with characters, weapons, plans (every kind
  of entry, "?" placeholders included) and inventories.
- `settings-v0.6.json`: `settings.json` as 0.6.0 writes it, every field set.

The data is synthetic (the UID is `paimon-sample.json`'s). **Never edit or regenerate these files**: a change of
format adds a new witness, written by the version that introduces it, and keeps the old ones.
