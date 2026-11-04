-- Ring goals come only from Apple ActivitySummary data (activity_goals).
-- Remove the old per-profile goal overrides.
ALTER TABLE profiles DROP COLUMN move_goal;
ALTER TABLE profiles DROP COLUMN exercise_goal;
ALTER TABLE profiles DROP COLUMN stand_goal;
