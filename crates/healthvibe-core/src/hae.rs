//! Health Auto Export (iOS app) ingest: the "REST API" automation in JSON
//! format. Body: `{"data": {"metrics": [...], "workouts": [...]}}`.
//!
//! - Known metric names (snake_case, e.g. `step_count`) map to the same
//!   HealthKit types export.xml uses, converted to healthvibe's units.
//! - `heart_rate` `Avg` is stored as heart rate; `Min`/`Max` as
//!   `healthvibe.HeartRateMin` / `healthvibe.HeartRateMax`.
//! - `sleep_analysis` works unaggregated (one row per stage segment) and
//!   aggregated (per night totals laid out as consecutive stage segments).
//! - Unknown metrics are still stored, as `healthvibe.hae.<name>`.
//! - Re-sent readings replace the earlier copy (key: type, time, source),
//!   so a day total that grows during the day is updated, not doubled.

use crate::auth::sha256_hex;
use crate::shortcut::{self as sc, Kind};
use chrono::{DateTime, Duration, FixedOffset, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::BTreeMap;

pub const DEVICE: &str = "Health Auto Export";
pub const PATH: &str = "/api/v1/ingest/hae";
/// Header to use when an app can't send `Authorization`.
pub const TOKEN_HEADER: &str = "X-Healthvibe-Token";

/// Health Auto Export metric name → healthvibe metric key.
const MAP: &[(&str, &str)] = &[
    ("step_count", "stepCount"),
    ("walking_running_distance", "distanceWalkingRunning"),
    ("active_energy", "activeEnergyBurned"),
    ("basal_energy_burned", "basalEnergyBurned"),
    ("resting_energy", "basalEnergyBurned"),
    ("apple_exercise_time", "appleExerciseTime"),
    ("apple_stand_hour", "appleStandHour"),
    ("apple_stand_time", "appleStandTime"),
    ("flights_climbed", "flightsClimbed"),
    ("cycling_distance", "distanceCycling"),
    ("swimming_distance", "distanceSwimming"),
    ("heart_rate", "heartRate"),
    ("resting_heart_rate", "restingHeartRate"),
    ("walking_heart_rate", "walkingHeartRateAverage"),
    ("walking_heart_rate_average", "walkingHeartRateAverage"),
    ("heart_rate_variability", "heartRateVariabilitySDNN"),
    ("cardio_recovery", "heartRateRecoveryOneMinute"),
    ("vo2max", "vo2Max"),
    ("vo2_max", "vo2Max"),
    ("atrial_fibrillation_burden", "atrialFibrillationBurden"),
    ("sleep_analysis", "sleepAnalysis"),
    (
        "apple_sleeping_wrist_temperature",
        "appleSleepingWristTemperature",
    ),
    (
        "breathing_disturbances",
        "appleSleepingBreathingDisturbances",
    ),
    ("respiratory_rate", "respiratoryRate"),
    ("blood_oxygen_saturation", "oxygenSaturation"),
    ("weight_body_mass", "bodyMass"),
    ("body_fat_percentage", "bodyFatPercentage"),
    ("lean_body_mass", "leanBodyMass"),
    ("body_mass_index", "bodyMassIndex"),
    ("height", "height"),
    ("waist_circumference", "waistCircumference"),
    ("walking_speed", "walkingSpeed"),
    ("walking_step_length", "walkingStepLength"),
    ("walking_asymmetry_percentage", "walkingAsymmetryPercentage"),
    (
        "walking_double_support_percentage",
        "walkingDoubleSupportPercentage",
    ),
    ("walking_steadiness", "appleWalkingSteadiness"),
    ("apple_walking_steadiness", "appleWalkingSteadiness"),
    (
        "six_minute_walking_test_distance",
        "sixMinuteWalkTestDistance",
    ),
    ("stair_speed_up", "stairAscentSpeed"),
    ("environmental_audio", "environmentalAudioExposure"),
    ("environmental_audio_exposure", "environmentalAudioExposure"),
    ("headphone_audio", "headphoneAudioExposure"),
    ("headphone_audio_exposure", "headphoneAudioExposure"),
    ("time_in_daylight", "timeInDaylight"),
    ("mindful_minutes", "mindfulSession"),
    ("dietary_water", "dietaryWater"),
    ("caffeine", "dietaryCaffeine"),
    ("dietary_caffeine", "dietaryCaffeine"),
    ("running_power", "runningPower"),
    ("running_speed", "runningSpeed"),
    ("running_stride_length", "runningStrideLength"),
    ("running_vertical_oscillation", "runningVerticalOscillation"),
    ("running_ground_contact_time", "runningGroundContactTime"),
];

pub fn metric_for(name: &str) -> Option<&'static sc::Metric> {
    let n = name.trim().to_lowercase();
    MAP.iter()
        .find(|(h, _)| *h == n)
        .and_then(|(_, k)| sc::metric(k))
}

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct HaeResult {
    pub accepted: u64,
    pub updated: u64,
    pub unchanged: u64,
    pub invalid: u64,
    /// Per stored type: readings received.
    pub types: BTreeMap<String, u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Reading {
    pub kind: String,
    pub value: Option<String>,
    pub unit: Option<String>,
    pub start: DateTime<FixedOffset>,
    pub end: DateTime<FixedOffset>,
    pub source: Option<String>,
    /// Identity for replace-on-resend (hashed).
    pub key: String,
}

fn num(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        Value::Object(o) => num(o.get("qty")),
        _ => None,
    }
    .filter(|x: &f64| x.is_finite())
}

fn txt(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    }
}

fn ts(v: Option<&Value>) -> Option<DateTime<FixedOffset>> {
    txt(v).and_then(|t| sc::parse_ts(&t))
}

fn fmt(x: f64) -> String {
    format!("{}", (x * 1e6).round() / 1e6)
}

fn utc(t: DateTime<FixedOffset>) -> String {
    t.with_timezone(&Utc)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

fn reading(
    kind: String,
    value: Option<String>,
    unit: Option<String>,
    start: DateTime<FixedOffset>,
    end: DateTime<FixedOffset>,
    source: Option<String>,
) -> Reading {
    let key = format!(
        "{kind}\x1f{}\x1f{}\x1f{}",
        utc(start),
        utc(end),
        source.as_deref().unwrap_or("")
    );
    Reading {
        kind,
        value,
        unit,
        start,
        end,
        source,
        key,
    }
}

fn quantity(m: &sc::Metric, x: f64, units: Option<&str>) -> String {
    let mut x = match units {
        Some(u) => sc::convert(x, u, m.unit),
        None => x,
    };
    if m.unit == "%" && m.hk.starts_with("HKQuantityTypeIdentifier") && x > 1.0 {
        x /= 100.0;
    }
    fmt(x)
}

/// Start/end of one data point: `date` (aggregated bucket or sample time)
/// or `startDate`/`endDate`.
fn span(
    o: &serde_json::Map<String, Value>,
) -> Option<(DateTime<FixedOffset>, DateTime<FixedOffset>)> {
    let start = ts(o.get("startDate")).or_else(|| ts(o.get("date")))?;
    let end = ts(o.get("endDate")).unwrap_or(start);
    (end >= start).then_some((start, end))
}

fn sleep_stage(v: &str) -> Option<String> {
    let m = sc::metric("sleepAnalysis")?;
    sc::category_value(m, &Value::String(v.into()))
}

fn sleep(
    o: &serde_json::Map<String, Value>,
    src: Option<String>,
    out: &mut Vec<Reading>,
) -> Result<(), String> {
    let hk = "HKCategoryTypeIdentifierSleepAnalysis".to_string();
    if let Some(v) = txt(o.get("value")) {
        // Unaggregated: one segment.
        let (s, e) = span(o).ok_or("sleep segment: bad startDate/endDate")?;
        let stage = sleep_stage(&v).ok_or_else(|| format!("unknown sleep stage {v:?}"))?;
        out.push(reading(hk, Some(stage), None, s, e, src));
        return Ok(());
    }
    // Aggregated: per-night totals in hours. Lay the stages out back to
    // back from sleepStart so the totals match; the times are approximate.
    let start = ts(o.get("sleepStart")).ok_or("aggregated sleep: missing sleepStart")?;
    let end = ts(o.get("sleepEnd")).unwrap_or(start);
    let night = txt(o.get("date")).unwrap_or_else(|| start.format("%Y-%m-%d").to_string());
    let h = |k: &str| num(o.get(k)).unwrap_or(0.0).max(0.0);
    let (core, deep, rem) = (h("core"), h("deep"), h("rem"));
    let total = if h("totalSleep") > 0.0 {
        h("totalSleep")
    } else {
        h("asleep")
    };
    let unspec = (total - core - deep - rem).max(0.0);
    let awake = (((end - start).num_seconds() as f64) / 3600.0 - total).max(0.0);
    let mut t = start;
    let key = |stage: &str| {
        format!(
            "{hk}\x1faggregated\x1f{night}\x1f{stage}\x1f{}",
            src.as_deref().unwrap_or("")
        )
    };
    for (stage, hours) in [
        ("Core", core),
        ("Deep", deep),
        ("REM", rem),
        ("Asleep", unspec),
        ("Awake", awake),
    ] {
        if hours < 1.0 / 120.0 {
            continue;
        }
        let e = t + Duration::seconds((hours * 3600.0).round() as i64);
        let mut r = reading(hk.clone(), sleep_stage(stage), None, t, e, src.clone());
        r.key = key(stage);
        out.push(r);
        t = e;
    }
    if let (Some(s), Some(e)) = (ts(o.get("inBedStart")), ts(o.get("inBedEnd"))) {
        if e > s && h("inBed") > 0.0 {
            let mut r = reading(hk.clone(), sleep_stage("In Bed"), None, s, e, src.clone());
            r.key = key("InBed");
            out.push(r);
        }
    }
    Ok(())
}

fn metric_points(m: &Value, out: &mut Vec<Reading>, errs: &mut Vec<String>) {
    let name = txt(m.get("name")).unwrap_or_default();
    let units = txt(m.get("units"));
    let Some(Value::Array(data)) = m.get("data") else {
        return;
    };
    let known = metric_for(&name);
    for p in data {
        let Some(o) = p.as_object() else {
            errs.push(format!("{name}: data point is not an object"));
            continue;
        };
        let src = txt(o.get("source"));
        let r: Result<(), String> = (|| {
            if name == "sleep_analysis" {
                return sleep(o, src.clone(), out);
            }
            let (s, e) = span(o).ok_or("bad or missing date")?;
            if name == "blood_pressure" {
                for (f, t) in [("systolic", "Systolic"), ("diastolic", "Diastolic")] {
                    if let Some(x) = num(o.get(f)) {
                        out.push(reading(
                            format!("HKQuantityTypeIdentifierBloodPressure{t}"),
                            Some(fmt(x)),
                            Some("mmHg".into()),
                            s,
                            e,
                            src.clone(),
                        ));
                    }
                }
                return Ok(());
            }
            match known {
                Some(k) if k.key == "heartRate" => {
                    let avg = num(o.get("Avg"))
                        .or_else(|| num(o.get("qty")))
                        .ok_or("heart_rate: no Avg/qty")?;
                    out.push(reading(
                        k.hk.into(),
                        Some(quantity(k, avg, None)),
                        Some(k.unit.into()),
                        s,
                        e,
                        src.clone(),
                    ));
                    for (f, t) in [
                        ("Min", "healthvibe.HeartRateMin"),
                        ("Max", "healthvibe.HeartRateMax"),
                    ] {
                        if let Some(x) = num(o.get(f)) {
                            out.push(reading(
                                t.into(),
                                Some(fmt(x)),
                                Some("count/min".into()),
                                s,
                                e,
                                src.clone(),
                            ));
                        }
                    }
                }
                Some(k) if k.key == "mindfulSession" => {
                    let mins = num(o.get("qty")).unwrap_or(0.0);
                    let e2 = if e > s {
                        e
                    } else {
                        s + Duration::seconds((mins * 60.0) as i64)
                    };
                    out.push(reading(
                        k.hk.into(),
                        Some("HKCategoryValueNotApplicable".into()),
                        None,
                        s,
                        e2,
                        src.clone(),
                    ));
                }
                Some(k) if k.key == "appleStandHour" && num(o.get("qty")).unwrap_or(0.0) <= 1.0 => {
                    let stood = num(o.get("qty")).unwrap_or(0.0) > 0.0;
                    let v = if stood {
                        "HKCategoryValueAppleStandHourStood"
                    } else {
                        "HKCategoryValueAppleStandHourIdle"
                    };
                    let e2 = if e > s { e } else { s + Duration::hours(1) };
                    out.push(reading(
                        k.hk.into(),
                        Some(v.into()),
                        None,
                        s,
                        e2,
                        src.clone(),
                    ));
                }
                Some(k) if k.kind == Kind::Quantity => {
                    let x = num(o.get("qty")).ok_or("no qty")?;
                    out.push(reading(
                        k.hk.into(),
                        Some(quantity(k, x, units.as_deref())),
                        Some(k.unit.into()),
                        s,
                        e,
                        src.clone(),
                    ));
                }
                _ => {
                    // Not mapped yet: keep every number as sent.
                    let kind = format!(
                        "healthvibe.hae.{}",
                        name.replace(|c: char| !c.is_ascii_alphanumeric() && c != '_', "")
                    );
                    let mut any = false;
                    for (f, v) in o {
                        if let Some(x) = num(Some(v))
                            .filter(|_| !matches!(f.as_str(), "date" | "startDate" | "endDate"))
                        {
                            let kind = if f == "qty" {
                                kind.clone()
                            } else {
                                format!("{kind}.{f}")
                            };
                            out.push(reading(
                                kind,
                                Some(fmt(x)),
                                units.clone(),
                                s,
                                e,
                                src.clone(),
                            ));
                            any = true;
                        }
                    }
                    if !any {
                        return Err("no numeric value".into());
                    }
                }
            }
            Ok(())
        })();
        if let Err(e) = r {
            errs.push(format!("{name}: {e}"));
        }
    }
}

fn workout_type(name: &str) -> String {
    let n = name.trim();
    let base = match n.to_lowercase().as_str() {
        "outdoor walk" | "indoor walk" | "walk" | "walking" => "Walking".to_string(),
        "outdoor run" | "indoor run" | "run" | "running" => "Running".into(),
        "outdoor cycle" | "indoor cycle" | "cycling" | "biking" => "Cycling".into(),
        "pool swim" | "open water swim" | "swimming" => "Swimming".into(),
        "hiking" | "hike" => "Hiking".into(),
        _ => n
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(|w| {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().chain(c).collect::<String>())
                    .unwrap_or_default()
            })
            .collect(),
    };
    format!(
        "HKWorkoutActivityType{}",
        if base.is_empty() {
            "Other".into()
        } else {
            base
        }
    )
}

fn workout(w: &Value) -> Result<Reading, String> {
    let o = w.as_object().ok_or("workout is not an object")?;
    let s = ts(o.get("start")).ok_or("workout: bad start")?;
    let e = ts(o.get("end")).ok_or("workout: bad end")?;
    let secs = num(o.get("duration")).unwrap_or_else(|| (e - s).num_seconds() as f64);
    let mut r = reading(
        workout_type(&txt(o.get("name")).unwrap_or_default()),
        Some(fmt((secs / 60.0 * 1000.0).round() / 1000.0)),
        Some("min".into()),
        s,
        e,
        txt(o.get("source")),
    );
    if let Some(id) = txt(o.get("id")) {
        r.key = format!("workout\x1f{id}");
    }
    Ok(r)
}

/// Parse a Health Auto Export JSON body into readings (no database).
pub fn parse(body: &[u8]) -> Result<(Vec<Reading>, Vec<String>), String> {
    let v: Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    let d = v.get("data").unwrap_or(&v);
    if !d.is_object() {
        return Err("expected {\"data\": {...}}".into());
    }
    let (mut out, mut errs) = (vec![], vec![]);
    if let Some(Value::Array(ms)) = d.get("metrics") {
        for m in ms {
            metric_points(m, &mut out, &mut errs);
        }
    }
    if let Some(Value::Array(ws)) = d.get("workouts") {
        for w in ws {
            match workout(w) {
                Ok(r) => out.push(r),
                Err(e) => errs.push(e),
            }
        }
    }
    Ok((out, errs))
}

const CHUNK: usize = 5000;

/// Parse and store. Err = body isn't JSON of the right shape.
pub async fn ingest(
    db: &SqlitePool,
    user_id: i64,
    body: &[u8],
) -> anyhow::Result<Result<HaeResult, String>> {
    let (rows, errs) = match parse(body) {
        Ok(x) => x,
        Err(e) => return Ok(Err(e)),
    };
    let mut res = HaeResult {
        invalid: errs.len() as u64,
        ..Default::default()
    };
    res.errors = errs.into_iter().take(5).collect();
    // Commit in chunks so a huge all-metrics batch doesn't hold the write lock
    // for the whole request; re-sends are idempotent, so a partial failure is
    // repaired by HAE retrying.
    let mut tx = db.begin().await?;
    for (i, r) in rows.iter().enumerate() {
        if i > 0 && i % CHUNK == 0 {
            tx.commit().await?;
            tx = db.begin().await?;
        }
        let hash = sha256_hex(&format!("hae\x1f{}", r.key));
        let old: Option<(Option<String>, String, String)> = sqlx::query_as(
            "SELECT value, start_utc, end_utc FROM readings WHERE user_id = ? AND hash = ?",
        )
        .bind(user_id)
        .bind(&hash)
        .fetch_optional(&mut *tx)
        .await?;
        *res.types.entry(r.kind.clone()).or_default() += 1;
        match old {
            Some((v, s, e)) if v == r.value && s == utc(r.start) && e == utc(r.end) => {
                res.unchanged += 1;
                continue;
            }
            Some(_) => res.updated += 1,
            None => res.accepted += 1,
        }
        sqlx::query(
            "INSERT INTO readings
             (user_id, type, value, unit, start_utc, end_utc, tz_offset, source, hash,
              local_start, local_end)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (user_id, hash) DO UPDATE SET
               value = excluded.value, unit = excluded.unit,
               start_utc = excluded.start_utc, end_utc = excluded.end_utc,
               tz_offset = excluded.tz_offset, local_start = excluded.local_start,
               local_end = excluded.local_end",
        )
        .bind(user_id)
        .bind(&r.kind)
        .bind(&r.value)
        .bind(&r.unit)
        .bind(utc(r.start))
        .bind(utc(r.end))
        .bind(r.start.format("%z").to_string())
        .bind(&r.source)
        .bind(&hash)
        .bind(r.start.format("%Y-%m-%d %H:%M:%S").to_string())
        .bind(r.end.format("%Y-%m-%d %H:%M:%S").to_string())
        .execute(&mut *tx)
        .await?;
    }
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    sqlx::query(
        "UPDATE upload_tokens SET last_upload_at = ?, last_seen_at = ?,
           hae_received = hae_received + ? WHERE user_id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(rows.len() as i64)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Ok(res))
}

/// Readings received from Health Auto Export with the current token.
pub async fn received(db: &SqlitePool, user_id: i64) -> anyhow::Result<Option<i64>> {
    Ok(
        sqlx::query_scalar("SELECT hae_received FROM upload_tokens WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(db)
            .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_fixture() {
        let body = include_bytes!("../tests/fixtures/hae.json");
        let (rows, errs) = parse(body).unwrap();
        assert!(errs.is_empty(), "{errs:?}");
        let get = |k: &str| rows.iter().filter(|r| r.kind == k).collect::<Vec<_>>();
        assert_eq!(get("HKQuantityTypeIdentifierStepCount").len(), 2);
        let w = get("HKQuantityTypeIdentifierBodyMass");
        assert_eq!(w[0].value.as_deref(), Some("81.646627")); // 180 lb
        assert_eq!(
            get("HKQuantityTypeIdentifierHeartRate")[0].value.as_deref(),
            Some("72")
        );
        assert_eq!(
            get("healthvibe.HeartRateMax")[0].value.as_deref(),
            Some("85")
        );
        assert_eq!(
            get("HKQuantityTypeIdentifierOxygenSaturation")[0]
                .value
                .as_deref(),
            Some("0.97")
        );
        let sleep = get("HKCategoryTypeIdentifierSleepAnalysis");
        assert!(sleep
            .iter()
            .any(|r| r.value.as_deref() == Some("HKCategoryValueSleepAnalysisAsleepDeep")));
        assert!(sleep
            .iter()
            .any(|r| r.value.as_deref() == Some("HKCategoryValueSleepAnalysisInBed")));
        assert_eq!(
            get("HKWorkoutActivityTypeRunning")[0].value.as_deref(),
            Some("30")
        );
        assert_eq!(
            get("HKQuantityTypeIdentifierBloodPressureSystolic").len(),
            1
        );
        assert_eq!(get("healthvibe.hae.toothbrushing").len(), 1);
    }

    #[test]
    fn aggregated_sleep_totals() {
        let body = br#"{"data":{"metrics":[{"name":"sleep_analysis","units":"hr","data":[{"date":"2026-10-06","totalSleep":7.5,"asleep":7.5,"core":4,"deep":1.5,"rem":2,"sleepStart":"2026-10-05 23:00:00 -0400","sleepEnd":"2026-10-06 07:00:00 -0400","inBed":8.25,"inBedStart":"2026-10-05 22:45:00 -0400","inBedEnd":"2026-10-06 07:00:00 -0400","source":"Synthetic Watch"}]}]}}"#;
        let (rows, _) = parse(body).unwrap();
        let mins = |v: &str| {
            rows.iter()
                .filter(|r| r.value.as_deref() == Some(v))
                .map(|r| (r.end - r.start).num_minutes())
                .sum::<i64>()
        };
        assert_eq!(mins("HKCategoryValueSleepAnalysisAsleepCore"), 240);
        assert_eq!(mins("HKCategoryValueSleepAnalysisAsleepDeep"), 90);
        assert_eq!(mins("HKCategoryValueSleepAnalysisAsleepREM"), 120);
        assert_eq!(mins("HKCategoryValueSleepAnalysisAwake"), 30);
        assert_eq!(mins("HKCategoryValueSleepAnalysisInBed"), 495);
    }
}
