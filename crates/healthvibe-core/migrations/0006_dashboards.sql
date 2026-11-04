-- Custom dashboards. Widgets are a small JSON array (layout + settings);
-- the data itself is always queried live from readings.
CREATE TABLE dashboards (
    id           INTEGER PRIMARY KEY,
    user_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name         TEXT    NOT NULL,
    position     INTEGER NOT NULL DEFAULT 0,
    widgets_json TEXT    NOT NULL DEFAULT '[]',
    created_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);
CREATE INDEX dashboards_user ON dashboards (user_id, position);
