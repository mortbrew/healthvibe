-- healthvibe no longer computes awards or streaks from data. Drop the cached
-- computed awards and the opt-in flag. Apple awards the user marks as earned
-- (apple_awards) are kept.
DROP TABLE IF EXISTS user_awards;
DROP TABLE IF EXISTS awards_state;
ALTER TABLE users DROP COLUMN awards_enabled;
