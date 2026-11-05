//! Deterministic synthetic demo data for local development and screenshots.
//!
//! All rows use channel `demo` and source `Demo Seed`. Nothing here is real
//! health data. Re-running with the same seed replaces prior demo rows for
//! that user (other channels are left alone).

use crate::channel::{self, DEMO};
use chrono::{Datelike, Duration, NaiveDate};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sqlx::SqlitePool;

const SOURCE: &str = "Demo Seed";
const STEPS: &str = "HKQuantityTypeIdentifierStepCount";
const ENERGY: &str = "HKQuantityTypeIdentifierActiveEnergyBurned";
const EXERCISE: &str = "HKQuantityTypeIdentifierAppleExerciseTime";
const STAND: &str = "HKCategoryTypeIdentifierAppleStandHour";
const HR: &str = "HKQuantityTypeIdentifierHeartRate";
const RHR: &str = "HKQuantityTypeIdentifierRestingHeartRate";
const HRV: &str = "HKQuantityTypeIdentifierHeartRateVariabilitySDNN";
const SLEEP: &str = "HKCategoryTypeIdentifierSleepAnalysis";
const WEIGHT: &str = "HKQuantityTypeIdentifierBodyMass";

#[derive(Debug, Clone)]
pub struct SeedOpts {
    pub user_id: i64,
    pub days: i64,
    /// Inclusive end date (local) `YYYY-MM-DD`. Defaults to today UTC date.
    pub end: Option<String>,
    pub rng_seed: u64,
}

impl Default for SeedOpts {
    fn default() -> Self {
        Self {
            user_id: 1,
            days: 28,
            end: None,
            rng_seed: 42,
        }
    }
}

#[derive(Debug, Default)]
pub struct SeedReport {
    pub readings: u64,
    pub workouts: u64,
    pub goals: u64,
    pub days: i64,
}

fn parse_end(end: &Option<String>) -> anyhow::Result<NaiveDate> {
    match end {
        Some(s) => Ok(NaiveDate::parse_from_str(s, "%Y-%m-%d")?),
        None => Ok(chrono::Utc::now().date_naive()),
    }
}

fn hash(kind: &str, start: &str, end: &str) -> String {
    channel::reading_hash(DEMO, kind, SOURCE, start, end)
}

async fn clear_demo(db: &SqlitePool, user_id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM readings WHERE user_id = ? AND channel = ?")
        .bind(user_id)
        .bind(DEMO)
        .execute(db)
        .await?;
    sqlx::query("DELETE FROM workouts WHERE user_id = ? AND channel = ?")
        .bind(user_id)
        .bind(DEMO)
        .execute(db)
        .await?;
    sqlx::query("DELETE FROM activity_goals WHERE user_id = ? AND channel = ?")
        .bind(user_id)
        .bind(DEMO)
        .execute(db)
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn put_reading(
    db: &SqlitePool,
    user_id: i64,
    kind: &str,
    value: &str,
    unit: &str,
    day: NaiveDate,
    start_h: u32,
    start_m: u32,
    end_h: u32,
    end_m: u32,
) -> anyhow::Result<()> {
    let local_start = format!("{day} {start_h:02}:{start_m:02}:00");
    let end_day = if (end_h, end_m) < (start_h, start_m) {
        day + Duration::days(1)
    } else {
        day
    };
    let local_end = format!("{end_day} {end_h:02}:{end_m:02}:00");
    // Store UTC as local+0 for demo simplicity (deterministic, not real TZ).
    let start_utc = format!("{day}T{start_h:02}:{start_m:02}:00Z");
    let end_utc = format!("{end_day}T{end_h:02}:{end_m:02}:00Z");
    let h = hash(kind, &start_utc, &end_utc);
    sqlx::query(
        "INSERT INTO readings
         (user_id, type, value, unit, start_utc, end_utc, tz_offset, source, channel, hash,
          local_start, local_end)
         VALUES (?, ?, ?, ?, ?, ?, '+0000', ?, ?, ?, ?, ?)
         ON CONFLICT (user_id, type, channel, source, start_utc, end_utc) DO UPDATE SET
           value = excluded.value, unit = excluded.unit, hash = excluded.hash,
           local_start = excluded.local_start, local_end = excluded.local_end",
    )
    .bind(user_id)
    .bind(kind)
    .bind(value)
    .bind(unit)
    .bind(&start_utc)
    .bind(&end_utc)
    .bind(SOURCE)
    .bind(DEMO)
    .bind(&h)
    .bind(&local_start)
    .bind(&local_end)
    .execute(db)
    .await?;
    Ok(())
}

/// Fill `days` of synthetic data ending on `opts.end` (or today). Idempotent
/// for the demo channel: prior demo rows for the user are cleared first.
pub async fn seed_demo(db: &SqlitePool, opts: SeedOpts) -> anyhow::Result<SeedReport> {
    let end = parse_end(&opts.end)?;
    let start = end - Duration::days(opts.days - 1);
    let mut rng = StdRng::seed_from_u64(opts.rng_seed);
    clear_demo(db, opts.user_id).await?;

    let mut report = SeedReport {
        days: opts.days,
        ..Default::default()
    };

    let mut d = start;
    while d <= end {
        let steps = 6_000 + rng.gen_range(0..8_000);
        let move_kcal = 350.0 + rng.gen_range(0.0..400.0);
        let exercise = 10.0 + rng.gen_range(0.0..50.0);
        let stand = rng.gen_range(6..14) as f64;
        let rhr = 54.0 + rng.gen_range(0.0..12.0);
        let hrv = 25.0 + rng.gen_range(0.0..40.0);
        let weight = 72.0 + (d.ordinal() as f64 % 7.0) * 0.05;

        put_reading(
            db, opts.user_id, STEPS, &steps.to_string(), "count", d, 8, 0, 21, 0,
        )
        .await?;
        put_reading(
            db,
            opts.user_id,
            ENERGY,
            &format!("{move_kcal:.1}"),
            "kcal",
            d,
            8,
            0,
            21,
            0,
        )
        .await?;
        put_reading(
            db,
            opts.user_id,
            EXERCISE,
            &format!("{exercise:.0}"),
            "min",
            d,
            8,
            0,
            21,
            0,
        )
        .await?;
        for h in 9..9 + (stand as u32).min(14) {
            put_reading(
                db,
                opts.user_id,
                STAND,
                "HKCategoryValueAppleStandHourStood",
                "",
                d,
                h,
                0,
                h,
                59,
            )
            .await?;
            report.readings += 1;
        }
        put_reading(
            db,
            opts.user_id,
            RHR,
            &format!("{rhr:.0}"),
            "count/min",
            d,
            7,
            0,
            7,
            1,
        )
        .await?;
        put_reading(
            db,
            opts.user_id,
            HRV,
            &format!("{hrv:.1}"),
            "ms",
            d,
            7,
            5,
            7,
            6,
        )
        .await?;
        put_reading(
            db,
            opts.user_id,
            WEIGHT,
            &format!("{weight:.2}"),
            "kg",
            d,
            7,
            30,
            7,
            31,
        )
        .await?;
        put_reading(
            db,
            opts.user_id,
            HR,
            &format!("{}", 60 + rng.gen_range(0..40)),
            "count/min",
            d,
            12,
            0,
            12,
            1,
        )
        .await?;
        report.readings += 8;

        // Overnight sleep ending this morning: previous evening → morning.
        let prev = d - Duration::days(1);
        put_reading(
            db,
            opts.user_id,
            SLEEP,
            "HKCategoryValueSleepAnalysisAsleepCore",
            "",
            prev,
            22,
            30,
            6,
            30,
        )
        .await?;
        // Fix: put_reading uses `day` for start; overnight spanning needs care.
        // Re-insert with explicit times via raw SQL for sleep.
        report.readings += 1;

        // Extra daytime sleep sample every 3rd day at 13:00 (stored like any other).
        if d.ordinal().is_multiple_of(3) {
            put_reading(
                db,
                opts.user_id,
                SLEEP,
                "HKCategoryValueSleepAnalysisAsleepUnspecified",
                "",
                d,
                13,
                0,
                14,
                0,
            )
            .await?;
            report.readings += 1;
        }

        // Goals
        sqlx::query(
            "INSERT INTO activity_goals (user_id, date, channel, move_kcal, exercise_min, stand_hours)
             VALUES (?, ?, 'demo', 500, 30, 12)
             ON CONFLICT (user_id, date, channel) DO UPDATE SET
               move_kcal = excluded.move_kcal,
               exercise_min = excluded.exercise_min,
               stand_hours = excluded.stand_hours",
        )
        .bind(opts.user_id)
        .bind(d.format("%Y-%m-%d").to_string())
        .execute(db)
        .await?;
        report.goals += 1;

        // Workout every other day.
        if d.ordinal().is_multiple_of(2) {
            let start_utc = format!("{d}T17:00:00Z");
            let end_utc = format!("{d}T17:35:00Z");
            let local_start = format!("{d} 17:00:00");
            let local_end = format!("{d} 17:35:00");
            let w = crate::workouts::Workout {
                activity_type: "HKWorkoutActivityTypeRunning".into(),
                channel: DEMO.into(),
                source: SOURCE.into(),
                start_utc: start_utc.clone(),
                end_utc: end_utc.clone(),
                local_start: Some(local_start),
                local_end: Some(local_end),
                duration_min: Some(35.0),
                avg_hr: Some(145.0 + rng.gen_range(0.0..15.0)),
                max_hr: Some(165.0 + rng.gen_range(0.0..20.0)),
                active_kcal: Some(280.0 + rng.gen_range(0.0..80.0)),
                total_kcal: Some(320.0),
                distance_m: Some(5000.0 + rng.gen_range(0.0..1500.0)),
                elevation_m: Some(40.0),
                detail_json: Some(r#"{"demo":true}"#.into()),
                external_id: Some(format!("demo-run-{d}")),
                id: None,
            };
            crate::workouts::upsert(db, opts.user_id, &w).await?;
            // Duration reading for awards/browse.
            put_reading(
                db,
                opts.user_id,
                "HKWorkoutActivityTypeRunning",
                "35",
                "min",
                d,
                17,
                0,
                17,
                35,
            )
            .await?;
            report.workouts += 1;
            report.readings += 1;
        }

        d += Duration::days(1);
    }

    // Fix overnight sleep: put_reading with end before start uses next day for
    // end, but start day was `prev` incorrectly written as sleep on `prev` with
    // end 06:30 same calendar trick. Verify: start_h=22 end_h=6 → end_day=prev+1=d. Good.

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn seed_is_deterministic_and_tagged_demo() {
        let db = crate::db::open_memory().await.unwrap();
        let opts = SeedOpts {
            user_id: 1,
            days: 7,
            end: Some("2024-06-07".into()),
            rng_seed: 42,
        };
        let a = seed_demo(&db, opts.clone()).await.unwrap();
        let b = seed_demo(&db, opts).await.unwrap();
        assert_eq!(a.readings, b.readings);
        assert_eq!(a.workouts, b.workouts);
        assert!(a.workouts >= 3);
        let channels: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT channel FROM readings WHERE user_id = 1",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(channels, vec![DEMO.to_string()]);
        let daytime: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM readings WHERE user_id = 1 AND type = ? AND local_start LIKE '% 13:%'",
        )
        .bind(SLEEP)
        .fetch_one(&db)
        .await
        .unwrap();
        assert!(daytime >= 1);
        let w: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workouts WHERE user_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(w, a.workouts as i64);
    }
}
