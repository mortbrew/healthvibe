-- Per-user manual Activity ring goals. History is kept: every set/edit is a
-- row. Values are nullable so a user can set only one ring. effective_from is
-- the local calendar day the goals start applying; they roll forward until a
-- later manual entry or an export.xml ActivitySummary goal takes over.
-- Resolution (see stats::goals_for): a day's own export goal wins; otherwise
-- the most recent export or manual goal on or before that day, with export
-- beating manual on a same-date tie. Never a default.

CREATE TABLE manual_goals (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id        INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    effective_from TEXT    NOT NULL, -- YYYY-MM-DD (local day)
    move_kcal      REAL,            -- nullable: unset ring
    exercise_min   REAL,
    stand_hours    REAL,
    created_at     TEXT    NOT NULL
);

CREATE INDEX manual_goals_user_from ON manual_goals (user_id, effective_from);
