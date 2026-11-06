-- Accounts: forced password change and per-user appearance.
ALTER TABLE users ADD COLUMN must_change_password INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN theme_mode  TEXT NOT NULL DEFAULT 'system'; -- light | dark | system
ALTER TABLE users ADD COLUMN theme_light TEXT NOT NULL DEFAULT 'daylight';
ALTER TABLE users ADD COLUMN theme_dark  TEXT NOT NULL DEFAULT 'midnight';

-- Wall-clock time in the zone the sample was recorded in ("YYYY-MM-DD HH:MM:SS"),
-- so days and nights group the way the user lived them.
ALTER TABLE readings ADD COLUMN local_start TEXT;
ALTER TABLE readings ADD COLUMN local_end   TEXT;
CREATE INDEX readings_user_type_local ON readings (user_id, type, local_start);
