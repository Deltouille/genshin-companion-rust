-- Weapons in the plan.
--
-- A weapon entry is a plan entry with `weapon_key` (a genshin-db key) and its
-- level range in the level columns (the talent columns keep their defaults).
-- The CHECK of 003 wants a `name` on any entry without a `character_key`, and
-- a shipped migration is never edited: a weapon entry's `name` holds its key
-- too. Entries with a `weapon_key` are weapons, whatever their name.
ALTER TABLE plan_entries ADD COLUMN weapon_key TEXT
    CHECK (weapon_key IS NULL OR (character_key IS NULL AND trim(weapon_key) <> ''));
