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
const SLEEP_CORE: &str = "HKCategoryValueSleepAnalysisAsleepCore";
const SLEEP_DEEP: &str = "HKCategoryValueSleepAnalysisAsleepDeep";
const SLEEP_REM: &str = "HKCategoryValueSleepAnalysisAsleepREM";
const SLEEP_AWAKE: &str = "HKCategoryValueSleepAnalysisAwake";
const WEIGHT: &str = "HKQuantityTypeIdentifierBodyMass";
const RESPIRATORY: &str = "HKQuantityTypeIdentifierRespiratoryRate";
const WRIST_TEMP: &str = "HKQuantityTypeIdentifierAppleSleepingWristTemperature";

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

/// Split `total` into `parts` integers that sum to `total`, each at least
/// `min_each` when the budget allows. Keeps demo stages from collapsing to a minute.
fn split_positive(total: i32, parts: usize, min_each: i32, rng: &mut StdRng) -> Vec<i32> {
    let floor = if parts > 0 && total >= min_each * parts as i32 {
        min_each
    } else {
        1
    };
    let mut out = vec![floor; parts];
    let mut left = total - floor * parts as i32;
    while left > 0 && parts > 0 {
        let i = rng.gen_range(0..parts);
        let give = rng.gen_range(1..=left.min(40));
        out[i] += give;
        left -= give;
    }
    out
}

/// (day offset from the wake date, hour, minute). Negative minutes are the previous evening.
fn wall(mins_from_midnight: i32) -> (i64, u32, u32) {
    let (delta, m) = if mins_from_midnight < 0 {
        (-1, mins_from_midnight + 24 * 60)
    } else {
        (0, mins_from_midnight)
    };
    (delta, (m / 60) as u32, (m % 60) as u32)
}

fn assign_stage(durs: &mut [i32], pattern: &[&str], stage: &str, budget: i32, rng: &mut StdRng) {
    let idx: Vec<usize> = pattern
        .iter()
        .enumerate()
        .filter(|(_, s)| **s == stage)
        .map(|(i, _)| i)
        .collect();
    let n = idx.len();
    let min_each = if stage == SLEEP_AWAKE { 4 } else { 12 };
    for (i, part) in idx
        .into_iter()
        .zip(split_positive(budget, n, min_each, rng))
    {
        durs[i] = part;
    }
}

/// Awake / REM / Core / Deep minutes that sum to `remain` (the post-midnight window).
fn stage_budgets(remain: i32, rng: &mut StdRng) -> (i32, i32, i32, i32) {
    let awake = rng.gen_range(8..28i32);
    let deep = rng.gen_range(22..70i32);
    let mut rem = rng.gen_range(36..90i32);
    let mut core = remain - awake - deep - rem;
    if core < 40 {
        let take = (40 - core).min((rem - 30).max(0));
        rem -= take;
        core += take;
    }
    (awake, deep, rem, core)
}

/// One varied Watch-style night. Minutes are relative to midnight of the wake date
/// (negative = previous evening). The first stage crosses midnight, so every
/// sample's end is on the wake date. Segments are contiguous and use only
/// Awake / REM / Core / Deep.
fn demo_sleep_segments(rng: &mut StdRng) -> Vec<(&'static str, i32, i32)> {
    let bed = 22 * 60 + rng.gen_range(0..110i32) - 24 * 60;
    let cross_end = rng.gen_range(20..95i32);
    let wake = rng.gen_range(5 * 60 + 35..8 * 60 + 5);
    let (awake, deep, rem, core) = stage_budgets(wake - cross_end, rng);
    let pattern: &[&str] = match rng.gen_range(0..4i32) {
        0 => &[
            SLEEP_DEEP,
            SLEEP_CORE,
            SLEEP_REM,
            SLEEP_CORE,
            SLEEP_AWAKE,
            SLEEP_REM,
            SLEEP_CORE,
            SLEEP_AWAKE,
        ],
        1 => &[
            SLEEP_REM,
            SLEEP_CORE,
            SLEEP_DEEP,
            SLEEP_CORE,
            SLEEP_AWAKE,
            SLEEP_CORE,
            SLEEP_REM,
            SLEEP_AWAKE,
        ],
        2 => &[
            SLEEP_CORE,
            SLEEP_DEEP,
            SLEEP_REM,
            SLEEP_AWAKE,
            SLEEP_CORE,
            SLEEP_DEEP,
            SLEEP_REM,
            SLEEP_CORE,
            SLEEP_AWAKE,
        ],
        _ => &[
            SLEEP_DEEP,
            SLEEP_CORE,
            SLEEP_AWAKE,
            SLEEP_REM,
            SLEEP_CORE,
            SLEEP_DEEP,
            SLEEP_CORE,
            SLEEP_REM,
            SLEEP_AWAKE,
        ],
    };
    let mut durs = vec![0; pattern.len()];
    assign_stage(&mut durs, pattern, SLEEP_CORE, core, rng);
    assign_stage(&mut durs, pattern, SLEEP_DEEP, deep, rng);
    assign_stage(&mut durs, pattern, SLEEP_REM, rem, rng);
    assign_stage(&mut durs, pattern, SLEEP_AWAKE, awake, rng);
    let mut out = Vec::with_capacity(pattern.len() + 1);
    // Opening Core crosses midnight so the evening is attributed to the wake date.
    out.push((SLEEP_CORE, bed, cross_end));
    let mut t = cross_end;
    for (stage, dur) in pattern.iter().zip(durs) {
        out.push((*stage, t, t + dur));
        t += dur;
    }
    out
}

/// One varied night ending the morning of `end_day`. Every segment's `local_end`
/// is on `end_day` (no daytime samples). Stage totals are the segment durations.
/// Returns rows inserted.
async fn put_sleep_night(
    db: &SqlitePool,
    user_id: i64,
    end_day: NaiveDate,
    rng: &mut StdRng,
) -> anyhow::Result<u64> {
    let segs = demo_sleep_segments(rng);
    for (stage, start_min, end_min) in &segs {
        let (d0, sh, sm) = wall(*start_min);
        let (_d1, eh, em) = wall(*end_min);
        let day = end_day + Duration::days(d0);
        put_reading(db, user_id, SLEEP, stage, "", day, sh, sm, eh, em).await?;
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
        // Daytime HR sample (activity).
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

        // Varied overnight sleep ending this morning (Watch-style stages).
        // No daytime nap samples — each night is one evening→morning window
        // whose samples all end on this local date.
        report.readings += put_sleep_night(db, opts.user_id, d, &mut rng).await?;

        // Overnight vitals overlapping the sleep window (for Comparisons).
        let prev = d - Duration::days(1);
        for (h, m) in [
            (22, 30),
            (23, 0),
            (23, 30),
            (0, 15),
            (1, 0),
            (1, 45),
            (2, 30),
            (3, 15),
            (4, 0),
            (4, 45),
            (5, 30),
            (6, 15),
            (6, 50),
        ] {
            let day = if h >= 22 { prev } else { d };
            let bpm = 52.0 + rng.gen_range(0.0..20.0);
            put_reading(
                db,
                opts.user_id,
                HR,
                &format!("{bpm:.0}"),
                "count/min",
                day,
                h,
                m,
                h,
                m.saturating_add(1).min(59),
            )
            .await?;
            report.readings += 1;
            if m % 30 == 0 {
                let rr = 10.0 + rng.gen_range(0.0..8.5);
                put_reading(
                    db,
                    opts.user_id,
                    RESPIRATORY,
                    &format!("{rr:.1}"),
                    "count/min",
                    day,
                    h,
                    m,
                    h,
                    m.saturating_add(1).min(59),
                )
                .await?;
                report.readings += 1;
            }
        }
        // Absolute wrist temperature (°C) once near wake — no "from baseline".
        put_reading(
            db,
            opts.user_id,
            WRIST_TEMP,
            &format!("{:.2}", 36.2 + rng.gen_range(-0.3..0.4)),
            "degC",
            d,
            6,
            40,
            6,
            41,
        )
        .await?;
        report.readings += 1;

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

        // Varied workout types for Sessions chips (every other day).
        if d.ordinal().is_multiple_of(2) {
            let kind = match d.ordinal() % 8 {
                0 => (
                    "HKWorkoutActivityTypeRunning",
                    "Running",
                    35.0,
                    Some(5200.0),
                    Some(300.0),
                ),
                2 => (
                    "HKWorkoutActivityTypeWalking",
                    "Walking",
                    42.0,
                    Some(3200.0),
                    Some(140.0),
                ),
                4 => (
                    "HKWorkoutActivityTypeTraditionalStrengthTraining",
                    "Strength",
                    28.0,
                    None,
                    Some(180.0),
                ),
                _ => ("HKWorkoutActivityTypeYoga", "Yoga", 40.0, None, Some(80.0)),
            };
            let (atype, _label, mins, dist, kcal) = kind;
            let eh = 17 + (mins as u32) / 60;
            let em = (mins as u32) % 60;
            let start_utc = format!("{d}T17:00:00Z");
            let end_utc = format!("{d}T{eh:02}:{em:02}:00Z");
            let local_start = format!("{d} 17:00:00");
            let local_end = format!("{d} {eh:02}:{em:02}:00");
            let w = crate::workouts::Workout {
                activity_type: atype.into(),
                channel: DEMO.into(),
                source: SOURCE.into(),
                start_utc: start_utc.clone(),
                end_utc: end_utc.clone(),
                local_start: Some(local_start),
                local_end: Some(local_end),
                duration_min: Some(mins),
                avg_hr: Some(120.0 + rng.gen_range(0.0..40.0)),
                max_hr: Some(145.0 + rng.gen_range(0.0..35.0)),
                active_kcal: kcal,
                total_kcal: kcal.map(|k| k + 20.0),
                distance_m: dist,
                elevation_m: if dist.is_some() { Some(30.0) } else { None },
                detail_json: Some(r#"{"demo":true}"#.into()),
                external_id: Some(format!(
                    "demo-{}-{d}",
                    atype.rsplit("Type").next().unwrap_or("w")
                )),
                id: None,
            };
            crate::workouts::upsert(db, opts.user_id, &w).await?;
            put_reading(
                db,
                opts.user_id,
                atype,
                &format!("{mins:.0}"),
                "min",
                d,
                17,
                0,
                eh,
                em,
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
    async fn seed_sleep_nights_vary_and_stages_match() {
        let db = crate::db::open_memory().await.unwrap();
        seed_demo(
            &db,
            SeedOpts {
                user_id: 1,
                days: 7,
                end: Some("2024-06-07".into()),
                rng_seed: 42,
            },
        )
        .await
        .unwrap();
        let s = crate::stats::sleep(&db, 1, "2024-06-07", 7).await.unwrap();
        assert_eq!(s.nights.len(), 7, "one night per local end date");
        let parse =
            |t: &str| chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M:%S").unwrap();
        let mut asleep = Vec::new();
        for n in &s.nights {
            assert!(n.core_min > 0.0, "core {n:?}");
            assert!(n.deep_min > 0.0, "deep {n:?}");
            assert!(n.rem_min > 0.0, "rem {n:?}");
            assert!(n.awake_min > 0.0, "awake {n:?}");
            assert_eq!(n.unspecified_min, 0.0);
            assert_eq!(n.in_bed_min, 0.0);
            assert_eq!(
                n.asleep_min.round(),
                (n.rem_min + n.core_min + n.deep_min + n.unspecified_min).round()
            );
            let prev = chrono::NaiveDate::parse_from_str(&n.date, "%Y-%m-%d").unwrap()
                - chrono::Duration::days(1);
            assert!(
                n.segments[0]
                    .start
                    .starts_with(&prev.format("%Y-%m-%d").to_string()),
                "chart opens the previous evening: {}",
                n.segments[0].start
            );
            let open_h: u32 = n.segments[0].start[11..13].parse().unwrap();
            assert!(
                (22..24).contains(&open_h),
                "bedtime {}",
                n.segments[0].start
            );
            let mut by = std::collections::HashMap::<&str, i64>::new();
            for w in n.segments.windows(2) {
                assert_eq!(w[0].end, w[1].start, "stages must be contiguous");
            }
            for seg in &n.segments {
                assert!(
                    seg.end.starts_with(&n.date),
                    "segment must end on night date: {seg:?}"
                );
                let hour: u32 = seg.end[11..13].parse().unwrap();
                assert!(hour < 12, "daytime end on night date: {seg:?}");
                let mins = (parse(&seg.end) - parse(&seg.start)).num_minutes();
                assert!(mins > 0, "{seg:?}");
                *by.entry(seg.stage.as_str()).or_default() += mins;
            }
            assert!((by["Awake"] as f64 - n.awake_min).abs() < 0.6);
            assert!((by["AsleepREM"] as f64 - n.rem_min).abs() < 0.6);
            assert!((by["AsleepCore"] as f64 - n.core_min).abs() < 0.6);
            assert!((by["AsleepDeep"] as f64 - n.deep_min).abs() < 0.6);
            asleep.push(n.asleep_min.round() as i64);
        }
        let (lo, hi) = (asleep.iter().min().unwrap(), asleep.iter().max().unwrap());
        assert!(
            hi - lo >= 20,
            "demo nights should differ, asleep minutes {asleep:?}"
        );
        assert_eq!(s.last_night, s.nights.last().unwrap().segments);
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
