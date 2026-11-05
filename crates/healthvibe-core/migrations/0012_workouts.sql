-- Rich workout details alongside duration readings (awards/browse still use
-- readings). HR zones are minutes in each band when heartRateData is present.
CREATE TABLE IF NOT EXISTS workouts (
    id            INTEGER PRIMARY KEY,
    user_id       INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    activity_type TEXT    NOT NULL,
    channel       TEXT    NOT NULL,
    source        TEXT    NOT NULL DEFAULT '',
    start_utc     TEXT    NOT NULL,
    end_utc       TEXT    NOT NULL,
    local_start   TEXT,
    local_end     TEXT,
    duration_min  REAL,
    avg_hr        REAL,
    max_hr        REAL,
    active_kcal   REAL,
    total_kcal    REAL,
    distance_m    REAL,
    elevation_m   REAL,
    zone1_min     REAL,
    zone2_min     REAL,
    zone3_min     REAL,
    zone4_min     REAL,
    zone5_min     REAL,
    detail_json   TEXT,
    external_id   TEXT,
    UNIQUE (user_id, channel, source, start_utc, end_utc, activity_type)
);
CREATE INDEX IF NOT EXISTS workouts_user_start ON workouts (user_id, start_utc DESC);
CREATE INDEX IF NOT EXISTS workouts_user_local ON workouts (user_id, local_start DESC);
