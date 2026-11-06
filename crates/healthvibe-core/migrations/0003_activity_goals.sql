-- Daily Activity ring goals from <ActivitySummary> in export.xml.
-- One row per user and day; a newer import overwrites older values.
CREATE TABLE activity_goals (
    user_id       INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    date          TEXT    NOT NULL, -- YYYY-MM-DD (local day)
    move_kcal     REAL,
    exercise_min  REAL,
    stand_hours   REAL,
    PRIMARY KEY (user_id, date)
);
