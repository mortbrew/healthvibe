-- healthvibe initial schema.
-- Every row of health data carries user_id from day one (multi-user later).

CREATE TABLE users (
    id            INTEGER PRIMARY KEY,
    username      TEXT    NOT NULL UNIQUE,
    -- NULL until the first-run setup sets a password (argon2id PHC string).
    password_hash TEXT,
    is_admin      INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

-- Single seeded admin; password is set on first run in the web UI.
INSERT INTO users (id, username, is_admin) VALUES (1, 'admin', 1);

CREATE TABLE sessions (
    -- sha256 of the cookie value; the raw token is never stored.
    token_hash TEXT    PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TEXT    NOT NULL
);

CREATE TABLE readings (
    id        INTEGER PRIMARY KEY,
    user_id   INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    type      TEXT    NOT NULL,          -- e.g. HKQuantityTypeIdentifierStepCount
    value     TEXT,                      -- numeric or category value as exported
    unit      TEXT,
    start_utc TEXT    NOT NULL,          -- RFC3339 UTC
    end_utc   TEXT    NOT NULL,
    tz_offset TEXT,                      -- original offset, e.g. -0500
    source    TEXT,
    hash      TEXT    NOT NULL,          -- content hash for dedupe
    UNIQUE (user_id, hash)
);
CREATE INDEX readings_user_type_start ON readings (user_id, type, start_utc);

CREATE TABLE import_jobs (
    id          INTEGER PRIMARY KEY,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status      TEXT    NOT NULL,        -- running | done | failed
    counts_json TEXT,                    -- {"type": {"inserted": n, "duplicates": m}}
    error       TEXT,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    finished_at TEXT
);
