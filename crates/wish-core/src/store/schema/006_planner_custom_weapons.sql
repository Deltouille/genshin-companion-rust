-- Custom weapons in the plan: like the custom characters of 004 (a name,
-- stars, a material per role in `plan_custom_materials`, the level range in
-- the level columns), with `custom_weapon` set.
ALTER TABLE plan_entries ADD COLUMN custom_weapon INTEGER NOT NULL DEFAULT 0
    CHECK (custom_weapon IN (0, 1) AND (custom_weapon = 0 OR stars IS NOT NULL));
