-- Per-user IANA time zone (e.g. America/New_York). Used by seed-demo for
-- "today"; imported Health local dates are never rewritten.
ALTER TABLE profiles ADD COLUMN timezone TEXT;
