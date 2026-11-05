-- Failure-only login lockout (success clears). IP limiting stays in-process.
CREATE TABLE IF NOT EXISTS auth_failures (
    username     TEXT PRIMARY KEY,
    fail_count   INTEGER NOT NULL DEFAULT 0,
    locked_until TEXT,
    updated_at   TEXT NOT NULL
);
