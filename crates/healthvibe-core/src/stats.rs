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
        "SELECT move_kcal, exercise_min, stand_hours FROM activity_goals_best
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

/// Sleep stage totals for one local date, from one source.
///
/// Attribution rule (the only rule applied): each imported sleep sample
/// belongs to the local calendar date of its **end** time. Samples are not
/// joined into sessions and nothing is classified as a nap. Per date the
/// preferred ingest channel is used (see `channel::rank`), then the source
/// with the most asleep minutes; sources are never mixed.
#[derive(Debug, Serialize, Default, Clone, PartialEq)]
pub struct Night {
    /// Local date of the samples' end time.
    pub date: String,
    pub awake_min: f64,
    pub rem_min: f64,
    pub core_min: f64,
    pub deep_min: f64,
    /// Asleep without stage detail (older devices).
    pub unspecified_min: f64,
    pub in_bed_min: f64,
    /// Sum of this source's asleep samples (REM + Core + Deep + unspecified)
    /// for the date. InBed and Awake are not included.
    pub asleep_min: f64,
    pub source: String,
    pub channel: String,
}

/// One imported sleep sample, as stored.
#[derive(Debug, Serialize, PartialEq)]
pub struct Segment {
    pub stage: String,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Serialize)]
pub struct Sleep {
    pub nights: Vec<Night>,
    /// Every sample (all stages, InBed included) of the most recent date's
    /// chosen source, ordered by start.
    pub last_night: Vec<Segment>,
}

fn stage(value: &str) -> &str {
    value
        .strip_prefix("HKCategoryValueSleepAnalysis")
        .unwrap_or(value)
}

pub async fn sleep(db: &SqlitePool, user_id: i64, end: &str, days: i64) -> anyhow::Result<Sleep> {
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
    .bind(shift(end, 1))
    .fetch_all(db)
    .await?;
    let mut by: BTreeMap<(String, String, String), Night> = BTreeMap::new();
    for (night, channel, source, value, min) in rows {
        let n = by
            .entry((night.clone(), channel.clone(), source.clone()))
            .or_insert(Night {
                date: night,
                source,
                channel,
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
    let mut best: BTreeMap<String, Night> = BTreeMap::new();
    for (_, mut n) in by {
        n.asleep_min = n.rem_min + n.core_min + n.deep_min + n.unspecified_min;
        let better = match best.get(&n.date) {
            None => true,
            Some(b) => {
                let (r, br) = (crate::channel::rank(&n.channel), crate::channel::rank(&b.channel));
                r < br
                    || (r == br
                        && (n.asleep_min, n.in_bed_min) > (b.asleep_min, b.in_bed_min))
            }
        };
        if better {
            best.insert(n.date.clone(), n);
        }
    }
    let nights: Vec<Night> = best.into_values().collect();
    let last_night = match nights.last() {
        Some(n) => sqlx::query_as::<_, (String, String, String)>(
            "SELECT value, local_start, local_end FROM readings
             WHERE user_id = ? AND type = ? AND substr(local_end, 1, 10) = ?
               AND channel = ? AND COALESCE(source, '') = ?
             ORDER BY local_start, local_end",
        )
        .bind(user_id)
        .bind(SLEEP)
        .bind(&n.date)
        .bind(&n.channel)
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
            "INSERT INTO activity_goals (user_id, date, channel, move_kcal, exercise_min, stand_hours)
             VALUES (1, '2024-01-01', 'export_xml', 650, 45, NULL),
                    (1, '2024-01-01', 'demo', 111, 11, 11),
                    (2, '2024-01-02', 'export_xml', 900, 60, 8)",
        )
        .execute(&db)
        .await
        .unwrap();
        // Earlier day's goals carry forward; export_xml beats demo for the whole row
        // (its NULL stand goal is not filled from demo); user 2's goals never apply.
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
    async fn sleep_is_samples_by_end_date_from_one_source() {
        let db = crate::db::open_memory().await.unwrap();
        let v = |s: &str| format!("HKCategoryValueSleepAnalysis{s}");
        // Samples ending 2024-01-02: Watch stages 23:00-06:00 with a 30 min
        // awake gap, a Watch daytime sample 13:00-14:00 (no nap rule: it is
        // simply another sample on that date), and iPhone InBed + unstaged
        // asleep samples that are never mixed with the Watch.
        for (st, s, e, src) in [
            ("InBed", "2024-01-01 22:30:00", "2024-01-02 06:30:00", "iPhone"),
            ("AsleepUnspecified", "2024-01-01 23:30:00", "2024-01-02 05:00:00", "iPhone"),
            ("AsleepCore", "2024-01-01 23:00:00", "2024-01-02 01:00:00", "Watch"),
            ("AsleepDeep", "2024-01-02 01:00:00", "2024-01-02 02:00:00", "Watch"),
            ("Awake", "2024-01-02 02:00:00", "2024-01-02 02:30:00", "Watch"),
            ("AsleepREM", "2024-01-02 02:30:00", "2024-01-02 03:30:00", "Watch"),
            ("AsleepCore", "2024-01-02 03:30:00", "2024-01-02 06:00:00", "Watch"),
            ("AsleepUnspecified", "2024-01-02 13:00:00", "2024-01-02 14:00:00", "Watch"),
        ] {
            add(&db, 1, SLEEP, &v(st), s, e, src).await;
        }
        let s = sleep(&db, 1, "2024-01-02", 14).await.unwrap();
        assert_eq!(s.nights.len(), 1, "{:?}", s.nights);
        let n = &s.nights[0];
        assert_eq!((n.date.as_str(), n.source.as_str()), ("2024-01-02", "Watch"));
        // Plain sum of the Watch's asleep samples: 120+60+60+150+60.
        assert_eq!(n.asleep_min.round(), 450.0);
        assert_eq!(n.unspecified_min.round(), 60.0);
        assert_eq!(n.awake_min.round(), 30.0);
        assert_eq!(n.in_bed_min.round(), 0.0, "Watch has no InBed");
        // Every Watch sample is returned as imported, in order.
        assert_eq!(s.last_night.len(), 6);
        assert_eq!(s.last_night[2].stage, "Awake");
        assert_eq!(s.last_night[5].start, "2024-01-02 13:00:00");
        // An InBed-only date is shown as what it is: in-bed time, 0 asleep.
        add(&db, 1, SLEEP, &v("InBed"), "2024-01-02 23:00:00", "2024-01-03 07:00:00", "iPhone").await;
        let s = sleep(&db, 1, "2024-01-03", 14).await.unwrap();
        assert_eq!(s.nights.len(), 2);
        assert_eq!((s.nights[1].asleep_min, s.nights[1].in_bed_min.round()), (0.0, 480.0));
        assert_eq!(s.last_night.len(), 1);
        assert_eq!(s.last_night[0].stage, "InBed");
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
