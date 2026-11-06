-- healthvibe stores what the source provides; it does not compute HR zones.
-- Drop the derived zone columns (they were only ever filled from HAE samples
-- binned by % of the workout's own max HR, and by the demo seeder).
ALTER TABLE workouts DROP COLUMN zone1_min;
ALTER TABLE workouts DROP COLUMN zone2_min;
ALTER TABLE workouts DROP COLUMN zone3_min;
ALTER TABLE workouts DROP COLUMN zone4_min;
ALTER TABLE workouts DROP COLUMN zone5_min;
