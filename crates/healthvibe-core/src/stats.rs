//! Dashboard queries. Every query is scoped by `user_id`; there is no way to
//! ask for another user's rows through this module.
//!
//! Days are the user's local calendar days (`local_start`). For additive
//! metrics (steps, energy, exercise) iPhone and Watch both record the same
//! movement, so per day we take the source with the highest total instead of
//! adding sources together (a simple stand-in for Apple's source priority).

use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::BTreeMap;

pub const STEPS: &str = "HKQuantityTypeIdentifierStepCount";
pub const ACTIVE_ENERGY: &str = "HKQuantityTypeIdentifierActiveEnergyBurned";
pub const EXERCISE: &str = "HKQuantityTypeIdentifierAppleExerciseTime";
pub const STAND_HOUR: &str = "HKCategoryTypeIdentifierAppleStandHour";
pub const HEART_RATE: &str = "HKQuantityTypeIdentifierHeartRate";
pub const RESTING_HR: &str = "HKQuantityTypeIdentifierRestingHeartRate";
pub const HRV: &str = "HKQuantityTypeIdentifierHeartRateVariabilitySDNN";
pub const SLEEP: &str = "HKCategoryTypeIdentifierSleepAnalysis";
pub const WEIGHT: &str = "HKQuantityTypeIdentifierBodyMass";
pub const BODY_FAT: &str = "HKQuantityTypeIdentifierBodyFatPercentage";
pub const LEAN_MASS: &str = "HKQuantityTypeIdentifierLeanBodyMass";

pub const DEFAULT_MOVE_GOAL: f64 = 500.0;
pub const DEFAULT_EXERCISE_GOAL: f64 = 30.0;
pub const DEFAULT_STAND_GOAL: f64 = 12.0;

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct DayValue {
    pub date: String,
    pub value: f64,
}

fn shift(date: &str, days: i64) -> String {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|d| (d + chrono::Duration::days(days)).to_string())
        .unwrap_or_else(|_| date.to_string())
}

/// Most recent local day with any data for this user.
pub async fn latest_day(db: &SqlitePool, user_id: i64) -> anyhow::Result<Option<String>> {
    Ok(sqlx::query_scalar::<_, Option<String>>(
        "SELECT substr(MAX(local_start), 1, 10) FROM readings WHERE user_id = ? AND type != ?",
    )
    .bind(user_id)
    .bind(SLEEP)
    .fetch_one(db)
    .await?)
}

/// Daily totals for an additive metric over `[end - days + 1, end]`.
pub async fn daily_sum(
    db: &SqlitePool,
    user_id: i64,
    kind: &str,
    end: &str,
    days: i64,
) -> anyhow::Result<Vec<DayValue>> {
    // Prefer best ingest channel for the day, then the device source with
    // the highest total within that channel (see channel::rank).
    let rows: Vec<(String, f64)> = sqlx::query_as(
        &format!(
            "SELECT d, total FROM (
                SELECT d, total,
                       ROW_NUMBER() OVER (
                         PARTITION BY d
                         ORDER BY {rank} ASC, total DESC
                       ) AS rn
                FROM (
                    SELECT substr(local_start, 1, 10) AS d, channel, source,
                           SUM(CAST(value AS REAL)) AS total
                    FROM readings
                    WHERE user_id = ? AND type = ? AND local_start >= ? AND local_start < ?
                    GROUP BY d, channel, source
                )
             ) WHERE rn = 1 ORDER BY d",
            rank = crate::channel::RANK_SQL,
        ),
    )
    .bind(user_id)
    .bind(kind)
    .bind(shift(end, 1 - days))
    .bind(shift(end, 1))
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(date, value)| DayValue { date, value })
        .collect())
}

/// Daily average of a sampled metric (resting HR, HRV, ...).
pub async fn daily_avg(
    db: &SqlitePool,
    user_id: i64,
    kind: &str,
    end: &str,
    days: i64,
) -> anyhow::Result<Vec<DayValue>> {
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT substr(local_start, 1, 10) AS d, AVG(CAST(value AS REAL))
         FROM readings
         WHERE user_id = ? AND type = ? AND local_start >= ? AND local_start < ?
         GROUP BY d ORDER BY d",
    )
    .bind(user_id)
    .bind(kind)
    .bind(shift(end, 1 - days))
    .bind(shift(end, 1))
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(date, value)| DayValue { date, value })
        .collect())
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Rings {
    pub date: String,
    pub move_kcal: f64,
    pub move_goal: f64,
    pub exercise_min: f64,
    pub exercise_goal: f64,
    pub stand_hours: f64,
    pub stand_goal: f64,
    pub steps: f64,
}

#[derive(Debug, Serialize)]
pub struct Activity {
    pub date: Option<String>,
    pub rings: Option<Rings>,
    pub steps: Vec<DayValue>,
    pub move_kcal: Vec<DayValue>,
}

fn on(v: &[DayValue], date: &str) -> f64 {
    v.iter().find(|d| d.date == date).map_or(0.0, |d| d.value)
}

pub async fn activity(
    db: &SqlitePool,
    user_id: i64,
    date: Option<String>,
) -> anyhow::Result<Activity> {
    let Some(date) = (match date {
        Some(d) => Some(d),
        None => latest_day(db, user_id).await?,
    }) else {
        return Ok(Activity {
            date: None,
            rings: None,
            steps: vec![],
            move_kcal: vec![],
        });
    };
    let steps = daily_sum(db, user_id, STEPS, &date, 30).await?;
    let energy = daily_sum(db, user_id, ACTIVE_ENERGY, &date, 30).await?;
    let exercise = daily_sum(db, user_id, EXERCISE, &date, 1).await?;
    let stand: i64 = sqlx::query_scalar(
        &format!(
            "SELECT COUNT(DISTINCT substr(local_start, 1, 13)) FROM readings
             WHERE user_id = ? AND type = ? AND value = 'HKCategoryValueAppleStandHourStood'
               AND local_start >= ? AND local_start < ?
               AND channel = (
                 SELECT channel FROM readings
                  WHERE user_id = ? AND type = ?
                    AND value = 'HKCategoryValueAppleStandHourStood'
                    AND local_start >= ? AND local_start < ?
                  ORDER BY {rank} ASC LIMIT 1
               )",
            rank = crate::channel::RANK_SQL,
        ),
    )
    .bind(user_id)
    .bind(STAND_HOUR)
    .bind(&date)
    .bind(shift(&date, 1))
    .bind(user_id)
    .bind(STAND_HOUR)
    .bind(&date)
    .bind(shift(&date, 1))
    .fetch_one(db)
    .await?;
    // Goals: this day's <ActivitySummary>, else the most recent earlier one,
    // else Apple's defaults.
    let g: Option<(Option<f64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT move_kcal, exercise_min, stand_hours FROM activity_goals
         WHERE user_id = ? AND date <= ? ORDER BY date DESC LIMIT 1",
    )
    .bind(user_id)
    .bind(&date)
    .fetch_optional(db)
    .await?;
    let (gm, ge, gs) = g.unwrap_or((None, None, None));
    let rings = Rings {
        date: date.clone(),
        move_kcal: on(&energy, &date),
        move_goal: gm.unwrap_or(DEFAULT_MOVE_GOAL),
        exercise_min: on(&exercise, &date),
        exercise_goal: ge.unwrap_or(DEFAULT_EXERCISE_GOAL),
        stand_hours: stand as f64,
        stand_goal: gs.unwrap_or(DEFAULT_STAND_GOAL),
        steps: on(&steps, &date),
    };
    Ok(Activity {
        date: Some(date),
        rings: Some(rings),
        steps,
        move_kcal: energy,
    })
}

#[derive(Debug, Serialize)]
pub struct HeartDay {
    pub date: String,
    pub min: f64,
    pub avg: f64,
    pub max: f64,
}

#[derive(Debug, Serialize)]
pub struct Heart {
    pub daily: Vec<HeartDay>,
    pub resting: Vec<DayValue>,
    pub hrv: Vec<DayValue>,
}

pub async fn heart(db: &SqlitePool, user_id: i64, end: &str, days: i64) -> anyhow::Result<Heart> {
    let rows: Vec<(String, f64, f64, f64)> = sqlx::query_as(
        "SELECT substr(local_start, 1, 10) AS d, MIN(CAST(value AS REAL)),
                AVG(CAST(value AS REAL)), MAX(CAST(value AS REAL))
         FROM readings
         WHERE user_id = ? AND type = ? AND local_start >= ? AND local_start < ?
         GROUP BY d ORDER BY d",
    )
    .bind(user_id)
    .bind(HEART_RATE)
    .bind(shift(end, 1 - days))
    .bind(shift(end, 1))
    .fetch_all(db)
    .await?;
    Ok(Heart {
        daily: rows
            .into_iter()
            .map(|(date, min, avg, max)| HeartDay {
                date,
                min,
                avg,
                max,
            })
            .collect(),
        resting: daily_avg(db, user_id, RESTING_HR, end, days).await?,
        hrv: daily_avg(db, user_id, HRV, end, days).await?,
    })
}

#[derive(Debug, Serialize, Default, Clone)]
pub struct Night {
    /// The local date the user woke up (end of the session).
    pub date: String,
    pub awake_min: f64,
    pub rem_min: f64,
    pub core_min: f64,
    pub deep_min: f64,
    /// Asleep without stage detail (older devices).
    pub unspecified_min: f64,
    pub in_bed_min: f64,
    /// Time asleep (Core + Deep + REM + unspecified), with overlapping
    /// samples from all sources merged so nothing is counted twice.
    /// InBed and Awake are excluded.
    pub asleep_min: f64,
    pub source: String,
    /// True when this session is a nap, not the main overnight sleep.
    ///
    /// Nap rule (documented): after clustering contiguous asleep intervals
    /// that are separated by gaps under 90 minutes into sessions (wake date =
    /// local end date), a session is a nap when it is under 3 hours **or**
    /// it starts between 10:00 and 18:00 local time and is not the longest
    /// session that ends on that wake date.
    #[serde(default)]
    pub is_nap: bool,
    /// Local start of the session (`YYYY-MM-DD HH:MM:SS`), when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// Local end of the session, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
}

fn is_asleep(stage: &str) -> bool {
    stage.starts_with("Asleep")
}

/// Total minutes covered by the union of `[start, end)` intervals (in days).
fn union_minutes(mut iv: Vec<(f64, f64)>) -> f64 {
    iv.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut total = 0.0;
    let mut cur: Option<(f64, f64)> = None;
    for (s, e) in iv {
        match cur {
            Some((cs, ce)) if s <= ce => cur = Some((cs, ce.max(e))),
            Some((cs, ce)) => {
                total += ce - cs;
                cur = Some((s, e));
            }
            None => cur = Some((s, e)),
        }
    }
    if let Some((cs, ce)) = cur {
        total += ce - cs;
    }
    total * 1440.0
}

#[derive(Debug, Serialize)]
pub struct Segment {
    pub stage: String,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Serialize)]
pub struct Sleep {
    pub nights: Vec<Night>,
    /// Stage timeline of the most recent night.
    pub last_night: Vec<Segment>,
}

fn stage(value: &str) -> &str {
    value
        .strip_prefix("HKCategoryValueSleepAnalysis")
        .unwrap_or(value)
}

pub async fn sleep(db: &SqlitePool, user_id: i64, end: &str, days: i64) -> anyhow::Result<Sleep> {
    // Night = local date of the sample's end; segments are grouped per source
    // and the source with the most recorded sleep wins (Watch over iPhone).
    let rows: Vec<(String, String, String, String, f64)> = sqlx::query_as(
        "SELECT substr(local_end, 1, 10) AS night, channel, COALESCE(source, ''), value,
                SUM((julianday(end_utc) - julianday(start_utc)) * 1440)
         FROM readings
         WHERE user_id = ? AND type = ? AND local_end >= ? AND local_end < ?
         GROUP BY night, channel, source, value",
    )
    .bind(user_id)
    .bind(SLEEP)
    .bind(shift(end, 1 - days))
    .bind(shift(end, 2))
    .fetch_all(db)
    .await?;
    let mut by: BTreeMap<(String, String, String), Night> = BTreeMap::new();
    for (night, channel, source, value, min) in rows {
        let n = by
            .entry((night.clone(), channel, source.clone()))
            .or_insert(Night {
                date: night,
                source,
                ..Default::default()
            });
        match stage(&value) {
            "Awake" => n.awake_min += min,
            "AsleepREM" => n.rem_min += min,
            "AsleepCore" => n.core_min += min,
            "AsleepDeep" => n.deep_min += min,
            "InBed" => n.in_bed_min += min,
            _ => n.unspecified_min += min,
        }
    }
    let asleep = |n: &Night| n.rem_min + n.core_min + n.deep_min + n.unspecified_min;
    let mut best: BTreeMap<String, Night> = BTreeMap::new();
    let mut best_key: BTreeMap<String, (i64, f64, String)> = BTreeMap::new();
    for ((night, channel, _), n) in by {
        let rank = crate::channel::rank(&channel);
        let a = asleep(&n);
        match best_key.get(&night) {
            Some(&(br, ba, _)) if br < rank || (br == rank && ba >= a) => {}
            _ => {
                best_key.insert(night.clone(), (rank, a, channel));
                best.insert(night, n);
            }
        }
    }
    // Asleep intervals (preferred channel only) with local bounds for clustering.
    // night, channel, source, value, local_start, local_end, start_jd, end_jd
    type SleepIv = (String, String, String, String, String, String, f64, f64);
    let ivs: Vec<SleepIv> = sqlx::query_as(
        "SELECT substr(local_end, 1, 10), channel, COALESCE(source, ''), value,
                local_start, local_end, julianday(start_utc), julianday(end_utc)
         FROM readings
         WHERE user_id = ? AND type = ? AND local_end >= ? AND local_end < ?",
    )
    .bind(user_id)
    .bind(SLEEP)
    .bind(shift(end, 1 - days))
    .bind(shift(end, 2))
    .fetch_all(db)
    .await?;
    let preferred_channel: BTreeMap<String, String> = best_key
        .iter()
        .map(|(d, (_, _, ch))| (d.clone(), ch.clone()))
        .collect();

    // Collect asleep intervals per wake-date for the preferred channel.
    let mut raw: BTreeMap<String, Vec<(String, String, f64, f64)>> = BTreeMap::new();
    let mut per: BTreeMap<String, Vec<(f64, f64)>> = BTreeMap::new();
    for (night, channel, _source, value, ls, le, s, e) in ivs {
        if preferred_channel.get(&night).map(|c| c == &channel) != Some(true) {
            continue;
        }
        if is_asleep(stage(&value)) && e > s {
            per.entry(night.clone()).or_default().push((s, e));
            raw.entry(night).or_default().push((ls, le, s, e));
        }
    }

    // Cluster into sessions (gap < 90 min), then flag naps.
    let mut sessions: Vec<(String, String, String, f64, bool)> = vec![]; // night, start, end, min, is_nap
    for (night, mut iv) in raw {
        iv.sort_by(|a, b| a.2.total_cmp(&b.2));
        // local_start, local_end, start_jd, end_jd, parts
        type Cluster = (String, String, f64, f64, Vec<(f64, f64)>);
        let mut clusters: Vec<Cluster> = vec![];
        for (ls, le, s, e) in iv {
            if let Some(c) = clusters.last_mut() {
                if (s - c.3) * 1440.0 < 90.0 {
                    c.3 = c.3.max(e);
                    if le > c.1 {
                        c.1 = le;
                    }
                    c.4.push((s, e));
                    continue;
                }
            }
            clusters.push((ls, le, s, e, vec![(s, e)]));
        }
        let mut scored: Vec<(String, String, f64)> = clusters
            .into_iter()
            .map(|(ls, le, _, _, parts)| {
                let mins = (union_minutes(parts) * 10.0).round() / 10.0;
                (ls, le, mins)
            })
            .collect();
        let longest = scored
            .iter()
            .map(|(_, _, m)| *m)
            .fold(0.0_f64, f64::max);
        for (ls, le, mins) in scored.drain(..) {
            let hour: u32 = ls.get(11..13).and_then(|h| h.parse().ok()).unwrap_or(0);
            let daytime = (10..18).contains(&hour);
            let short = mins < 180.0;
            let is_longest = (mins - longest).abs() <= 0.05;
            // Nap rule: under 3h, or daytime start and not the day's longest.
            // The longest non-daytime session is always the main overnight.
            let is_nap = match (is_longest, daytime, short) {
                (true, true, true) => true,   // only a short daytime sleep
                (true, _, _) => false,        // main overnight (or long day sleep)
                (false, _, true) => true,     // short secondary session
                (false, true, _) => true,     // daytime secondary
                (false, false, false) => false,
            };
            sessions.push((night.clone(), ls, le, mins, is_nap));
        }
    }

    // Primary night totals (non-nap) keep stage breakdown from `best`.
    let mut nights: Vec<Night> = best
        .into_values()
        .filter_map(|mut n| {
            let total = per
                .remove(&n.date)
                .map_or(0.0, |v| (union_minutes(v) * 10.0).round() / 10.0);
            let main = sessions
                .iter()
                .filter(|(d, _, _, _, nap)| d == &n.date && !*nap)
                .map(|(_, ls, le, m, _)| (ls.clone(), le.clone(), *m))
                .max_by(|a, b| a.2.total_cmp(&b.2));
            let Some((ls, le, mins)) = main else {
                // Only naps that day — still surface the night as naps below.
                return None;
            };
            n.asleep_min = mins;
            n.is_nap = false;
            n.start = Some(ls);
            n.end = Some(le);
            // Prefer total union when stages cover the main window.
            if total > 0.0 && (total - mins).abs() < 1.0 {
                n.asleep_min = total;
            }
            Some(n)
        })
        .collect();

    for (date, ls, le, mins, is_nap) in &sessions {
        if !*is_nap {
            continue;
        }
        let source = nights
            .iter()
            .find(|n| n.date == *date)
            .map(|n| n.source.clone())
            .unwrap_or_default();
        nights.push(Night {
            date: date.clone(),
            asleep_min: *mins,
            source,
            is_nap: true,
            start: Some(ls.clone()),
            end: Some(le.clone()),
            ..Default::default()
        });
    }
    nights.sort_by(|a, b| a.date.cmp(&b.date).then(a.is_nap.cmp(&b.is_nap)));

    let last_main = nights.iter().rev().find(|n| !n.is_nap);
    let last_night = match last_main {
        Some(n) => sqlx::query_as::<_, (String, String, String)>(
            "SELECT value, local_start, local_end FROM readings
             WHERE user_id = ? AND type = ? AND substr(local_end, 1, 10) = ?
               AND COALESCE(source, '') = ? AND value != 'HKCategoryValueSleepAnalysisInBed'
             ORDER BY local_start",
        )
        .bind(user_id)
        .bind(SLEEP)
        .bind(&n.date)
        .bind(&n.source)
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|(v, start, end)| Segment {
            stage: stage(&v).to_string(),
            start,
            end,
        })
        .collect(),
        None => vec![],
    };
    Ok(Sleep { nights, last_night })
}

#[derive(Debug, Serialize)]
pub struct Series {
    pub unit: Option<String>,
    pub points: Vec<DayValue>,
}

#[derive(Debug, Serialize)]
pub struct Body {
    pub weight: Series,
    pub body_fat: Series,
    pub lean_mass: Series,
}

async fn last_per_day(
    db: &SqlitePool,
    user_id: i64,
    kind: &str,
    end: &str,
    days: i64,
) -> anyhow::Result<Series> {
    let rows: Vec<(String, f64, Option<String>)> = sqlx::query_as(
        "SELECT substr(local_start, 1, 10) AS d, CAST(value AS REAL), unit FROM readings r
         WHERE user_id = ? AND type = ? AND local_start >= ? AND local_start < ?
           AND local_start = (SELECT MAX(local_start) FROM readings r2
                              WHERE r2.user_id = r.user_id AND r2.type = r.type
                                AND substr(r2.local_start, 1, 10) = substr(r.local_start, 1, 10))
         GROUP BY d ORDER BY d",
    )
    .bind(user_id)
    .bind(kind)
    .bind(shift(end, 1 - days))
    .bind(shift(end, 1))
    .fetch_all(db)
    .await?;
    let unit = rows.last().and_then(|r| r.2.clone());
    Ok(Series {
        unit,
        points: rows
            .into_iter()
            .map(|(date, value, _)| DayValue { date, value })
            .collect(),
    })
}

pub async fn body(db: &SqlitePool, user_id: i64, end: &str, days: i64) -> anyhow::Result<Body> {
    Ok(Body {
        weight: last_per_day(db, user_id, WEIGHT, end, days).await?,
        body_fat: last_per_day(db, user_id, BODY_FAT, end, days).await?,
        lean_mass: last_per_day(db, user_id, LEAN_MASS, end, days).await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seeded() -> SqlitePool {
        let db = crate::db::open_memory().await.unwrap();
        sqlx::query("INSERT INTO users (id, username, password_hash) VALUES (2, 'b', 'x')")
            .execute(&db)
            .await
            .unwrap();
        let rows = [
            // user 1: phone 1000 + watch 800 steps on the same day -> 1000
            (
                1,
                STEPS,
                "600",
                "2024-01-02 08:00:00",
                "2024-01-02 08:10:00",
                "Phone",
            ),
            (
                1,
                STEPS,
                "400",
                "2024-01-02 09:00:00",
                "2024-01-02 09:10:00",
                "Phone",
            ),
            (
                1,
                STEPS,
                "800",
                "2024-01-02 08:00:00",
                "2024-01-02 08:10:00",
                "Watch",
            ),
            (
                1,
                STAND_HOUR,
                "HKCategoryValueAppleStandHourStood",
                "2024-01-02 08:00:00",
                "2024-01-02 09:00:00",
                "Watch",
            ),
            (
                1,
                SLEEP,
                "HKCategoryValueSleepAnalysisAsleepDeep",
                "2024-01-01 23:00:00",
                "2024-01-02 00:00:00",
                "Watch",
            ),
            (
                1,
                SLEEP,
                "HKCategoryValueSleepAnalysisAsleepCore",
                "2024-01-02 00:00:00",
                "2024-01-02 06:00:00",
                "Watch",
            ),
            (
                1,
                WEIGHT,
                "80",
                "2024-01-02 07:00:00",
                "2024-01-02 07:00:00",
                "Scale",
            ),
            // user 2: other data that must never leak
            (
                2,
                STEPS,
                "99999",
                "2024-01-02 08:00:00",
                "2024-01-02 08:10:00",
                "Phone",
            ),
        ];
        for (i, (u, t, v, s, e, src)) in rows.iter().enumerate() {
            let su = format!("{}Z", s.replace(' ', "T"));
            let eu = format!("{}Z", e.replace(' ', "T"));
            sqlx::query(
                "INSERT INTO readings (user_id, type, value, unit, start_utc, end_utc, source, hash,
                 local_start, local_end) VALUES (?, ?, ?, 'u', ?, ?, ?, ?, ?, ?)",
            )
            .bind(u)
            .bind(t)
            .bind(v)
            .bind(su)
            .bind(eu)
            .bind(src)
            .bind(i.to_string())
            .bind(s)
            .bind(e)
            .execute(&db)
            .await
            .unwrap();
        }
        db
    }

    #[tokio::test]
    async fn activity_takes_best_source_and_isolates_users() {
        let db = seeded().await;
        let a = activity(&db, 1, None).await.unwrap();
        let r = a.rings.unwrap();
        assert_eq!(r.date, "2024-01-02");
        assert_eq!(r.steps, 1000.0);
        assert_eq!(r.stand_hours, 1.0);
        let b = activity(&db, 2, Some("2024-01-02".into())).await.unwrap();
        assert_eq!(b.rings.unwrap().steps, 99999.0);
        let empty = activity(&db, 3, None).await.unwrap();
        assert!(empty.rings.is_none());
    }

    async fn add(db: &SqlitePool, u: i64, t: &str, v: &str, s: &str, e: &str, src: &str) {
        sqlx::query(
            "INSERT INTO readings (user_id, type, value, unit, start_utc, end_utc, source, hash,
             local_start, local_end) VALUES (?, ?, ?, 'u', ?, ?, ?, ?, ?, ?)",
        )
        .bind(u)
        .bind(t)
        .bind(v)
        .bind(format!("{}Z", s.replace(' ', "T")))
        .bind(format!("{}Z", e.replace(' ', "T")))
        .bind(src)
        .bind(format!("{t}{v}{s}{e}{src}{u}"))
        .bind(s)
        .bind(e)
        .execute(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn ring_goals_from_activity_summary_with_defaults() {
        let db = seeded().await;
        let r = activity(&db, 1, None).await.unwrap().rings.unwrap();
        assert_eq!(
            (r.move_goal, r.exercise_goal, r.stand_goal),
            (DEFAULT_MOVE_GOAL, DEFAULT_EXERCISE_GOAL, DEFAULT_STAND_GOAL)
        );
        sqlx::query(
            "INSERT INTO activity_goals (user_id, date, move_kcal, exercise_min, stand_hours)
             VALUES (1, '2024-01-01', 650, 45, NULL), (2, '2024-01-02', 900, 60, 8)",
        )
        .execute(&db)
        .await
        .unwrap();
        // Earlier day's goals carry forward; a missing stand goal uses the default;
        // user 2's goals never apply to user 1.
        let r = activity(&db, 1, None).await.unwrap().rings.unwrap();
        assert_eq!(
            (r.move_goal, r.exercise_goal, r.stand_goal),
            (650.0, 45.0, 12.0)
        );
        let r2 = activity(&db, 2, Some("2024-01-02".into()))
            .await
            .unwrap()
            .rings
            .unwrap();
        assert_eq!(
            (r2.move_goal, r2.exercise_goal, r2.stand_goal),
            (900.0, 60.0, 8.0)
        );
    }

    #[tokio::test]
    async fn sleep_merges_overlapping_sources_and_ignores_in_bed_and_awake() {
        let db = crate::db::open_memory().await.unwrap();
        let v = |s: &str| format!("HKCategoryValueSleepAnalysis{s}");
        // Night ending 2024-01-02: Watch stages 23:00-06:00 with a 30 min awake gap,
        // iPhone InBed 22:30-06:30 and an unstaged iPhone "asleep" 23:30-05:00
        // that overlaps the Watch.
        for (st, s, e, src) in [
            (
                "InBed",
                "2024-01-01 22:30:00",
                "2024-01-02 06:30:00",
                "iPhone",
            ),
            (
                "AsleepUnspecified",
                "2024-01-01 23:30:00",
                "2024-01-02 05:00:00",
                "iPhone",
            ),
            (
                "AsleepCore",
                "2024-01-01 23:00:00",
                "2024-01-02 01:00:00",
                "Watch",
            ),
            (
                "AsleepDeep",
                "2024-01-02 01:00:00",
                "2024-01-02 02:00:00",
                "Watch",
            ),
            (
                "Awake",
                "2024-01-02 02:00:00",
                "2024-01-02 02:30:00",
                "Watch",
            ),
            (
                "AsleepREM",
                "2024-01-02 02:30:00",
                "2024-01-02 03:30:00",
                "Watch",
            ),
            (
                "AsleepCore",
                "2024-01-02 03:30:00",
                "2024-01-02 06:00:00",
                "Watch",
            ),
        ] {
            add(&db, 1, SLEEP, &v(st), s, e, src).await;
        }
        let s = sleep(&db, 1, "2024-01-02", 14).await.unwrap();
        assert_eq!(s.nights.len(), 1);
        let n = &s.nights[0];
        assert_eq!(n.source, "Watch");
        // Union 23:00-06:00 = 420 min; the iPhone sample fills the Watch's
        // awake gap but nothing is counted twice and InBed (480) is ignored.
        assert_eq!(n.asleep_min.round(), 420.0);
        assert_eq!(n.in_bed_min.round(), 0.0, "Watch has no InBed");
        // An InBed-only night is not a sleep night.
        add(
            &db,
            1,
            SLEEP,
            &v("InBed"),
            "2024-01-02 23:00:00",
            "2024-01-03 07:00:00",
            "iPhone",
        )
        .await;
        let s = sleep(&db, 1, "2024-01-03", 14).await.unwrap();
        assert_eq!(s.nights.len(), 1);
    }

    
    #[tokio::test]
    async fn flags_daytime_nap_separate_from_overnight() {
        let db = crate::db::open_memory().await.unwrap();
        async fn put(
            db: &sqlx::SqlitePool,
            value: &str,
            local_start: &str,
            local_end: &str,
            start_utc: &str,
            end_utc: &str,
        ) {
            sqlx::query(
                "INSERT INTO readings
                 (user_id, type, value, unit, start_utc, end_utc, tz_offset, source, channel, hash,
                  local_start, local_end)
                 VALUES (1, ?, ?, 'u', ?, ?, '-0500', 'Watch', 'export_xml', ?, ?, ?)",
            )
            .bind(SLEEP)
            .bind(value)
            .bind(start_utc)
            .bind(end_utc)
            .bind(format!("{value}-{start_utc}"))
            .bind(local_start)
            .bind(local_end)
            .execute(db)
            .await
            .unwrap();
        }
        // Overnight Core 22:30–06:30 local ending 2024-01-02.
        put(
            &db,
            "HKCategoryValueSleepAnalysisAsleepCore",
            "2024-01-01 22:30:00",
            "2024-01-02 06:30:00",
            "2024-01-02T03:30:00Z",
            "2024-01-02T11:30:00Z",
        )
        .await;
        // Daytime nap 13:00–14:00.
        put(
            &db,
            "HKCategoryValueSleepAnalysisAsleepUnspecified",
            "2024-01-02 13:00:00",
            "2024-01-02 14:00:00",
            "2024-01-02T18:00:00Z",
            "2024-01-02T19:00:00Z",
        )
        .await;
        let s = sleep(&db, 1, "2024-01-03", 14).await.unwrap();
        let main: Vec<_> = s.nights.iter().filter(|n| !n.is_nap).collect();
        let naps: Vec<_> = s.nights.iter().filter(|n| n.is_nap).collect();
        assert_eq!(main.len(), 1, "{:?}", s.nights);
        assert_eq!(main[0].date, "2024-01-02");
        assert!(main[0].asleep_min > 400.0, "{}", main[0].asleep_min);
        assert_eq!(naps.len(), 1, "{:?}", s.nights);
        assert_eq!(naps[0].date, "2024-01-02");
        assert!((naps[0].asleep_min - 60.0).abs() < 0.2, "{}", naps[0].asleep_min);
    }

#[tokio::test]
    async fn sleep_and_body() {
        let db = seeded().await;
        let s = sleep(&db, 1, "2024-01-02", 7).await.unwrap();
        assert_eq!(s.nights.len(), 1);
        assert_eq!(s.nights[0].deep_min.round(), 60.0);
        assert_eq!(s.nights[0].core_min.round(), 360.0);
        assert_eq!(s.last_night.len(), 2);
        assert!(sleep(&db, 2, "2024-01-02", 7)
            .await
            .unwrap()
            .nights
            .is_empty());
        let b = body(&db, 1, "2024-01-02", 30).await.unwrap();
        assert_eq!(b.weight.points[0].value, 80.0);
        assert!(body(&db, 2, "2024-01-02", 30)
            .await
            .unwrap()
            .weight
            .points
            .is_empty());
    }
}
