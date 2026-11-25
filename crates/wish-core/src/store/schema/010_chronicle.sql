-- The chronicle: what HoYoLAB keeps of an account's play for a while only
-- (the Spiral Abyss, the Imaginarium Theater, the Traveler's Diary), kept
-- here as HoYoLAB sent it, one row per period. `period` is the game's number
-- for an Abyss or Theater period, or the Diary's month as YYYYMM. A period
-- fetched once it was over (`ended`) never changes; one fetched while under
-- way gives way to a later fetch. `data` is HoYoLAB's JSON, whole.
CREATE TABLE chronicle (
    account_id INTEGER NOT NULL REFERENCES accounts (id),
    kind       TEXT NOT NULL CHECK (kind IN ('abyss', 'theater', 'diary')),
    period     INTEGER NOT NULL CHECK (period > 0),
    fetched_at TEXT NOT NULL CHECK (trim(fetched_at) <> ''),
    ended      INTEGER NOT NULL CHECK (ended IN (0, 1)),
    data       TEXT NOT NULL CHECK (trim(data) <> ''),
    PRIMARY KEY (account_id, kind, period)
) STRICT;
