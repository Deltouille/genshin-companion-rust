-- The collection: what each account owns (characters, weapons, materials)
-- and plans. The plan and the inventory of 003-006 were the whole app's; they
-- now belong to an account, like the wishes.

-- What was planned or owned before goes to the oldest account; with no
-- account at all, to a new one named "Main".
INSERT INTO accounts (label, uid, server, created_at)
SELECT 'Main', NULL, NULL, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
WHERE NOT EXISTS (SELECT 1 FROM accounts)
  AND (EXISTS (SELECT 1 FROM plan_entries) OR EXISTS (SELECT 1 FROM inventory));

-- A plan entry's account. `plan_entries` keeps its rows and ids (the custom
-- costs and materials point at them), so the column is added rather than
-- the table rebuilt: the app always sets it.
ALTER TABLE plan_entries ADD COLUMN account_id INTEGER REFERENCES accounts (id);
UPDATE plan_entries SET account_id = (SELECT min(id) FROM accounts);
CREATE INDEX plan_entries_by_account ON plan_entries (account_id, position);

-- The inventory, rebuilt with its unique keys per account.
CREATE TABLE inventory_by_account (
    account_id  INTEGER NOT NULL REFERENCES accounts (id),
    material_id INTEGER,
    name        TEXT,
    count       INTEGER NOT NULL CHECK (count > 0),
    updated_at  TEXT NOT NULL,
    UNIQUE (account_id, material_id),
    UNIQUE (account_id, name),
    CHECK ((material_id IS NULL) <> (name IS NULL)),
    CHECK (name IS NULL OR trim(name) <> '')
) STRICT;
INSERT INTO inventory_by_account (account_id, material_id, name, count, updated_at)
SELECT (SELECT min(id) FROM accounts), material_id, name, count, updated_at FROM inventory;
DROP TABLE inventory;
ALTER TABLE inventory_by_account RENAME TO inventory;

-- The characters an account owns (genshin-db keys, as in the planner), as
-- the player entered them: level and ascension phase, constellation, the
-- three talents' base levels (without the +3 of constellations).
CREATE TABLE owned_characters (
    account_id    INTEGER NOT NULL REFERENCES accounts (id),
    character_key TEXT NOT NULL CHECK (trim(character_key) <> ''),
    level         INTEGER NOT NULL DEFAULT 1 CHECK (level BETWEEN 1 AND 100),
    phase         INTEGER NOT NULL DEFAULT 0 CHECK (phase BETWEEN 0 AND 6),
    constellation INTEGER NOT NULL DEFAULT 0 CHECK (constellation BETWEEN 0 AND 6),
    talent1       INTEGER NOT NULL DEFAULT 1 CHECK (talent1 BETWEEN 1 AND 10),
    talent2       INTEGER NOT NULL DEFAULT 1 CHECK (talent2 BETWEEN 1 AND 10),
    talent3       INTEGER NOT NULL DEFAULT 1 CHECK (talent3 BETWEEN 1 AND 10),
    favourite     INTEGER NOT NULL DEFAULT 0 CHECK (favourite IN (0, 1)),
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    PRIMARY KEY (account_id, character_key)
) STRICT;

-- The weapons an account owns, one row per copy: level and ascension phase,
-- refinement, and the character holding it (a key; one weapon each).
CREATE TABLE owned_weapons (
    id          INTEGER PRIMARY KEY,
    account_id  INTEGER NOT NULL REFERENCES accounts (id),
    weapon_key  TEXT NOT NULL CHECK (trim(weapon_key) <> ''),
    level       INTEGER NOT NULL DEFAULT 1 CHECK (level BETWEEN 1 AND 100),
    phase       INTEGER NOT NULL DEFAULT 0 CHECK (phase BETWEEN 0 AND 6),
    refinement  INTEGER NOT NULL DEFAULT 1 CHECK (refinement BETWEEN 1 AND 5),
    holder_key  TEXT CHECK (holder_key IS NULL OR trim(holder_key) <> ''),
    favourite   INTEGER NOT NULL DEFAULT 0 CHECK (favourite IN (0, 1)),
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
) STRICT;
CREATE INDEX owned_weapons_by_account ON owned_weapons (account_id, weapon_key);
CREATE UNIQUE INDEX owned_weapons_one_per_holder
    ON owned_weapons (account_id, holder_key) WHERE holder_key IS NOT NULL;
