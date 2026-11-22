-- The series the player picked for the Battle Pass's boxes, each a material
-- id: a Guide (a talent book series' middle tier), a Philosophies (its top
-- tier), a local specialty. Null: the planner gives the boxes to what the plan
-- lacks, in its order.
ALTER TABLE battle_pass ADD COLUMN guides_pick INTEGER CHECK (guides_pick > 0);
ALTER TABLE battle_pass ADD COLUMN philosophies_pick INTEGER CHECK (philosophies_pick > 0);
ALTER TABLE battle_pass ADD COLUMN specialties_pick INTEGER CHECK (specialties_pick > 0);
