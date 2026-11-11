-- Raw records only: pity, banners and 50/50 are recomputed, never stored.

CREATE TABLE accounts (
    id         INTEGER PRIMARY KEY,
    label      TEXT NOT NULL,
    uid        TEXT UNIQUE,
    server     TEXT,
    created_at TEXT NOT NULL
) STRICT;

-- One row per account per applied import, kept as an audit log.
CREATE TABLE imports (
    id          INTEGER PRIMARY KEY,
    account_id  INTEGER NOT NULL REFERENCES accounts (id),
    source      TEXT NOT NULL,
    detail      TEXT,
    imported_at TEXT NOT NULL,
    added       INTEGER NOT NULL
) STRICT;

CREATE TABLE wishes (
    account_id  INTEGER NOT NULL REFERENCES accounts (id),
    id          TEXT NOT NULL,
    gacha_type  TEXT NOT NULL CHECK (gacha_type IN ('100', '200', '301', '302', '400', '500')),
    time        TEXT NOT NULL,
    seq         INTEGER NOT NULL CHECK (seq >= 0),
    name        TEXT NOT NULL,
    item_type   TEXT NOT NULL CHECK (item_type IN ('Character', 'Weapon', 'Unknown')),
    rarity      INTEGER NOT NULL CHECK (rarity BETWEEN 3 AND 5),
    item_key    TEXT,
    manual_pity INTEGER CHECK (manual_pity >= 0),
    source      TEXT NOT NULL CHECK (source IN ('api', 'paimon', 'manual')),
    import_id   INTEGER REFERENCES imports (id),
    PRIMARY KEY (account_id, id)
) STRICT;

CREATE INDEX wishes_timeline ON wishes (account_id, gacha_type, time, seq);

-- Counters typed by hand in paimon.moe's manual mode.
CREATE TABLE manual_counters (
    account_id INTEGER NOT NULL REFERENCES accounts (id),
    pool       TEXT NOT NULL,
    total      INTEGER NOT NULL,
    pity5      INTEGER NOT NULL,
    pity4      INTEGER NOT NULL,
    PRIMARY KEY (account_id, pool)
) STRICT;
