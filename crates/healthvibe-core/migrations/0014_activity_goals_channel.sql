-- Tag Activity ring goals with the ingest channel, like readings.
-- Existing rows get 'unknown' (we can't tell which path wrote them); the next
-- export.xml import adds export_xml rows, which outrank them.
--
-- Readers use activity_goals_best: one row per user/day, picked by the same
-- channel preference as readings (see channel::rank / RANK_SQL):
--   export_xml > health_auto_export > shortcut > demo > unknown
-- The whole row comes from one channel; fields are never mixed across channels.

CREATE TABLE activity_goals_new (
    user_id       INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    date          TEXT    NOT NULL, -- YYYY-MM-DD (local day)
    channel       TEXT    NOT NULL DEFAULT 'unknown',
    move_kcal     REAL,
    exercise_min  REAL,
    stand_hours   REAL,
    PRIMARY KEY (user_id, date, channel)
);

INSERT INTO activity_goals_new (user_id, date, channel, move_kcal, exercise_min, stand_hours)
SELECT user_id, date, 'unknown', move_kcal, exercise_min, stand_hours FROM activity_goals;

DROP TABLE activity_goals;
ALTER TABLE activity_goals_new RENAME TO activity_goals;

CREATE VIEW activity_goals_best AS
SELECT user_id, date, channel, move_kcal, exercise_min, stand_hours
FROM (
    SELECT g.*, ROW_NUMBER() OVER (
        PARTITION BY user_id, date
        ORDER BY CASE channel
            WHEN 'export_xml' THEN 1
            WHEN 'health_auto_export' THEN 2
            WHEN 'shortcut' THEN 3
            WHEN 'demo' THEN 4
            ELSE 5 END
    ) AS rn
    FROM activity_goals g
)
WHERE rn = 1;
