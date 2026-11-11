-- The ascension planner: what the player wants to raise, as they typed it in.
-- What it costs is never stored: it is worked out from the game data.

-- One line of the plan: a character of the game data (`character_key`, a
-- genshin-db key) with where it is and where it should go, or a custom entry
-- (`name`) for something the data does not have yet, such as a leak.
CREATE TABLE plan_entries (
    id            INTEGER PRIMARY KEY,
    position      INTEGER NOT NULL,
    character_key TEXT,
    name          TEXT,
    level_from    INTEGER NOT NULL DEFAULT 1  CHECK (level_from BETWEEN 1 AND 90),
    phase_from    INTEGER NOT NULL DEFAULT 0  CHECK (phase_from BETWEEN 0 AND 6),
    level_to      INTEGER NOT NULL DEFAULT 90 CHECK (level_to BETWEEN 1 AND 90),
    phase_to      INTEGER NOT NULL DEFAULT 6  CHECK (phase_to BETWEEN 0 AND 6),
    talent1_from  INTEGER NOT NULL DEFAULT 1  CHECK (talent1_from BETWEEN 1 AND 10),
    talent1_to    INTEGER NOT NULL DEFAULT 1  CHECK (talent1_to BETWEEN 1 AND 10),
    talent2_from  INTEGER NOT NULL DEFAULT 1  CHECK (talent2_from BETWEEN 1 AND 10),
    talent2_to    INTEGER NOT NULL DEFAULT 1  CHECK (talent2_to BETWEEN 1 AND 10),
    talent3_from  INTEGER NOT NULL DEFAULT 1  CHECK (talent3_from BETWEEN 1 AND 10),
    talent3_to    INTEGER NOT NULL DEFAULT 1  CHECK (talent3_to BETWEEN 1 AND 10),
    created_at    TEXT NOT NULL,
    CHECK ((character_key IS NULL) <> (name IS NULL)),
    CHECK (name IS NULL OR trim(name) <> '')
) STRICT;

-- What a custom entry needs: a material the game data knows (`material_id`,
-- the game's id) or one it does not (`name`), and how many.
CREATE TABLE plan_custom_costs (
    entry_id    INTEGER NOT NULL REFERENCES plan_entries (id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    material_id INTEGER,
    name        TEXT,
    count       INTEGER NOT NULL CHECK (count > 0),
    PRIMARY KEY (entry_id, position),
    CHECK ((material_id IS NULL) <> (name IS NULL)),
    CHECK (name IS NULL OR trim(name) <> '')
) STRICT;
