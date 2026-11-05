//! Apple Health-style Browse: categories, latest values, raw record pages,
//! and per-user pinned metrics. Every query is scoped by `user_id`.

use serde::Serialize;
use sqlx::SqlitePool;

pub const MAX_PINNED: usize = 30;
pub const MAX_PAGE: i64 = 200;

/// Browse category for a metric type id (matches the web app's category list).
pub fn category(kind: &str) -> &'static str {
    let k = kind.to_ascii_lowercase();
    let has = |ws: &[&str]| ws.iter().any(|w| k.contains(w));
    if has(&["sleep"]) {
        "sleep"
    } else if has(&["respiratory", "oxygen", "vo2", "breath"]) {
        "respiratory"
    } else if has(&["heart", "hrv", "pulse"]) {
        "heart"
    } else if has(&[
        "walkingspeed",
        "walking_speed",
        "steplength",
        "step_length",
        "asymmetry",
        "doublesupport",
        "double_support",
        "stairspeed",
        "stair_speed",
        "steadiness",
        "sixminute",
        "six_minute",
    ]) {
        "mobility"
    } else if has(&[
        "bodymass",
        "body_mass",
        "weight",
        "bodyfat",
        "body_fat",
        "leanbody",
        "lean_body",
        "height",
        "waist",
    ]) {
        "body"
    } else if has(&["bloodpressure", "blood_pressure", "temperature", "glucose"]) {
        "vitals"
    } else if has(&["dietary", "water"]) {
        "nutrition"
    } else if has(&["mindful"]) {
        "mindfulness"
    } else if has(&[
        "step", "distance", "energy", "exercise", "stand", "flights", "workout", "move",
        "daylight", "swim", "cycling",
    ]) {
        "activity"
    } else {
        "other"
    }
}

#[derive(Debug, Serialize)]
pub struct Record {
    pub id: i64,
    pub value: Option<String>,
    pub unit: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub source: Option<String>,
}

/// Newest-first page of individual records for one metric. `before` is the
/// last id seen (keyset pagination).
pub async fn records(
    db: &SqlitePool,
    uid: i64,
    kind: &str,
    before: Option<i64>,
    limit: i64,
) -> anyhow::Result<Vec<Record>> {
    type Row = (
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, value, unit, COALESCE(local_start, start_utc), COALESCE(local_end, end_utc), source
         FROM readings WHERE user_id = ? AND type = ? AND (? IS NULL OR id < ?)
         ORDER BY id DESC LIMIT ?",
    )
    .bind(uid)
    .bind(kind)
    .bind(before)
    .bind(before)
    .bind(limit.clamp(1, MAX_PAGE))
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, value, unit, start, end, source)| Record {
            id,
            value,
            unit,
            start,
            end,
            source,
        })
        .collect())
}

/// Latest (value, local start) per metric for this user.
pub async fn latest(
    db: &SqlitePool,
    uid: i64,
) -> anyhow::Result<Vec<(String, Option<String>, Option<String>)>> {
    Ok(sqlx::query_as(
        "SELECT r.type, r.value, COALESCE(r.local_start, r.start_utc) FROM readings r
         JOIN (SELECT type, MAX(COALESCE(local_start, start_utc)) m FROM readings WHERE user_id = ? GROUP BY type) x
           ON x.type = r.type AND x.m = COALESCE(r.local_start, r.start_utc)
         WHERE r.user_id = ? GROUP BY r.type",
    )
    .bind(uid)
    .bind(uid)
    .fetch_all(db)
    .await?)
}

pub async fn pinned(db: &SqlitePool, uid: i64) -> anyhow::Result<Vec<String>> {
    Ok(
        sqlx::query_scalar("SELECT metric FROM pinned_metrics WHERE user_id = ? ORDER BY position")
            .bind(uid)
            .fetch_all(db)
            .await?,
    )
}

/// Replaces the pinned list (order = position). Caller validates length.
pub async fn set_pinned(db: &SqlitePool, uid: i64, metrics: &[String]) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM pinned_metrics WHERE user_id = ?")
        .bind(uid)
        .execute(&mut *tx)
        .await?;
    for (i, m) in metrics.iter().enumerate() {
        sqlx::query(
            "INSERT OR IGNORE INTO pinned_metrics (user_id, metric, position) VALUES (?, ?, ?)",
        )
        .bind(uid)
        .bind(m)
        .bind(i as i64)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// 24 hourly values for one local day (sum for additive metrics, else avg),
/// taking the source with the highest total per hour like the daily sums.
pub async fn hourly(db: &SqlitePool, uid: i64, kind: &str, date: &str) -> anyhow::Result<Vec<f64>> {
    let agg = crate::dashboards::default_agg(kind);
    let sql = if agg == "count" {
        // Categories (stand hours, sleep...): one per hour that has a non-idle sample.
        "SELECT CAST(substr(local_start, 12, 2) AS INTEGER) h, CAST(COUNT(*) > 0 AS REAL)
           FROM readings WHERE user_id = ? AND type = ? AND substr(local_start, 1, 10) = ?
             AND COALESCE(value, '') NOT LIKE '%Idle' GROUP BY h".to_string()
    } else if agg == "sum" {
        format!(
            "SELECT h, t FROM (
               SELECT h, t, ROW_NUMBER() OVER (PARTITION BY h ORDER BY {rank} ASC, t DESC) rn
               FROM (
                 SELECT CAST(substr(local_start, 12, 2) AS INTEGER) h, channel, source,
                        SUM(CAST(value AS REAL)) t
                 FROM readings WHERE user_id = ? AND type = ? AND substr(local_start, 1, 10) = ?
                 GROUP BY h, channel, source
               )
             ) WHERE rn = 1",
            rank = crate::channel::RANK_SQL,
        )
    } else {
        "SELECT CAST(substr(local_start, 12, 2) AS INTEGER) h, AVG(CAST(value AS REAL))
           FROM readings WHERE user_id = ? AND type = ? AND substr(local_start, 1, 10) = ? GROUP BY h".to_string()
    };
    let rows: Vec<(i64, f64)> = sqlx::query_as(&sql)
        .bind(uid)
        .bind(kind)
        .bind(date)
        .fetch_all(db)
        .await?;
    let mut out = vec![0.0; 24];
    for (h, v) in rows {
        if (0..24).contains(&h) {
            out[h as usize] = v;
        }
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
pub struct RingDay {
    pub date: String,
    pub move_kcal: f64,
    pub move_goal: Option<f64>,
    pub exercise_min: f64,
    pub exercise_goal: Option<f64>,
    pub stand_hours: f64,
    pub stand_goal: Option<f64>,
    pub workout: bool,
}

/// Activity rings for every day in `[start, end]` (at most ~13 months).
pub async fn rings(
    db: &SqlitePool,
    uid: i64,
    start: &str,
    end: &str,
) -> anyhow::Result<Vec<RingDay>> {
    rings_span(db, uid, start, end, 400).await
}

/// Like [`rings`] with a caller-chosen day cap. Each day's goals come only from
/// that day's own ActivitySummary; a day without one has no goal.
pub async fn rings_span(
    db: &SqlitePool,
    uid: i64,
    start: &str,
    end: &str,
    cap: i64,
) -> anyhow::Result<Vec<RingDay>> {
    use crate::stats::*;
    let (Ok(s), Ok(e)) = (
        chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d"),
        chrono::NaiveDate::parse_from_str(end, "%Y-%m-%d"),
    ) else {
        return Ok(vec![]);
    };
    let days = ((e - s).num_days() + 1).clamp(0, cap);
    if days == 0 {
        return Ok(vec![]);
    }
    let energy = daily_sum(db, uid, ACTIVE_ENERGY, end, days).await?;
    let exercise = daily_sum(db, uid, EXERCISE, end, days).await?;
    let next = (e + chrono::Duration::days(1)).to_string();
    let stand: Vec<(String, f64)> = sqlx::query_as(
        "SELECT substr(local_start, 1, 10) d, CAST(COUNT(DISTINCT substr(local_start, 1, 13)) AS REAL) FROM readings
         WHERE user_id = ? AND type = ? AND value = 'HKCategoryValueAppleStandHourStood' AND local_start >= ? AND local_start < ?
         GROUP BY d",
    ).bind(uid).bind(STAND_HOUR).bind(start).bind(&next).fetch_all(db).await?;
    let workouts: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT substr(local_start, 1, 10) FROM readings
         WHERE user_id = ? AND type LIKE 'HKWorkoutActivityType%' AND local_start >= ? AND local_start < ?",
    ).bind(uid).bind(start).bind(&next).fetch_all(db).await?;
    type Goal = (String, Option<f64>, Option<f64>, Option<f64>);
    let goals: Vec<Goal> = sqlx::query_as(
        "SELECT date, move_kcal, exercise_min, stand_hours FROM activity_goals_best WHERE user_id = ? AND date >= ? AND date <= ? ORDER BY date",
    ).bind(uid).bind(start).bind(end).fetch_all(db).await?;
    let val = |v: &[DayValue], d: &str| v.iter().find(|x| x.date == d).map_or(0.0, |x| x.value);
    Ok((0..days)
        .map(|i| {
            let d = (s + chrono::Duration::days(i)).to_string();
            let g = goals.iter().find(|g| g.0 == d);
            RingDay {
                move_kcal: val(&energy, &d),
                move_goal: g.and_then(|g| g.1),
                exercise_min: val(&exercise, &d),
                exercise_goal: g.and_then(|g| g.2),
                stand_hours: stand.iter().find(|x| x.0 == d).map_or(0.0, |x| x.1),
                stand_goal: g.and_then(|g| g.3),
                workout: workouts.contains(&d),
                date: d,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::category;
    #[test]
    fn categories() {
        assert_eq!(category("HKQuantityTypeIdentifierStepCount"), "activity");
        assert_eq!(
            category("HKQuantityTypeIdentifierRestingHeartRate"),
            "heart"
        );
        assert_eq!(
            category("HKQuantityTypeIdentifierHeartRateVariabilitySDNN"),
            "heart"
        );
        assert_eq!(category("HKCategoryTypeIdentifierSleepAnalysis"), "sleep");
        assert_eq!(category("HKQuantityTypeIdentifierBodyMass"), "body");
        assert_eq!(category("HKQuantityTypeIdentifierWalkingSpeed"), "mobility");
        assert_eq!(
            category("HKQuantityTypeIdentifierRespiratoryRate"),
            "respiratory"
        );
        assert_eq!(category("healthvibe.hae.tooth_brushing"), "other");
    }
}
