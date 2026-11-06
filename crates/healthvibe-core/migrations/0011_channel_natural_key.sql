-- Ingest channel on every reading, plus a natural unique key so re-sends
-- update instead of duplicating. Device/app name stays in `source`.
--
-- Channel preference for aggregations (see channel::rank / RANK_SQL):
--   export_xml > health_auto_export > shortcut > demo > unknown
-- For a given user/metric/local day we keep only the best channel, then
-- (for additive metrics) the device source with the highest total. That way
-- an export.xml day never double-counts with a later HAE sync of the same day,
-- and days after the last import (HAE-only) use HAE.

ALTER TABLE readings ADD COLUMN channel TEXT NOT NULL DEFAULT 'unknown';

UPDATE readings SET source = '' WHERE source IS NULL;

-- Keep the newest row when the natural key collides.
DELETE FROM readings
 WHERE id NOT IN (
   SELECT MAX(id) FROM readings
   GROUP BY user_id, type, channel, source, start_utc, end_utc
 );

CREATE TABLE readings_new (
    id          INTEGER PRIMARY KEY,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    type        TEXT    NOT NULL,
    value       TEXT,
    unit        TEXT,
    start_utc   TEXT    NOT NULL,
    end_utc     TEXT    NOT NULL,
    tz_offset   TEXT,
    source      TEXT    NOT NULL DEFAULT '',
    channel     TEXT    NOT NULL DEFAULT 'unknown',
    hash        TEXT    NOT NULL,
    local_start TEXT,
    local_end   TEXT,
    UNIQUE (user_id, type, channel, source, start_utc, end_utc)
);

INSERT INTO readings_new
    (id, user_id, type, value, unit, start_utc, end_utc, tz_offset,
     source, channel, hash, local_start, local_end)
SELECT id, user_id, type, value, unit, start_utc, end_utc, tz_offset,
       COALESCE(source, ''), channel, hash, local_start, local_end
  FROM readings;

DROP TABLE readings;
ALTER TABLE readings_new RENAME TO readings;

-- Recreate indexes after rename so names do not collide with the dropped table's.
CREATE INDEX readings_user_type_start ON readings (user_id, type, start_utc);
CREATE INDEX readings_user_type_local ON readings (user_id, type, local_start);
CREATE INDEX readings_user_hash ON readings (user_id, hash);
CREATE INDEX readings_user_type_channel_local ON readings (user_id, type, channel, local_start);
