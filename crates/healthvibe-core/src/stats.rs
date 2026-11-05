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
    let rows: Vec<(String, f64)> = sqlx::query_as(&format!(
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
    ))
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
    /// Resolved ring goal; see [`RingGoals`].
    pub move_goal: Option<f64>,
    pub move_goal_from: Option<String>,
    pub move_goal_source: Option<String>,
    pub exercise_min: f64,
    pub exercise_goal: Option<f64>,
    pub exercise_goal_from: Option<String>,
    pub exercise_goal_source: Option<String>,
    pub stand_hours: f64,
    pub stand_goal: Option<f64>,
    pub stand_goal_from: Option<String>,
    pub stand_goal_source: Option<String>,
    pub steps: f64,
}

/// One day's ring goals, per ring.
///
/// Resolution for local day `D`:
/// 1. If `D` has its own export.xml ActivitySummary value for that ring, use it.
/// 2. Else take the most recent goal dated on or before `D` across export goals
///    and manual entries (by effective date). On a same-date tie, export wins.
/// 3. A later manual entry overrides an older export going forward; a later
///    export takes over again. Days before any goal show none. Never a default.
///
/// `*_from` is the goal's date. For an export goal on `D` itself it is `None`
/// (same as the export-only reuse behaviour). For a manual goal it is always
/// the entry's `effective_from`, even when that equals `D`. `*_source` is
/// `"export"` or `"manual"` when a goal is present.
#[derive(Debug, Default, PartialEq)]
pub struct RingGoals {
    pub move_goal: Option<f64>,
    pub move_goal_from: Option<String>,
    pub move_goal_source: Option<String>,
    pub exercise_goal: Option<f64>,
    pub exercise_goal_from: Option<String>,
    pub exercise_goal_source: Option<String>,
    pub stand_goal: Option<f64>,
    pub stand_goal_from: Option<String>,
    pub stand_goal_source: Option<String>,
}

/// A candidate goal row for resolution: export ActivitySummary or manual entry.
#[derive(Debug, Clone, PartialEq)]
pub struct GoalRow {
    pub date: String,
    /// `"export"` or `"manual"`.
    pub source: &'static str,
    pub move_kcal: Option<f64>,
    pub exercise_min: Option<f64>,
    pub stand_hours: Option<f64>,
}

/// Export + manual goal rows for a user up to and including `end`, oldest first.
pub async fn goal_rows_upto(
    db: &SqlitePool,
    user_id: i64,
    end: &str,
) -> anyhow::Result<Vec<GoalRow>> {
    type ExportRow = (String, Option<f64>, Option<f64>, Option<f64>);
    let export: Vec<ExportRow> = sqlx::query_as(
        "SELECT date, move_kcal, exercise_min, stand_hours FROM activity_goals_best
         WHERE user_id = ? AND date <= ? ORDER BY date",
    )
    .bind(user_id)
    .bind(end)
    .fetch_all(db)
    .await?;
    let manual = crate::manual_goals::rows_upto(db, user_id, end).await?;
    let mut rows: Vec<GoalRow> = export
        .into_iter()
        .map(|(date, move_kcal, exercise_min, stand_hours)| GoalRow {
            date,
            source: "export",
            move_kcal,
            exercise_min,
            stand_hours,
        })
        .chain(
            manual
                .into_iter()
                .map(|(date, move_kcal, exercise_min, stand_hours)| GoalRow {
                    date,
                    source: "manual",
                    move_kcal,
                    exercise_min,
                    stand_hours,
                }),
        )
        .collect();
    // Date ascending; on a tie put export before manual so a reverse scan that
    // prefers later dates still sees export first when dates match and we
    // compare sources explicitly in `goals_for`.
    rows.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| (a.source != "export").cmp(&(b.source != "export")))
    });
    Ok(rows)
}

/// Goals for `date` from `rows` (as [`goal_rows_upto`] returns).
pub fn goals_for(rows: &[GoalRow], date: &str) -> RingGoals {
    let upto = &rows[..rows.partition_point(|r| r.date.as_str() <= date)];
    let pick = |f: fn(&GoalRow) -> Option<f64>| -> (Option<f64>, Option<String>, Option<String>) {
        let mut best: Option<&GoalRow> = None;
        for r in upto {
            if f(r).is_none() {
                continue;
            }
            best = Some(match best {
                None => r,
                Some(b) if r.date > b.date => r,
                // Same date: export beats manual.
                Some(b) if r.date == b.date && r.source == "export" && b.source == "manual" => r,
                Some(b) => b,
            });
        }
        match best {
            Some(r) => {
                let v = f(r).unwrap();
                let from = if r.source == "export" && r.date == date {
                    None
                } else {
                    Some(r.date.clone())
                };
                (Some(v), from, Some(r.source.to_string()))
            }
            None => (None, None, None),
        }
    };
    let (move_goal, move_goal_from, move_goal_source) = pick(|r| r.move_kcal);
    let (exercise_goal, exercise_goal_from, exercise_goal_source) = pick(|r| r.exercise_min);
    let (stand_goal, stand_goal_from, stand_goal_source) = pick(|r| r.stand_hours);
    RingGoals {
        move_goal,
        move_goal_from,
        move_goal_source,
        exercise_goal,
        exercise_goal_from,
        exercise_goal_source,
        stand_goal,
        stand_goal_from,
        stand_goal_source,
    }
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
    let stand: i64 = sqlx::query_scalar(&format!(
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
    ))
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
    // Goals: export ActivitySummary + manual entries (see [`RingGoals`]). No defaults.
    let g = goals_for(&goal_rows_upto(db, user_id, &date).await?, &date);
    let rings = Rings {
        date: date.clone(),
        move_kcal: on(&energy, &date),
        move_goal: g.move_goal,
        move_goal_from: g.move_goal_from,
        move_goal_source: g.move_goal_source,
        exercise_min: on(&exercise, &date),
        exercise_goal: g.exercise_goal,
        exercise_goal_from: g.exercise_goal_from,
        exercise_goal_source: g.exercise_goal_source,
        stand_hours: stand as f64,
        stand_goal: g.stand_goal,
        stand_goal_from: g.stand_goal_from,
        stand_goal_source: g.stand_goal_source,
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
                let (r, br) = (
                    crate::channel::rank(&n.channel),
                    crate::channel::rank(&b.channel),
                );
                r < br || (r == br && (n.asleep_min, n.in_bed_min) > (b.asleep_min, b.in_bed_min))
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
    async fn ring_goals_from_that_day_else_latest_earlier_import() {
        let db = seeded().await;
        let latest = latest_day(&db, 1).await.unwrap().unwrap();
        let r = activity(&db, 1, None).await.unwrap().rings.unwrap();
        // No ActivitySummary at all: no goal (no default).
        assert_eq!(
            (r.move_goal, r.exercise_goal, r.stand_goal),
            (None, None, None)
        );
        assert_eq!(r.move_goal_from, None);
        sqlx::query(
            "INSERT INTO activity_goals (user_id, date, channel, move_kcal, exercise_min, stand_hours)
             VALUES (1, ?, 'export_xml', 650, 45, NULL),
                    (1, ?, 'demo', 111, 11, 11),
                    (1, '2000-01-01', 'export_xml', 999, 99, 9),
                    (1, '2000-01-03', 'export_xml', 500, NULL, NULL),
                    (2, '1999-12-01', 'export_xml', 900, 60, 8)",
        )
        .bind(&latest)
        .bind(&latest)
        .execute(&db)
        .await
        .unwrap();
        let ring = |d: &str| {
            let db = db.clone();
            let d = d.to_string();
            async move { activity(&db, 1, Some(d)).await.unwrap().rings.unwrap() }
        };
        // Before the first imported goal: none (user 2's goals never apply).
        let r = ring("1999-12-31").await;
        assert_eq!(
            (r.move_goal, r.exercise_goal, r.stand_goal),
            (None, None, None)
        );
        // The day's own goals: no `_from` date.
        let r = ring("2000-01-01").await;
        assert_eq!(
            (r.move_goal, r.exercise_goal, r.stand_goal),
            (Some(999.0), Some(99.0), Some(9.0))
        );
        assert_eq!(
            (r.move_goal_from, r.exercise_goal_from, r.stand_goal_from),
            (None, None, None)
        );
        // A day without a summary reuses the latest earlier one.
        let r = ring("2000-01-02").await;
        assert_eq!(
            (r.move_goal, r.exercise_goal, r.stand_goal),
            (Some(999.0), Some(99.0), Some(9.0))
        );
        let from = Some("2000-01-01".to_string());
        assert_eq!(r.move_goal_from, from);
        assert_eq!(r.stand_goal_from, from);
        // Per ring: 01-03 has its own move goal; exercise/stand come from 01-01.
        let r = ring("2000-01-05").await;
        assert_eq!(
            (r.move_goal, r.exercise_goal, r.stand_goal),
            (Some(500.0), Some(99.0), Some(9.0))
        );
        assert_eq!(
            (r.move_goal_from.as_deref(), r.exercise_goal_from.as_deref()),
            (Some("2000-01-03"), Some("2000-01-01"))
        );
        // export_xml beats demo for the whole row on `latest`; its missing stand
        // goal is not taken from demo but from the latest earlier import.
        let r = ring(&latest).await;
        assert_eq!(
            (r.move_goal, r.exercise_goal, r.stand_goal),
            (Some(650.0), Some(45.0), Some(9.0))
        );
        assert_eq!(
            (r.move_goal_from, r.stand_goal_from.as_deref()),
            (None, Some("2000-01-01"))
        );
        // rings_span applies the same rule day by day.
        let days = crate::browse::rings_span(&db, 1, "1999-12-31", "2000-01-03", 10)
            .await
            .unwrap();
        let got: Vec<_> = days
            .iter()
            .map(|d| (d.move_goal, d.move_goal_from.as_deref(), d.exercise_goal))
            .collect();
        assert_eq!(
            got,
            vec![
                (None, None, None),
                (Some(999.0), None, Some(99.0)),
                (Some(999.0), Some("2000-01-01"), Some(99.0)),
                (Some(500.0), None, Some(99.0)),
            ]
        );
        assert_eq!(days[3].exercise_goal_from.as_deref(), Some("2000-01-01"));
        // Export-only path still reports source "export".
        assert_eq!(days[1].move_goal_source.as_deref(), Some("export"));
        assert_eq!(days[2].move_goal_source.as_deref(), Some("export"));
    }

    #[tokio::test]
    async fn ring_goals_manual_roll_forward_export_wins_ties_and_isolates() {
        let db = seeded().await;
        // Export on Jan 1; manual on Jan 5 (move only); export on Jan 10; manual on Jan 10 (tie).
        sqlx::query(
            "INSERT INTO activity_goals (user_id, date, channel, move_kcal, exercise_min, stand_hours)
             VALUES (1, '2000-01-01', 'export_xml', 500, 30, 8),
                    (1, '2000-01-10', 'export_xml', 700, 40, 10),
                    (2, '2000-01-01', 'export_xml', 999, 99, 9)",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO manual_goals (user_id, effective_from, move_kcal, exercise_min, stand_hours, created_at)
             VALUES (1, '2000-01-05', 600, NULL, NULL, '2000-01-05T00:00:00Z'),
                    (1, '2000-01-10', 650, 35, NULL, '2000-01-10T00:00:00Z'),
                    (2, '2000-01-05', 1, 1, 1, '2000-01-05T00:00:00Z')",
        )
        .execute(&db)
        .await
        .unwrap();

        let ring = |d: &str| {
            let db = db.clone();
            let d = d.to_string();
            async move { activity(&db, 1, Some(d)).await.unwrap().rings.unwrap() }
        };

        // Nothing before the first goal.
        let r = ring("1999-12-31").await;
        assert_eq!(
            (r.move_goal, r.exercise_goal, r.stand_goal),
            (None, None, None)
        );
        assert_eq!(r.move_goal_source, None);

        // Day's own export wins; no _from for export-on-day.
        let r = ring("2000-01-01").await;
        assert_eq!(
            (r.move_goal, r.exercise_goal, r.stand_goal),
            (Some(500.0), Some(30.0), Some(8.0))
        );
        assert_eq!(
            (r.move_goal_from, r.move_goal_source.as_deref()),
            (None, Some("export"))
        );

        // Export rolls forward.
        let r = ring("2000-01-03").await;
        assert_eq!(r.move_goal, Some(500.0));
        assert_eq!(
            (r.move_goal_from.as_deref(), r.move_goal_source.as_deref()),
            (Some("2000-01-01"), Some("export"))
        );

        // Later manual overrides export going forward (move only; exercise/stand stay export).
        let r = ring("2000-01-05").await;
        assert_eq!(
            (r.move_goal, r.exercise_goal, r.stand_goal),
            (Some(600.0), Some(30.0), Some(8.0))
        );
        assert_eq!(
            (
                r.move_goal_from.as_deref(),
                r.move_goal_source.as_deref(),
                r.exercise_goal_from.as_deref(),
                r.exercise_goal_source.as_deref()
            ),
            (
                Some("2000-01-05"),
                Some("manual"),
                Some("2000-01-01"),
                Some("export")
            )
        );
        let r = ring("2000-01-07").await;
        assert_eq!(r.move_goal, Some(600.0));
        assert_eq!(r.move_goal_source.as_deref(), Some("manual"));

        // Same-date tie: export beats manual. Manual's exercise does not apply on that day
        // for rings the export provides; export move/exercise/stand win.
        let r = ring("2000-01-10").await;
        assert_eq!(
            (r.move_goal, r.exercise_goal, r.stand_goal),
            (Some(700.0), Some(40.0), Some(10.0))
        );
        assert_eq!(
            (r.move_goal_from, r.move_goal_source.as_deref()),
            (None, Some("export"))
        );

        // Later export takes over again after the Jan 5 manual.
        let r = ring("2000-01-12").await;
        assert_eq!(
            (r.move_goal, r.exercise_goal, r.stand_goal),
            (Some(700.0), Some(40.0), Some(10.0))
        );
        assert_eq!(
            (r.move_goal_from.as_deref(), r.move_goal_source.as_deref()),
            (Some("2000-01-10"), Some("export"))
        );

        // User isolation: user 2's goals never apply to user 1; empty user has none.
        let other = activity(&db, 2, Some("2000-01-05".into()))
            .await
            .unwrap()
            .rings
            .unwrap();
        assert_eq!(other.move_goal, Some(1.0));
        let empty = activity(&db, 3, Some("2000-01-05".into()))
            .await
            .unwrap()
            .rings
            .unwrap();
        // User 3 has no goals of their own (user 1/2 data never leaks).
        assert_eq!(
            (empty.move_goal, empty.exercise_goal, empty.stand_goal),
            (None, None, None)
        );

        // Partial manual only: clear goals and set stand-only manual.
        sqlx::query("DELETE FROM activity_goals WHERE user_id = 1")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("DELETE FROM manual_goals WHERE user_id = 1")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO manual_goals (user_id, effective_from, move_kcal, exercise_min, stand_hours, created_at)
             VALUES (1, '2000-02-01', NULL, NULL, 12, '2000-02-01T00:00:00Z')",
        )
        .execute(&db)
        .await
        .unwrap();
        let r = ring("2000-02-03").await;
        assert_eq!(
            (r.move_goal, r.exercise_goal, r.stand_goal),
            (None, None, Some(12.0))
        );
        assert_eq!(r.stand_goal_source.as_deref(), Some("manual"));
        assert_eq!(r.stand_goal_from.as_deref(), Some("2000-02-01"));
    }

    #[tokio::test]
    async fn manual_goals_crud_is_user_scoped() {
        let db = seeded().await;
        use crate::manual_goals::{self, ManualGoalInput};
        let a = ManualGoalInput {
            effective_from: "2024-01-01".into(),
            move_kcal: Some(400.0),
            exercise_min: Some(30.0),
            stand_hours: Some(12.0),
        };
        let g = manual_goals::create(&db, 1, &a).await.unwrap();
        assert_eq!(g.move_kcal, Some(400.0));
        let listed = manual_goals::list(&db, 1).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert!(manual_goals::list(&db, 2).await.unwrap().is_empty());
        let upd = ManualGoalInput {
            effective_from: "2024-01-02".into(),
            move_kcal: Some(450.0),
            exercise_min: None,
            stand_hours: Some(10.0),
        };
        let g2 = manual_goals::update(&db, 1, g.id, &upd)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(g2.effective_from, "2024-01-02");
        assert_eq!(g2.move_kcal, Some(450.0));
        assert_eq!(g2.exercise_min, None);
        // Other user cannot update/delete.
        assert!(manual_goals::update(&db, 2, g.id, &upd)
            .await
            .unwrap()
            .is_none());
        assert!(!manual_goals::delete(&db, 2, g.id).await.unwrap());
        assert!(manual_goals::delete(&db, 1, g.id).await.unwrap());
        assert!(manual_goals::list(&db, 1).await.unwrap().is_empty());
        assert_eq!(
            manual_goals::validate(&ManualGoalInput {
                effective_from: "bad".into(),
                move_kcal: Some(1.0),
                exercise_min: None,
                stand_hours: None,
            }),
            Err("invalid_date")
        );
        assert_eq!(
            manual_goals::validate(&ManualGoalInput {
                effective_from: "2024-01-01".into(),
                move_kcal: None,
                exercise_min: None,
                stand_hours: None,
            }),
            Err("empty_goal")
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
            (
                "AsleepUnspecified",
                "2024-01-02 13:00:00",
                "2024-01-02 14:00:00",
                "Watch",
            ),
        ] {
            add(&db, 1, SLEEP, &v(st), s, e, src).await;
        }
        let s = sleep(&db, 1, "2024-01-02", 14).await.unwrap();
        assert_eq!(s.nights.len(), 1, "{:?}", s.nights);
        let n = &s.nights[0];
        assert_eq!(
            (n.date.as_str(), n.source.as_str()),
            ("2024-01-02", "Watch")
        );
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
        assert_eq!(s.nights.len(), 2);
        assert_eq!(
            (s.nights[1].asleep_min, s.nights[1].in_bed_min.round()),
            (0.0, 480.0)
        );
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
