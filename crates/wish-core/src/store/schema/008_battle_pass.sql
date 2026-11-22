-- Each account's Battle Pass, as the player entered it: the level reached and
-- the start of the period it was reached in (a later period starts again at
-- 0), whether it has the Gnostic Hymn, the reward plan picked in the game, and
-- whether the planner counts the rewards still to come.
CREATE TABLE battle_pass (
    account_id   INTEGER PRIMARY KEY REFERENCES accounts (id),
    level        INTEGER NOT NULL CHECK (level BETWEEN 0 AND 50),
    period_start TEXT NOT NULL CHECK (trim(period_start) <> ''),
    premium      INTEGER NOT NULL CHECK (premium IN (0, 1)),
    reward_plan  TEXT NOT NULL
        CHECK (reward_plan IN ('no-artifact-exp', 'no-weapon-exp', 'no-character-exp', 'no-mora')),
    counted      INTEGER NOT NULL CHECK (counted IN (0, 1)),
    updated_at   TEXT NOT NULL
) STRICT;
