//! Workout rows (HR, energy, distance, detail JSON).
//!
//! Duration readings for browse stay in `readings`; this table holds the
//! extras. Re-sends upsert on
//! `(user_id, channel, source, start_utc, end_utc, activity_type)`.
//!
//! Only values the source provides are stored (unit-normalised). Nothing is
//! computed from samples: no HR zones, and avg/max HR are NULL unless the
//! source states them.

use chrono::{DateTime, FixedOffset, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Workout {
    pub id: Option<i64>,
    pub activity_type: String,
    pub channel: String,
    pub source: String,
    pub start_utc: String,
    pub end_utc: String,
    pub local_start: Option<String>,
    pub local_end: Option<String>,
    pub duration_min: Option<f64>,
    pub avg_hr: Option<f64>,
    pub max_hr: Option<f64>,
    pub active_kcal: Option<f64>,
    pub total_kcal: Option<f64>,
    pub distance_m: Option<f64>,
    pub elevation_m: Option<f64>,
    pub detail_json: Option<String>,
    pub external_id: Option<String>,
}

fn utc(t: DateTime<FixedOffset>) -> String {
    t.with_timezone(&Utc)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

fn local(t: DateTime<FixedOffset>) -> String {
    t.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn num(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s
            .trim()
            .chars()
            .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
            .collect::<String>()
            .parse()
            .ok(),
        Value::Object(o) => num(o.get("qty")).or_else(|| num(o.get("value"))),
        _ => None,
    }
    .filter(|x| x.is_finite())
}

fn txt(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn to_meters(x: f64, unit: &str) -> f64 {
    match unit.trim().to_lowercase().as_str() {
        "km" | "kilometer" | "kilometers" => x * 1000.0,
        "mi" | "mile" | "miles" => x * 1609.344,
        "ft" | "feet" => x * 0.3048,
        "yd" | "yard" | "yards" => x * 0.9144,
        "cm" => x / 100.0,
        _ => x, // assume metres
    }
}

fn qty_unit(v: Option<&Value>) -> Option<(f64, String)> {
    match v? {
        Value::Object(o) => {
            let x = num(o.get("qty")).or_else(|| num(o.get("value")))?;
            Some((x, txt(o.get("units")).unwrap_or_default()))
        }
        Value::Number(n) => Some((n.as_f64()?, String::new())),
        Value::String(s) => Some((s.parse().ok()?, String::new())),
        _ => None,
    }
}

/// Build a rich workout from a Health Auto Export workout object.
pub fn from_hae(w: &Value, channel: &str) -> Result<Workout, String> {
    let o = w.as_object().ok_or("workout is not an object")?;
    let start = crate::shortcut::parse_ts(&txt(o.get("start")).ok_or("missing start")?)
        .ok_or("bad start")?;
    let end = crate::shortcut::parse_ts(&txt(o.get("end")).ok_or("missing end")?)
        .ok_or("bad end")?;
    if end < start {
        return Err("end before start".into());
    }
    let name = txt(o.get("name")).unwrap_or_else(|| "Other".into());
    // Reuse HAE name → HK type mapping via a thin parse of duration reading.
    let activity = {
        // Duplicate of hae::workout_type without making it pub — keep local.
        let n = name.trim();
        let base = match n.to_lowercase().as_str() {
            "outdoor walk" | "indoor walk" | "walk" | "walking" => "Walking",
            "outdoor run" | "indoor run" | "run" | "running" => "Running",
            "outdoor cycle" | "indoor cycle" | "cycling" | "biking" => "Cycling",
            "pool swim" | "open water swim" | "swimming" => "Swimming",
            "hiking" | "hike" => "Hiking",
            _ => "",
        };
        let base = if base.is_empty() {
            let pascal: String = n
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| !w.is_empty())
                .map(|w| {
                    let mut c = w.chars();
                    c.next()
                        .map(|f| f.to_uppercase().chain(c).collect::<String>())
                        .unwrap_or_default()
                })
                .collect();
            if pascal.is_empty() {
                "Other".into()
            } else {
                pascal
            }
        } else {
            base.into()
        };
        format!("HKWorkoutActivityType{base}")
    };
    let secs = num(o.get("duration")).unwrap_or_else(|| (end - start).num_seconds() as f64);
    let source = txt(o.get("source")).unwrap_or_default();

    let active_kcal = qty_unit(o.get("activeEnergyBurned"))
        .or_else(|| qty_unit(o.get("activeEnergy")))
        .map(|(x, _)| x);
    let total_kcal = qty_unit(o.get("totalEnergyBurned"))
        .or_else(|| qty_unit(o.get("totalEnergy")))
        .map(|(x, _)| x);
    let distance_m = qty_unit(o.get("distance"))
        .or_else(|| qty_unit(o.get("totalDistance")))
        .map(|(x, u)| to_meters(x, &u));
    let elevation_m = qty_unit(o.get("elevationAscended"))
        .or_else(|| qty_unit(o.get("elevationGain")))
        .map(|(x, u)| to_meters(x, if u.is_empty() { "m" } else { &u }));

    let avg_hr = num(o.get("avgHeartRate"))
        .or_else(|| num(o.get("averageHeartRate")))
        .or_else(|| {
            o.get("heartRate")
                .and_then(|h| h.as_object())
                .and_then(|h| num(h.get("avg")).or_else(|| num(h.get("Avg"))))
        });
    let max_hr = num(o.get("maxHeartRate"))
        .or_else(|| num(o.get("maximumHeartRate")))
        .or_else(|| {
            o.get("heartRate")
                .and_then(|h| h.as_object())
                .and_then(|h| num(h.get("max")).or_else(|| num(h.get("Max"))))
        });

    let mut detail = json!({});
    if let Some(obj) = detail.as_object_mut() {
        // Kept verbatim, as the source sent them.
        for k in [
            "route",
            "metadata",
            "laps",
            "splits",
            "stepCadence",
            "speed",
            "heartRate",
            "heartRateData",
            "heartRateSamples",
            "heartRateRecovery",
        ] {
            if let Some(v) = o.get(k) {
                obj.insert(k.into(), v.clone());
            }
        }
    }
    let detail_json = detail
        .as_object()
        .filter(|o| !o.is_empty())
        .map(|o| serde_json::to_string(o).unwrap_or_else(|_| "{}".into()));

    Ok(Workout {
        id: None,
        activity_type: activity,
        channel: channel.into(),
        source,
        start_utc: utc(start),
        end_utc: utc(end),
        local_start: Some(local(start)),
        local_end: Some(local(end)),
        duration_min: Some((secs / 60.0 * 1000.0).round() / 1000.0),
        avg_hr,
        max_hr,
        active_kcal,
        total_kcal,
        distance_m,
        elevation_m,
        detail_json,
        external_id: txt(o.get("id")),
    })
}

/// Children of an export.xml `<Workout>` element, kept as Apple wrote them
/// (attribute maps of raw strings). Only values Apple provides directly are
/// copied into columns; nothing is recomputed from them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExportChildren {
    /// `<WorkoutStatistics>` attributes (type, startDate, endDate, average,
    /// minimum, maximum, sum, unit).
    pub statistics: Vec<BTreeMap<String, String>>,
    /// `<WorkoutEvent>` attributes (type, date, duration, durationUnit).
    pub events: Vec<BTreeMap<String, String>>,
    /// Direct `<MetadataEntry key value>` children of the workout.
    pub metadata: Vec<(String, String)>,
    /// `<WorkoutRoute><FileReference path>` paths (the GPX files themselves
    /// are not imported).
    pub route_files: Vec<String>,
}

impl ExportChildren {
    fn is_empty(&self) -> bool {
        self.statistics.is_empty()
            && self.events.is_empty()
            && self.metadata.is_empty()
            && self.route_files.is_empty()
    }

    /// First statistic of exactly `HKQuantityTypeIdentifier{name}`.
    fn stat(&self, name: &str) -> Option<&BTreeMap<String, String>> {
        let want = format!("HKQuantityTypeIdentifier{name}");
        self.statistics
            .iter()
            .find(|s| s.get("type").is_some_and(|t| *t == want))
    }
}

fn f(a: &BTreeMap<String, String>, k: &str) -> Option<f64> {
    a.get(k)?.trim().parse::<f64>().ok().filter(|x| x.is_finite())
}

/// Energy in kcal when Apple's unit is kcal/Cal; other units stay NULL in the
/// column (the raw value is still in detail_json).
fn kcal(x: Option<f64>, unit: Option<&String>) -> Option<f64> {
    match unit.map(|u| u.as_str()) {
        None | Some("kcal") | Some("Cal") => x,
        Some("kJ") => x.map(|v| v / 4.184),
        _ => None,
    }
}

/// Parse a metadata quantity like `"1234 cm"` into (value, unit).
fn meta_qty(s: &str) -> Option<(f64, String)> {
    let mut it = s.split_whitespace();
    let v: f64 = it.next()?.parse().ok()?;
    Some((v, it.next().unwrap_or("").to_string()))
}

/// Build from an export.xml `<Workout>` and its children.
///
/// Column sources (all taken as given, only unit-normalised):
/// - duration: `duration`/`durationUnit` attrs
/// - distance: `totalDistance` attr, else the Distance* statistic `sum`
/// - total energy: `totalEnergyBurned` attr (older exports only)
/// - active energy: ActiveEnergyBurned statistic `sum`
/// - avg/max HR: HeartRate statistic `average`/`maximum`
/// - elevation: `HKElevationAscended` metadata
///
/// Everything else (all statistics incl. min, events, metadata, route file
/// refs) is stored verbatim in `detail_json`.
pub fn from_export(attrs: &BTreeMap<String, String>, kids: &ExportChildren) -> Option<Workout> {
    let activity = attrs.get("workoutActivityType")?.clone();
    let (start_utc, _, local_start) = crate::import::parse_date(attrs.get("startDate")?)?;
    let (end_utc, _, local_end) = crate::import::parse_date(attrs.get("endDate")?)?;
    let source = attrs.get("sourceName").cloned().unwrap_or_default();
    let duration_min = f(attrs, "duration").map(|d| match attrs.get("durationUnit").map(|u| u.as_str()) {
        Some("sec") | Some("s") => d / 60.0,
        Some("hr") | Some("hour") => d * 60.0,
        _ => d, // min
    });
    let distance_m = f(attrs, "totalDistance")
        .map(|d| to_meters(d, attrs.get("totalDistanceUnit").map(|s| s.as_str()).unwrap_or("m")))
        .or_else(|| {
            let s = kids.statistics.iter().find(|s| {
                s.get("type")
                    .is_some_and(|t| t.starts_with("HKQuantityTypeIdentifierDistance"))
            })?;
            Some(to_meters(f(s, "sum")?, s.get("unit").map(|u| u.as_str()).unwrap_or("m")))
        });
    let total_kcal = kcal(f(attrs, "totalEnergyBurned"), attrs.get("totalEnergyBurnedUnit"));
    let active_kcal = kids
        .stat("ActiveEnergyBurned")
        .and_then(|s| kcal(f(s, "sum"), s.get("unit")));
    let hr = kids.stat("HeartRate");
    let avg_hr = hr.and_then(|s| f(s, "average"));
    let max_hr = hr.and_then(|s| f(s, "maximum"));
    let elevation_m = kids
        .metadata
        .iter()
        .find(|(k, _)| k == "HKElevationAscended")
        .and_then(|(_, v)| meta_qty(v))
        .map(|(v, u)| to_meters(v, if u.is_empty() { "m" } else { &u }));
    let external_id = kids
        .metadata
        .iter()
        .find(|(k, _)| k == "HKExternalUUID")
        .map(|(_, v)| v.clone());
    let detail_json = (!kids.is_empty()).then(|| {
        let mut o = serde_json::Map::new();
        if !kids.statistics.is_empty() {
            o.insert("statistics".into(), json!(kids.statistics));
        }
        if !kids.events.is_empty() {
            o.insert("events".into(), json!(kids.events));
        }
        if !kids.metadata.is_empty() {
            let m: serde_json::Map<String, Value> = kids
                .metadata
                .iter()
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect();
            o.insert("metadata".into(), Value::Object(m));
        }
        if !kids.route_files.is_empty() {
            o.insert("route_files".into(), json!(kids.route_files));
        }
        Value::Object(o).to_string()
    });
    Some(Workout {
        id: None,
        activity_type: activity,
        channel: crate::channel::EXPORT_XML.into(),
        source,
        start_utc,
        end_utc,
        local_start: Some(local_start),
        local_end: Some(local_end),
        duration_min,
        avg_hr,
        max_hr,
        active_kcal,
        total_kcal,
        distance_m,
        elevation_m,
        detail_json,
        external_id,
    })
}

pub async fn upsert(db: &SqlitePool, user_id: i64, w: &Workout) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO workouts
         (user_id, activity_type, channel, source, start_utc, end_utc, local_start, local_end,
          duration_min, avg_hr, max_hr, active_kcal, total_kcal, distance_m, elevation_m,
          detail_json, external_id)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
         ON CONFLICT (user_id, channel, source, start_utc, end_utc, activity_type) DO UPDATE SET
           duration_min = excluded.duration_min, avg_hr = excluded.avg_hr, max_hr = excluded.max_hr,
           active_kcal = excluded.active_kcal, total_kcal = excluded.total_kcal,
           distance_m = excluded.distance_m, elevation_m = excluded.elevation_m,
           detail_json = excluded.detail_json,
           external_id = COALESCE(excluded.external_id, workouts.external_id),
           local_start = excluded.local_start, local_end = excluded.local_end",
    )
    .bind(user_id)
    .bind(&w.activity_type)
    .bind(&w.channel)
    .bind(&w.source)
    .bind(&w.start_utc)
    .bind(&w.end_utc)
    .bind(&w.local_start)
    .bind(&w.local_end)
    .bind(w.duration_min)
    .bind(w.avg_hr)
    .bind(w.max_hr)
    .bind(w.active_kcal)
    .bind(w.total_kcal)
    .bind(w.distance_m)
    .bind(w.elevation_m)
    .bind(&w.detail_json)
    .bind(&w.external_id)
    .execute(db)
    .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct WorkoutRow {
    id: i64,
    activity_type: String,
    channel: String,
    source: String,
    start_utc: String,
    end_utc: String,
    local_start: Option<String>,
    local_end: Option<String>,
    duration_min: Option<f64>,
    avg_hr: Option<f64>,
    max_hr: Option<f64>,
    active_kcal: Option<f64>,
    total_kcal: Option<f64>,
    distance_m: Option<f64>,
    elevation_m: Option<f64>,
    detail_json: Option<String>,
    external_id: Option<String>,
}

impl From<WorkoutRow> for Workout {
    fn from(r: WorkoutRow) -> Self {
        Workout {
            id: Some(r.id),
            activity_type: r.activity_type,
            channel: r.channel,
            source: r.source,
            start_utc: r.start_utc,
            end_utc: r.end_utc,
            local_start: r.local_start,
            local_end: r.local_end,
            duration_min: r.duration_min,
            avg_hr: r.avg_hr,
            max_hr: r.max_hr,
            active_kcal: r.active_kcal,
            total_kcal: r.total_kcal,
            distance_m: r.distance_m,
            elevation_m: r.elevation_m,
            detail_json: r.detail_json,
            external_id: r.external_id,
        }
    }
}

pub async fn list(
    db: &SqlitePool,
    user_id: i64,
    limit: i64,
    before: Option<&str>,
) -> anyhow::Result<Vec<Workout>> {
    let rows: Vec<WorkoutRow> = sqlx::query_as(
        "SELECT id, activity_type, channel, source, start_utc, end_utc, local_start, local_end,
                duration_min, avg_hr, max_hr, active_kcal, total_kcal, distance_m, elevation_m,
                detail_json, external_id
         FROM workouts
         WHERE user_id = ? AND (? IS NULL OR start_utc < ?)
         ORDER BY start_utc DESC LIMIT ?",
    )
    .bind(user_id)
    .bind(before)
    .bind(before)
    .bind(limit.clamp(1, 200))
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(Workout::from).collect())
}

pub async fn get(db: &SqlitePool, user_id: i64, id: i64) -> anyhow::Result<Option<Workout>> {
    let row: Option<WorkoutRow> = sqlx::query_as(
        "SELECT id, activity_type, channel, source, start_utc, end_utc, local_start, local_end,
                duration_min, avg_hr, max_hr, active_kcal, total_kcal, distance_m, elevation_m,
                detail_json, external_id
         FROM workouts WHERE user_id = ? AND id = ?",
    )
    .bind(user_id)
    .bind(id)
    .fetch_optional(db)
    .await?;
    Ok(row.map(Workout::from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_hae_run() {
        let w = from_hae(
            &json!({
                "id": "abc",
                "name": "Outdoor Run",
                "start": "2024-06-01 18:00:00 -0400",
                "end": "2024-06-01 18:30:00 -0400",
                "duration": 1800,
                "source": "Watch",
                "activeEnergyBurned": {"qty": 310, "units": "kcal"},
                "distance": {"qty": 5.2, "units": "km"},
                "avgHeartRate": 145,
                "maxHeartRate": 172,
                "heartRateData": [
                    {"date": "2024-06-01 18:00:00 -0400", "qty": 120},
                    {"date": "2024-06-01 18:15:00 -0400", "qty": 150},
                    {"date": "2024-06-01 18:30:00 -0400", "qty": 160}
                ]
            }),
            crate::channel::HEALTH_AUTO_EXPORT,
        )
        .unwrap();
        assert_eq!(w.activity_type, "HKWorkoutActivityTypeRunning");
        assert_eq!(w.active_kcal, Some(310.0));
        assert!((w.distance_m.unwrap() - 5200.0).abs() < 0.1);
        assert_eq!(w.avg_hr, Some(145.0));
        assert_eq!(w.max_hr, Some(172.0));
        assert_eq!(w.external_id.as_deref(), Some("abc"));
        let d: Value = serde_json::from_str(w.detail_json.as_deref().unwrap()).unwrap();
        assert_eq!(d["heartRateData"][1]["qty"], 150, "samples kept verbatim");
    }

    #[test]
    fn hae_hr_not_computed_from_samples() {
        let w = from_hae(
            &json!({
                "name": "Outdoor Walk",
                "start": "2024-06-02 08:00:00 -0400",
                "end": "2024-06-02 08:30:00 -0400",
                "heartRateData": [
                    {"date": "2024-06-02 08:00:00 -0400", "qty": 100},
                    {"date": "2024-06-02 08:15:00 -0400", "qty": 130}
                ]
            }),
            crate::channel::HEALTH_AUTO_EXPORT,
        )
        .unwrap();
        assert_eq!((w.avg_hr, w.max_hr), (None, None));
        let w = from_hae(
            &json!({
                "name": "Outdoor Walk",
                "start": "2024-06-02 08:00:00 -0400",
                "end": "2024-06-02 08:30:00 -0400",
                "heartRate": {"min": {"qty": 90}, "avg": {"qty": 118}, "max": {"qty": 141}}
            }),
            crate::channel::HEALTH_AUTO_EXPORT,
        )
        .unwrap();
        assert_eq!((w.avg_hr, w.max_hr), (Some(118.0), Some(141.0)));
    }
}
