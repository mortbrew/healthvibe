-- Health Auto Export sync + first-login onboarding.
ALTER TABLE upload_tokens ADD COLUMN hae_received INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN onboarding_done INTEGER NOT NULL DEFAULT 0;
-- Users who already have data don't need the onboarding flow.
UPDATE users SET onboarding_done = 1 WHERE id IN (SELECT DISTINCT user_id FROM readings);
