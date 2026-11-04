-- Awards (opt-in), Apple-only manual checks, and the optional user profile.
ALTER TABLE users ADD COLUMN awards_enabled INTEGER NOT NULL DEFAULT 0;

CREATE TABLE user_awards (
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key         TEXT    NOT NULL,
    first_date  TEXT    NOT NULL,
    latest_date TEXT    NOT NULL,
    count       INTEGER NOT NULL,
    detail      TEXT,
    PRIMARY KEY (user_id, key)
);

-- Fingerprint of the data the stored awards were computed from.
CREATE TABLE awards_state (
    user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    stamp   TEXT NOT NULL
);

CREATE TABLE apple_awards (
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key         TEXT    NOT NULL,
    earned_date TEXT,
    PRIMARY KEY (user_id, key)
);

CREATE TABLE profiles (
    user_id      INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    display_name TEXT,
    birth_date   TEXT,
    sex          TEXT,
    height_cm    REAL,
    wheelchair   INTEGER,
    avatar       TEXT,
    -- Goal overrides (NULL = use Apple's goals from the export).
    move_goal     REAL,
    exercise_goal REAL,
    stand_goal    REAL,
    -- JSON {distance:'mi'|'km', weight:'lb'|'kg', energy:'cal'|'kj', temp:'f'|'c'}
    units        TEXT
);
