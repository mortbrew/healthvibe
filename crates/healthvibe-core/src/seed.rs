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
    /// Inclusive end date (local calendar) `YYYY-MM-DD`. Defaults to today
    /// in the user's profile IANA time zone (falling back to process-local).
    /// Imported Health `local_*` dates are never rewritten from the profile TZ.
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

fn parse_end(end: &Option<String>, timezone: Option<&str>) -> anyhow::Result<NaiveDate> {
    match end {
        Some(s) => Ok(NaiveDate::parse_from_str(s, "%Y-%m-%d")?),
        None => Ok(crate::profile::today_in(timezone)),
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

/// One Watch-style night ending the morning of `end_day`: Awake / REM / Core / Deep.
/// Every segment's `local_end` is on `end_day` so end-date attribution shows the
/// full evening→morning window (no daytime samples). Returns rows inserted.
async fn put_sleep_night(db: &SqlitePool, user_id: i64, end_day: NaiveDate) -> anyhow::Result<u64> {
    let prev = end_day - Duration::days(1);
    // (stage, day, start_h, start_m, end_h, end_m) — overnight via end < start on `prev`.
    let segs: &[(&str, NaiveDate, u32, u32, u32, u32)] = &[
        // First Core crosses midnight so the evening appears on this night date.
        ("HKCategoryValueSleepAnalysisAsleepCore", prev, 22, 10, 1, 5),
        (
            "HKCategoryValueSleepAnalysisAsleepDeep",
            end_day,
            1,
            5,
            2,
            0,
        ),
        (
            "HKCategoryValueSleepAnalysisAsleepCore",
            end_day,
            2,
            0,
            3,
            10,
        ),
        (
            "HKCategoryValueSleepAnalysisAsleepREM",
            end_day,
            3,
            10,
            3,
            55,
        ),
        ("HKCategoryValueSleepAnalysisAwake", end_day, 3, 55, 4, 5),
        (
            "HKCategoryValueSleepAnalysisAsleepCore",
            end_day,
            4,
            5,
            5,
            25,
        ),
        (
            "HKCategoryValueSleepAnalysisAsleepDeep",
            end_day,
            5,
            25,
            6,
            5,
        ),
        (
            "HKCategoryValueSleepAnalysisAsleepREM",
            end_day,
            6,
            5,
            6,
            55,
        ),
        ("HKCategoryValueSleepAnalysisAwake", end_day, 6, 55, 7, 10),
    ];
    for (stage, day, sh, sm, eh, em) in segs {
        put_reading(db, user_id, SLEEP, stage, "", *day, *sh, *sm, *eh, *em).await?;
    }
    Ok(segs.len() as u64)
}

/// Fill `days` of synthetic data ending on `opts.end` (or today). Idempotent
/// for the demo channel: prior demo rows for the user are cleared first.
pub async fn seed_demo(db: &SqlitePool, opts: SeedOpts) -> anyhow::Result<SeedReport> {
    let tz = crate::profile::get(db, opts.user_id)
        .await
        .ok()
        .and_then(|p| p.timezone);
    let end = parse_end(&opts.end, tz.as_deref())?;
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
            db,
            opts.user_id,
            STEPS,
            &steps.to_string(),
            "count",
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

        // Realistic overnight sleep ending this morning (Watch-style stages).
        // No daytime nap samples — seed shows only a night window.
        report.readings += put_sleep_night(db, opts.user_id, d).await?;

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
            // Duration reading for browse.
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
        let channels: Vec<String> =
            sqlx::query_scalar("SELECT DISTINCT channel FROM readings WHERE user_id = 1")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(channels, vec![DEMO.to_string()]);
        let daytime: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM readings WHERE user_id = 1 AND type = ? AND substr(local_start, 12, 2) BETWEEN '10' AND '20'",
        )
        .bind(SLEEP)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(daytime, 0, "demo seed must not invent daytime sleep");
        let w: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workouts WHERE user_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(w, a.workouts as i64);
    }

    #[tokio::test]
    async fn seed_sleep_is_realistic_four_stage_night() {
        let db = crate::db::open_memory().await.unwrap();
        seed_demo(
            &db,
            SeedOpts {
                user_id: 1,
                days: 3,
                end: Some("2024-06-07".into()),
                rng_seed: 42,
            },
        )
        .await
        .unwrap();
        let s = crate::stats::sleep(&db, 1, "2024-06-07", 3).await.unwrap();
        assert!(!s.nights.is_empty());
        let n = s.nights.last().unwrap();
        assert!(n.core_min > 0.0, "core {n:?}");
        assert!(n.deep_min > 0.0, "deep {n:?}");
        assert!(n.rem_min > 0.0, "rem {n:?}");
        assert!(n.awake_min > 0.0, "awake {n:?}");
        assert_eq!(n.unspecified_min, 0.0);
        assert_eq!(
            n.asleep_min.round(),
            (n.rem_min + n.core_min + n.deep_min + n.unspecified_min).round()
        );
        assert!(!s.last_night.is_empty());
        for seg in &s.last_night {
            assert!(
                seg.end.starts_with("2024-06-07"),
                "segment must end on night date: {seg:?}"
            );
            let hour: u32 = seg.end[11..13].parse().unwrap();
            assert!(hour < 12, "daytime end on night date: {seg:?}");
        }
        let t0 = &s.last_night[0].start;
        assert!(
            t0.starts_with("2024-06-06 22:") || t0.starts_with("2024-06-06 23:"),
            "chart should open in the evening: {t0}"
        );
        let stages: std::collections::HashSet<_> =
            s.last_night.iter().map(|g| g.stage.as_str()).collect();
        for need in ["Awake", "AsleepREM", "AsleepCore", "AsleepDeep"] {
            assert!(stages.contains(need), "missing {need} in {stages:?}");
        }
    }

    #[tokio::test]
    /// Default end is today in the profile time zone (else process-local).
    async fn seed_default_end_is_local_today_not_past() {
        let db = crate::db::open_memory().await.unwrap();
        let today = crate::profile::today_in(None);
        let report = seed_demo(
            &db,
            SeedOpts {
                user_id: 1,
                days: 3,
                end: None,
                rng_seed: 7,
            },
        )
        .await
        .unwrap();
        assert_eq!(report.days, 3);
        let max_local: String = sqlx::query_scalar(
            "SELECT max(substr(local_end, 1, 10)) FROM readings WHERE user_id = 1",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        let max_goal: String =
            sqlx::query_scalar("SELECT max(date) FROM activity_goals WHERE user_id = 1")
                .fetch_one(&db)
                .await
                .unwrap();
        let today_s = today.format("%Y-%m-%d").to_string();
        assert_eq!(max_local, today_s);
        assert_eq!(max_goal, today_s);
        // No sample's local end date is after today (overnight sleep ends on `end`).
        let after: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM readings WHERE user_id = 1 AND substr(local_end, 1, 10) > ?",
        )
        .bind(&today_s)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(after, 0);
    }

    #[tokio::test]
    async fn seed_uses_profile_timezone() {
        let db = crate::db::open_memory().await.unwrap();
        crate::profile::put(
            &db,
            1,
            &crate::profile::Profile {
                timezone: Some("UTC".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let today = crate::profile::today_in(Some("UTC"));
        seed_demo(
            &db,
            SeedOpts {
                user_id: 1,
                days: 1,
                end: None,
                rng_seed: 1,
            },
        )
        .await
        .unwrap();
        let max_local: String = sqlx::query_scalar(
            "SELECT max(substr(local_end, 1, 10)) FROM readings WHERE user_id = 1",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(max_local, today.format("%Y-%m-%d").to_string());
    }
}
