-- Custom characters in the plan, and the player's inventory.

-- A custom character is a plan entry with a name (like the custom entries of
-- 003) and its stars; its goal uses the level and talent columns, and its
-- materials are in `plan_custom_materials`. Entries without stars are
-- characters of the game data or the custom entries of 003.
ALTER TABLE plan_entries ADD COLUMN stars INTEGER CHECK (stars IS NULL OR stars IN (4, 5));

-- The material a custom character uses for each role (`common`,
-- `local-specialty`, `boss-material`, `elemental-gem`, `talent-book`,
-- `weekly-material`): one the game data knows (`material_id`, the lowest
-- tier of its series) or one it does not (`name`).
CREATE TABLE plan_custom_materials (
    entry_id    INTEGER NOT NULL REFERENCES plan_entries (id) ON DELETE CASCADE,
    role        TEXT NOT NULL,
    material_id INTEGER,
    name        TEXT,
    PRIMARY KEY (entry_id, role),
    CHECK ((material_id IS NULL) <> (name IS NULL)),
    CHECK (name IS NULL OR trim(name) <> '')
) STRICT;

-- How many of each material the player has, as they entered it: a material
-- the game data knows (`material_id`, the game's id) or one a plan entry
-- names (`name`). A material they have none of has no row.
CREATE TABLE inventory (
    material_id INTEGER UNIQUE,
    name        TEXT UNIQUE,
    count       INTEGER NOT NULL CHECK (count > 0),
    updated_at  TEXT NOT NULL,
    CHECK ((material_id IS NULL) <> (name IS NULL)),
    CHECK (name IS NULL OR trim(name) <> '')
) STRICT;
