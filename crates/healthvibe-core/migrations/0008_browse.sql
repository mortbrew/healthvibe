-- Pinned metrics on the Summary page (per user, ordered).
CREATE TABLE pinned_metrics (
    user_id  INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    metric   TEXT    NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, metric)
);

-- Apple Health-style palettes become the default (light + dark).
UPDATE users SET theme_light = 'health' WHERE theme_light = 'daylight';
UPDATE users SET theme_dark  = 'health' WHERE theme_dark  = 'midnight';
CREATE TRIGGER users_default_theme AFTER INSERT ON users
WHEN NEW.theme_light = 'daylight' AND NEW.theme_dark = 'midnight'
BEGIN
    UPDATE users SET theme_light = 'health', theme_dark = 'health' WHERE id = NEW.id;
END;
