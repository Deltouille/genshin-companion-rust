-- The game's ids for wishes stored without one (migrated from paimon.moe),
-- learned when the game's history is imported. Kept apart from `wishes` so
-- the original records are never rewritten, and so an import can be undone.
CREATE TABLE wish_links (
    account_id INTEGER NOT NULL,
    wish_id    TEXT NOT NULL,
    api_id     TEXT NOT NULL CHECK (api_id NOT GLOB '*[^0-9]*' AND api_id <> ''),
    import_id  INTEGER NOT NULL REFERENCES imports (id),
    PRIMARY KEY (account_id, wish_id),
    UNIQUE (account_id, api_id),
    FOREIGN KEY (account_id, wish_id) REFERENCES wishes (account_id, id)
) STRICT;

-- Which import brought a manual counter, so undoing it removes the counter.
ALTER TABLE manual_counters ADD COLUMN import_id INTEGER REFERENCES imports (id);

-- When an import was undone. Undone imports stay in the log.
ALTER TABLE imports ADD COLUMN undone_at TEXT;
