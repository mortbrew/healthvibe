-- iPhone Shortcut sync (docs/shortcut-design.md).

-- What the user chose to sync. Chosen in the web app before any QR code.
ALTER TABLE users ADD COLUMN sync_configured  INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN sync_trigger     TEXT    NOT NULL DEFAULT 'alarm_stopped';
ALTER TABLE users ADD COLUMN sync_notify      TEXT    NOT NULL DEFAULT 'errors_only';

CREATE TABLE sync_metrics (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    metric  TEXT    NOT NULL,          -- API key, e.g. stepCount
    PRIMARY KEY (user_id, metric)
);

-- One upload token per user (one device). Token = hvu_<prefix>_<secret>;
-- only the prefix and sha256(secret) are stored.
CREATE TABLE upload_tokens (
    user_id          INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    prefix           TEXT    NOT NULL UNIQUE,
    secret_hash      TEXT    NOT NULL,
    device           TEXT,
    shortcut_version TEXT,
    created_at       TEXT    NOT NULL,
    last_seen_at     TEXT,
    last_upload_at   TEXT
);

-- One-time setup codes shown in QR code 2 (sha256 stored, ~10 min TTL).
CREATE TABLE setup_codes (
    code_hash  TEXT    PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TEXT    NOT NULL
);

-- Per metric: end of the newest sample received from the Shortcut.
CREATE TABLE sync_cursors (
    user_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    metric       TEXT    NOT NULL,
    last_end_utc TEXT    NOT NULL,
    PRIMARY KEY (user_id, metric)
);

-- Raw upload bodies: written before parsing, deleted once parsed.
-- Failed ones are kept until expires_at (24 h) for debugging.
CREATE TABLE ingest_raw (
    id          INTEGER PRIMARY KEY,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    received_at TEXT    NOT NULL,
    body        BLOB    NOT NULL,
    status      TEXT    NOT NULL,      -- pending | failed
    error       TEXT,
    expires_at  TEXT
);
