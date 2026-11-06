-- Built-in Summary vs custom dashboards, and the sidebar order.
-- Widgets stay in dashboards.widgets_json. Health readings are not touched.
ALTER TABLE dashboards ADD COLUMN builtin TEXT;
ALTER TABLE dashboards ADD COLUMN pins_applied INTEGER NOT NULL DEFAULT 0;
CREATE TABLE nav_layout (
    user_id    INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    items_json TEXT    NOT NULL
);
