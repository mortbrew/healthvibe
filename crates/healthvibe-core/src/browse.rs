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
             AND COALESCE(value, '') NOT LIKE '%Idle' GROUP BY h"
    } else if agg == "sum" {
        "SELECT h, MAX(t) FROM (SELECT CAST(substr(local_start, 12, 2) AS INTEGER) h, source, SUM(CAST(value AS REAL)) t
           FROM readings WHERE user_id = ? AND type = ? AND substr(local_start, 1, 10) = ? GROUP BY h, source) GROUP BY h"
    } else {
        "SELECT CAST(substr(local_start, 12, 2) AS INTEGER) h, AVG(CAST(value AS REAL))
           FROM readings WHERE user_id = ? AND type = ? AND substr(local_start, 1, 10) = ? GROUP BY h"
    };
    let rows: Vec<(i64, f64)> = sqlx::query_as(sql)
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

#[derive(Debug, Serialize, PartialEq)]
pub struct Award {
    pub key: &'static str,
    pub name: String,
    pub detail: String,
    pub date: Option<String>,
}

fn longest_run(days: &[(String, bool)]) -> (usize, Option<String>) {
    let (mut best, mut cur, mut end) = (0, 0, None);
    let mut prev: Option<chrono::NaiveDate> = None;
    for (d, ok) in days {
        let date = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok();
        let consecutive =
            matches!((prev, date), (Some(p), Some(x)) if x - p == chrono::Duration::days(1));
        cur = if *ok {
            if consecutive {
                cur + 1
            } else {
                1
            }
        } else {
            0
        };
        if cur > best {
            best = cur;
            end = Some(d.clone());
        }
        prev = date;
    }
    (best, end)
}

/// healthvibe's own awards (Apple's are not in the export), newest-earned first.
pub async fn awards(db: &SqlitePool, uid: i64) -> anyhow::Result<Vec<Award>> {
    use crate::stats::{daily_sum, ACTIVE_ENERGY, STEPS};
    let Some(end) = crate::stats::latest_day(db, uid).await? else {
        return Ok(vec![]);
    };
    let steps = daily_sum(db, uid, STEPS, &end, 3650).await?;
    let energy = daily_sum(db, uid, ACTIVE_ENERGY, &end, 3650).await?;
    let goal: f64 = sqlx::query_scalar("SELECT move_kcal FROM activity_goals WHERE user_id = ? AND move_kcal IS NOT NULL ORDER BY date DESC LIMIT 1")
        .bind(uid).fetch_optional(db).await?.unwrap_or(crate::stats::DEFAULT_MOVE_GOAL);
    let mut out = vec![];
    let move_days: Vec<(String, bool)> = energy
        .iter()
        .map(|d| (d.date.clone(), d.value >= goal))
        .collect();
    let (run, run_end) = longest_run(&move_days);
    if run >= 7 {
        out.push(Award {
            key: "perfect_week",
            name: "Perfect Week (Move)".into(),
            detail: format!("Closed your Move ring {run} days in a row"),
            date: run_end.clone(),
        });
    }
    if run >= 2 {
        out.push(Award {
            key: "move_streak",
            name: format!("{run}-Day Move Streak"),
            detail: "Longest Move streak".into(),
            date: run_end,
        });
    }
    if let Some(best) = steps.iter().max_by(|a, b| a.value.total_cmp(&b.value)) {
        out.push(Award {
            key: "record_steps",
            name: "New Step Record".into(),
            detail: format!("{} steps", best.value.round() as i64),
            date: Some(best.date.clone()),
        });
    }
    if let Some(d) = steps.iter().rev().find(|d| d.value >= 10_000.0) {
        out.push(Award {
            key: "ten_k",
            name: "10,000 Steps".into(),
            detail: format!(
                "{} days over 10k",
                steps.iter().filter(|d| d.value >= 10_000.0).count()
            ),
            date: Some(d.date.clone()),
        });
    }
    out.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(out)
}

#[derive(Debug, Serialize)]
pub struct RingDay {
    pub date: String,
    pub move_kcal: f64,
    pub move_goal: f64,
    pub exercise_min: f64,
    pub exercise_goal: f64,
    pub stand_hours: f64,
    pub stand_goal: f64,
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

/// Like [`rings`] with a caller-chosen day cap. Profile goal overrides win.
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
        "SELECT date, move_kcal, exercise_min, stand_hours FROM activity_goals WHERE user_id = ? AND date <= ? ORDER BY date",
    ).bind(uid).bind(end).fetch_all(db).await?;
    let (om, oe, os) = crate::profile::goal_overrides(db, uid).await?;
    let val = |v: &[DayValue], d: &str| v.iter().find(|x| x.date == d).map_or(0.0, |x| x.value);
    Ok((0..days)
        .map(|i| {
            let d = (s + chrono::Duration::days(i)).to_string();
            let g = goals.iter().rev().find(|g| g.0 <= d);
            RingDay {
                move_kcal: val(&energy, &d),
                move_goal: om.or(g.and_then(|g| g.1)).unwrap_or(DEFAULT_MOVE_GOAL),
                exercise_min: val(&exercise, &d),
                exercise_goal: oe.or(g.and_then(|g| g.2)).unwrap_or(DEFAULT_EXERCISE_GOAL),
                stand_hours: stand.iter().find(|x| x.0 == d).map_or(0.0, |x| x.1),
                stand_goal: os.or(g.and_then(|g| g.3)).unwrap_or(DEFAULT_STAND_GOAL),
                workout: workouts.contains(&d),
                date: d,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::category;
    #[allow(unused_imports)]
    use super::longest_run;
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
    #[test]
    fn runs() {
        let d = |s: &str, ok| (s.to_string(), ok);
        let v = vec![
            d("2026-01-01", true),
            d("2026-01-02", true),
            d("2026-01-04", true),
            d("2026-01-05", true),
            d("2026-01-06", true),
        ];
        assert_eq!(super::longest_run(&v), (3, Some("2026-01-06".into())));
    }
}
