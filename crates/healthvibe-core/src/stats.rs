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
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT d, MAX(total) FROM (
            SELECT substr(local_start, 1, 10) AS d, source, SUM(CAST(value AS REAL)) AS total
            FROM readings
            WHERE user_id = ? AND type = ? AND local_start >= ? AND local_start < ?
            GROUP BY d, source)
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
        "SELECT COUNT(DISTINCT substr(local_start, 1, 13)) FROM readings
         WHERE user_id = ? AND type = ? AND value = 'HKCategoryValueAppleStandHourStood'
           AND local_start >= ? AND local_start < ?",
    )
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
    let (om, oe, os) = crate::profile::goal_overrides(db, user_id).await?;
    let (gm, ge, gs) = (om.or(gm), oe.or(ge), os.or(gs));
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
    /// The date the user woke up.
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
    let rows: Vec<(String, String, String, f64)> = sqlx::query_as(
        "SELECT substr(local_end, 1, 10) AS night, COALESCE(source, ''), value,
                SUM((julianday(end_utc) - julianday(start_utc)) * 1440)
         FROM readings
         WHERE user_id = ? AND type = ? AND local_end >= ? AND local_end < ?
         GROUP BY night, source, value",
    )
    .bind(user_id)
    .bind(SLEEP)
    .bind(shift(end, 1 - days))
    .bind(shift(end, 2))
    .fetch_all(db)
    .await?;
    let mut by: BTreeMap<(String, String), Night> = BTreeMap::new();
    for (night, source, value, min) in rows {
        let n = by.entry((night.clone(), source.clone())).or_insert(Night {
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
    for ((night, _), n) in by {
        match best.get(&night) {
            Some(b) if asleep(b) >= asleep(&n) => {}
            _ => {
                best.insert(night, n);
            }
        }
    }
    // Merged asleep time across every source, per night.
    let ivs: Vec<(String, String, f64, f64)> = sqlx::query_as(
        "SELECT substr(local_end, 1, 10), value, julianday(start_utc), julianday(end_utc)
         FROM readings
         WHERE user_id = ? AND type = ? AND local_end >= ? AND local_end < ?",
    )
    .bind(user_id)
    .bind(SLEEP)
    .bind(shift(end, 1 - days))
    .bind(shift(end, 2))
    .fetch_all(db)
    .await?;
    let mut per: BTreeMap<String, Vec<(f64, f64)>> = BTreeMap::new();
    for (night, value, s, e) in ivs {
        if is_asleep(stage(&value)) && e > s {
            per.entry(night).or_default().push((s, e));
        }
    }
    let nights: Vec<Night> = best
        .into_values()
        .map(|mut n| {
            n.asleep_min = per
                .remove(&n.date)
                .map_or(0.0, |v| (union_minutes(v) * 10.0).round() / 10.0);
            n
        })
        .filter(|n| n.asleep_min > 0.0)
        .collect();
    let last_night = match nights.last() {
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
