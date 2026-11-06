-- Phone handoff: a one-time link (QR code) that opens a copy page on the
-- iPhone. Only the sha256 of the code is stored; the upload token is minted
-- at the moment the link is opened and is never stored in plain text.
CREATE TABLE handoffs (
    id          TEXT PRIMARY KEY,           -- public id the desktop polls with
    code_hash   TEXT NOT NULL UNIQUE,       -- sha256(code)
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL,
    expires_at  TEXT NOT NULL,
    claimed_at  TEXT
);
CREATE INDEX handoffs_user ON handoffs(user_id);
