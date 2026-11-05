//! iPhone Shortcut sync: metric catalog, sync settings, upload tokens,
//! one-time setup codes, per-metric cursors and the ingest parser.
//! See docs/shortcut-design.md (§2a, §3, §4, §5, §10).

use crate::auth::sha256_hex;
use chrono::{DateTime, Duration, FixedOffset, SecondsFormat, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::BTreeMap;

pub const SHORTCUT_VERSION: &str = "0.1.0";
pub const MIN_SHORTCUT_VERSION: &str = "0.1.0";
pub const SCHEMA: i64 = 1;
pub const DEFAULT_LOOKBACK_DAYS: i64 = 7;
pub const MAX_LOOKBACK_DAYS: i64 = 30;
pub const MAX_SAMPLES_PER_METRIC: i64 = 5000;
/// Samples are re-requested from 10 min before the newest one we have,
/// because the Watch syncs to the phone late. Duplicates are dropped.
pub const OVERLAP_MIN: i64 = 10;
pub const SETUP_CODE_MINUTES: i64 = 10;
pub const RAW_KEEP_HOURS: i64 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Quantity,
    Category,
    Workout,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Metric {
    pub key: &'static str,
    /// Stored `readings.type` (same as export.xml).
    #[serde(skip)]
    pub hk: &'static str,
    pub name: &'static str,
    pub area: &'static str,
    /// Unit the Shortcut must send values in.
    pub unit: &'static str,
    pub kind: Kind,
    /// Enabled in the "Recommended" preset.
    pub recommended: bool,
    /// Name of the type in the Shortcuts "Find Health Samples" picker.
    pub shortcut_type: &'static str,
}

const fn q(
    key: &'static str,
    hk: &'static str,
    name: &'static str,
    area: &'static str,
    unit: &'static str,
    recommended: bool,
    shortcut_type: &'static str,
) -> Metric {
    Metric {
        key,
        hk,
        name,
        area,
        unit,
        kind: Kind::Quantity,
        recommended,
        shortcut_type,
    }
}

const fn c(
    key: &'static str,
    hk: &'static str,
    name: &'static str,
    area: &'static str,
    recommended: bool,
    shortcut_type: &'static str,
) -> Metric {
    Metric {
        key,
        hk,
        name,
        area,
        unit: "",
        kind: Kind::Category,
        recommended,
        shortcut_type,
    }
}

const QT: &str = "HKQuantityTypeIdentifier";
macro_rules! hq {
    ($s:literal) => {
        concat!("HKQuantityTypeIdentifier", $s)
    };
}
macro_rules! hc {
    ($s:literal) => {
        concat!("HKCategoryTypeIdentifier", $s)
    };
}

/// Supported metrics (docs/metrics.md).
pub const METRICS: &[Metric] = &[
    q(
        "stepCount",
        hq!("StepCount"),
        "Steps",
        "Activity",
        "count",
        true,
        "Steps",
    ),
    q(
        "distanceWalkingRunning",
        hq!("DistanceWalkingRunning"),
        "Walking + running distance",
        "Activity",
        "km",
        true,
        "Walking + Running Distance",
    ),
    q(
        "activeEnergyBurned",
        hq!("ActiveEnergyBurned"),
        "Active energy (Move)",
        "Activity",
        "kcal",
        true,
        "Active Energy",
    ),
    q(
        "basalEnergyBurned",
        hq!("BasalEnergyBurned"),
        "Resting energy",
        "Activity",
        "kcal",
        false,
        "Resting Energy",
    ),
    q(
        "appleExerciseTime",
        hq!("AppleExerciseTime"),
        "Exercise minutes",
        "Activity",
        "min",
        true,
        "Exercise Minutes",
    ),
    c(
        "appleStandHour",
        hc!("AppleStandHour"),
        "Stand hours",
        "Activity",
        true,
        "Stand Hours",
    ),
    q(
        "appleStandTime",
        hq!("AppleStandTime"),
        "Stand time",
        "Activity",
        "min",
        false,
        "Stand Minutes",
    ),
    q(
        "flightsClimbed",
        hq!("FlightsClimbed"),
        "Flights climbed",
        "Activity",
        "count",
        true,
        "Flights Climbed",
    ),
    q(
        "distanceCycling",
        hq!("DistanceCycling"),
        "Cycling distance",
        "Activity",
        "km",
        false,
        "Cycling Distance",
    ),
    q(
        "distanceSwimming",
        hq!("DistanceSwimming"),
        "Swimming distance",
        "Activity",
        "m",
        false,
        "Swimming Distance",
    ),
    q(
        "heartRate",
        hq!("HeartRate"),
        "Heart rate",
        "Heart",
        "count/min",
        true,
        "Heart Rate",
    ),
    q(
        "restingHeartRate",
        hq!("RestingHeartRate"),
        "Resting heart rate",
        "Heart",
        "count/min",
        true,
        "Resting Heart Rate",
    ),
    q(
        "walkingHeartRateAverage",
        hq!("WalkingHeartRateAverage"),
        "Walking heart rate avg",
        "Heart",
        "count/min",
        true,
        "Walking Heart Rate Average",
    ),
    q(
        "heartRateVariabilitySDNN",
        hq!("HeartRateVariabilitySDNN"),
        "HRV (SDNN)",
        "Heart",
        "ms",
        true,
        "Heart Rate Variability",
    ),
    q(
        "heartRateRecoveryOneMinute",
        hq!("HeartRateRecoveryOneMinute"),
        "Heart rate recovery (1 min)",
        "Heart",
        "count/min",
        false,
        "Cardio Recovery",
    ),
    q(
        "vo2Max",
        hq!("VO2Max"),
        "Cardio fitness (VO2 max)",
        "Heart",
        "mL/min·kg",
        true,
        "Cardio Fitness",
    ),
    c(
        "highHeartRateEvent",
        hc!("HighHeartRateEvent"),
        "High heart rate alert",
        "Heart",
        false,
        "High Heart Rate Notifications",
    ),
    c(
        "lowHeartRateEvent",
        hc!("LowHeartRateEvent"),
        "Low heart rate alert",
        "Heart",
        false,
        "Low Heart Rate Notifications",
    ),
    c(
        "irregularHeartRhythmEvent",
        hc!("IrregularHeartRhythmEvent"),
        "Irregular rhythm notification",
        "Heart",
        false,
        "Irregular Rhythm Notifications",
    ),
    q(
        "atrialFibrillationBurden",
        hq!("AtrialFibrillationBurden"),
        "AFib history",
        "Heart",
        "%",
        false,
        "AFib History",
    ),
    c(
        "sleepAnalysis",
        hc!("SleepAnalysis"),
        "Sleep stages",
        "Sleep",
        true,
        "Sleep",
    ),
    q(
        "appleSleepingWristTemperature",
        hq!("AppleSleepingWristTemperature"),
        "Sleeping wrist temperature",
        "Sleep",
        "degC",
        true,
        "Wrist Temperature",
    ),
    c(
        "sleepApneaEvent",
        hc!("SleepApneaEvent"),
        "Breathing disturbances notification",
        "Sleep",
        false,
        "Sleep Apnea",
    ),
    q(
        "appleSleepingBreathingDisturbances",
        hq!("AppleSleepingBreathingDisturbances"),
        "Breathing disturbances",
        "Respiratory",
        "count/hr",
        false,
        "Breathing Disturbances",
    ),
    q(
        "respiratoryRate",
        hq!("RespiratoryRate"),
        "Respiratory rate",
        "Respiratory",
        "count/min",
        true,
        "Respiratory Rate",
    ),
    q(
        "oxygenSaturation",
        hq!("OxygenSaturation"),
        "Blood oxygen",
        "Respiratory",
        "%",
        true,
        "Blood Oxygen",
    ),
    q(
        "bodyMass",
        hq!("BodyMass"),
        "Weight",
        "Body",
        "kg",
        true,
        "Weight",
    ),
    q(
        "bodyFatPercentage",
        hq!("BodyFatPercentage"),
        "Body fat",
        "Body",
        "%",
        true,
        "Body Fat Percentage",
    ),
    q(
        "leanBodyMass",
        hq!("LeanBodyMass"),
        "Lean body mass",
        "Body",
        "kg",
        true,
        "Lean Body Mass",
    ),
    q(
        "bodyMassIndex",
        hq!("BodyMassIndex"),
        "BMI",
        "Body",
        "count",
        false,
        "Body Mass Index",
    ),
    q(
        "height",
        hq!("Height"),
        "Height",
        "Body",
        "cm",
        false,
        "Height",
    ),
    q(
        "waistCircumference",
        hq!("WaistCircumference"),
        "Waist circumference",
        "Body",
        "cm",
        false,
        "Waist Circumference",
    ),
    q(
        "walkingSpeed",
        hq!("WalkingSpeed"),
        "Walking speed",
        "Mobility",
        "km/hr",
        false,
        "Walking Speed",
    ),
    q(
        "walkingStepLength",
        hq!("WalkingStepLength"),
        "Step length",
        "Mobility",
        "cm",
        false,
        "Walking Step Length",
    ),
    q(
        "walkingAsymmetryPercentage",
        hq!("WalkingAsymmetryPercentage"),
        "Walking asymmetry",
        "Mobility",
        "%",
        false,
        "Walking Asymmetry",
    ),
    q(
        "walkingDoubleSupportPercentage",
        hq!("WalkingDoubleSupportPercentage"),
        "Double support time",
        "Mobility",
        "%",
        false,
        "Double Support Time",
    ),
    q(
        "appleWalkingSteadiness",
        hq!("AppleWalkingSteadiness"),
        "Walking steadiness",
        "Mobility",
        "%",
        false,
        "Walking Steadiness",
    ),
    q(
        "sixMinuteWalkTestDistance",
        hq!("SixMinuteWalkTestDistance"),
        "Six-minute walk (estimated)",
        "Mobility",
        "m",
        false,
        "Six-Minute Walk",
    ),
    q(
        "stairAscentSpeed",
        hq!("StairAscentSpeed"),
        "Stair speed: up",
        "Mobility",
        "m/s",
        false,
        "Stair Speed: Up",
    ),
    q(
        "environmentalAudioExposure",
        hq!("EnvironmentalAudioExposure"),
        "Environmental sound level",
        "Hearing",
        "dBASPL",
        false,
        "Environmental Sound Levels",
    ),
    q(
        "headphoneAudioExposure",
        hq!("HeadphoneAudioExposure"),
        "Headphone audio level",
        "Hearing",
        "dBASPL",
        false,
        "Headphone Audio Levels",
    ),
    q(
        "timeInDaylight",
        hq!("TimeInDaylight"),
        "Time in daylight",
        "Other",
        "min",
        false,
        "Time in Daylight",
    ),
    c(
        "mindfulSession",
        hc!("MindfulSession"),
        "Mindful minutes",
        "Other",
        false,
        "Mindful Minutes",
    ),
    q(
        "dietaryWater",
        hq!("DietaryWater"),
        "Water",
        "Other",
        "mL",
        false,
        "Water",
    ),
    q(
        "dietaryCaffeine",
        hq!("DietaryCaffeine"),
        "Caffeine",
        "Other",
        "mg",
        false,
        "Caffeine",
    ),
    Metric {
        key: "workouts",
        hk: "HKWorkoutActivityType",
        name: "Workouts",
        area: "Workouts",
        unit: "",
        kind: Kind::Workout,
        recommended: true,
        shortcut_type: "Find Workouts",
    },
    q(
        "runningPower",
        hq!("RunningPower"),
        "Running power",
        "Workouts",
        "W",
        false,
        "Running Power",
    ),
    q(
        "runningSpeed",
        hq!("RunningSpeed"),
        "Running speed",
        "Workouts",
        "km/hr",
        false,
        "Running Speed",
    ),
    q(
        "runningStrideLength",
        hq!("RunningStrideLength"),
        "Running stride length",
        "Workouts",
        "m",
        false,
        "Running Stride Length",
    ),
    q(
        "runningVerticalOscillation",
        hq!("RunningVerticalOscillation"),
        "Vertical oscillation",
        "Workouts",
        "cm",
        false,
        "Running Vertical Oscillation",
    ),
    q(
        "runningGroundContactTime",
        hq!("RunningGroundContactTime"),
        "Ground contact time",
        "Workouts",
        "ms",
        false,
        "Running Ground Contact Time",
    ),
];

pub fn metric(key: &str) -> Option<&'static Metric> {
    METRICS.iter().find(|m| m.key == key)
}

/// Apple stores these as fractions (0.21) in export.xml; the Shortcut
/// reports percent (21). Values above 1 are divided by 100.
fn is_fraction(m: &Metric) -> bool {
    m.unit == "%" && m.hk.starts_with(QT)
}

pub const TRIGGERS: &[&str] = &[
    "alarm_stopped",
    "time_of_day",
    "arrive_home",
    "charger",
    "manual",
];
pub const NOTIFY: &[&str] = &["errors_only", "always"];

// ---------------------------------------------------------------- settings

#[derive(Debug, Serialize)]
pub struct SyncSettings {
    pub configured: bool,
    pub enabled: Vec<String>,
    pub trigger: String,
    pub notify: String,
}

pub async fn settings(db: &SqlitePool, user_id: i64) -> anyhow::Result<SyncSettings> {
    let (configured, trigger, notify): (bool, String, String) =
        sqlx::query_as("SELECT sync_configured, sync_trigger, sync_notify FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(db)
            .await?;
    let enabled = if configured {
        sqlx::query_scalar("SELECT metric FROM sync_metrics WHERE user_id = ? ORDER BY metric")
            .bind(user_id)
            .fetch_all(db)
            .await?
    } else {
        METRICS
            .iter()
            .filter(|m| m.recommended)
            .map(|m| m.key.to_string())
            .collect()
    };
    Ok(SyncSettings {
        configured,
        enabled,
        trigger,
        notify,
    })
}

#[derive(Debug)]
pub enum SettingsError {
    UnknownMetric(String),
    NoMetrics,
    BadTrigger,
    BadNotify,
}

pub async fn save_settings(
    db: &SqlitePool,
    user_id: i64,
    enabled: &[String],
    trigger: &str,
    notify: &str,
) -> anyhow::Result<Result<(), SettingsError>> {
    if enabled.is_empty() {
        return Ok(Err(SettingsError::NoMetrics));
    }
    if let Some(bad) = enabled.iter().find(|k| metric(k).is_none()) {
        return Ok(Err(SettingsError::UnknownMetric(bad.clone())));
    }
    if !TRIGGERS.contains(&trigger) {
        return Ok(Err(SettingsError::BadTrigger));
    }
    if !NOTIFY.contains(&notify) {
        return Ok(Err(SettingsError::BadNotify));
    }
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM sync_metrics WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    for k in enabled {
        sqlx::query("INSERT OR IGNORE INTO sync_metrics (user_id, metric) VALUES (?, ?)")
            .bind(user_id)
            .bind(k)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "UPDATE users SET sync_configured = 1, sync_trigger = ?, sync_notify = ? WHERE id = ?",
    )
    .bind(trigger)
    .bind(notify)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Ok(()))
}

// ---------------------------------------------------------------- tokens

fn random_hex(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// New one-time setup code for QR code 2. Older unused codes of this user
/// are invalidated. Returns (code, expires_at).
pub async fn new_setup_code(db: &SqlitePool, user_id: i64) -> anyhow::Result<(String, String)> {
    let code = random_hex(16);
    let exp = (Utc::now() + Duration::minutes(SETUP_CODE_MINUTES))
        .to_rfc3339_opts(SecondsFormat::Secs, true);
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM setup_codes WHERE user_id = ? OR expires_at < ?")
        .bind(user_id)
        .bind(now())
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO setup_codes (code_hash, user_id, expires_at) VALUES (?, ?, ?)")
        .bind(sha256_hex(&code))
        .bind(user_id)
        .bind(&exp)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok((code, exp))
}

/// Exchange a setup code for this user's upload token. The code is single
/// use; any previous token of the user is replaced (one device per user).
pub async fn claim(
    db: &SqlitePool,
    code: &str,
    device: &str,
    version: &str,
) -> anyhow::Result<Option<(i64, String)>> {
    let mut tx = db.begin().await?;
    let user: Option<i64> = sqlx::query_scalar(
        "DELETE FROM setup_codes WHERE code_hash = ? AND expires_at > ? RETURNING user_id",
    )
    .bind(sha256_hex(code.trim()))
    .bind(now())
    .fetch_optional(&mut *tx)
    .await?;
    let Some(user_id) = user else {
        return Ok(None);
    };
    let prefix = random_hex(6);
    let secret = random_hex(32);
    sqlx::query(
        "INSERT OR REPLACE INTO upload_tokens
         (user_id, prefix, secret_hash, device, shortcut_version, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(&prefix)
    .bind(sha256_hex(&secret))
    .bind(device.chars().take(100).collect::<String>())
    .bind(version.chars().take(20).collect::<String>())
    .bind(now())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some((user_id, format!("hvu_{prefix}_{secret}"))))
}

/// Create (or replace) this user's upload token directly from the web app,
/// for apps such as Health Auto Export that take a pasted token.
pub async fn new_token(db: &SqlitePool, user_id: i64, device: &str) -> anyhow::Result<String> {
    let prefix = random_hex(6);
    let secret = random_hex(32);
    sqlx::query(
        "INSERT OR REPLACE INTO upload_tokens
         (user_id, prefix, secret_hash, device, shortcut_version, created_at)
         VALUES (?, ?, ?, ?, NULL, ?)",
    )
    .bind(user_id)
    .bind(&prefix)
    .bind(sha256_hex(&secret))
    .bind(device.chars().take(100).collect::<String>())
    .bind(now())
    .execute(db)
    .await?;
    Ok(format!("hvu_{prefix}_{secret}"))
}

fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Resolve a bearer upload token to its user id.
pub async fn token_user(db: &SqlitePool, token: &str) -> anyhow::Result<Option<i64>> {
    let Some(rest) = token.strip_prefix("hvu_") else {
        return Ok(None);
    };
    let Some((prefix, secret)) = rest.split_once('_') else {
        return Ok(None);
    };
    let row: Option<(i64, String)> =
        sqlx::query_as("SELECT user_id, secret_hash FROM upload_tokens WHERE prefix = ?")
            .bind(prefix)
            .fetch_optional(db)
            .await?;
    Ok(row.and_then(|(u, h)| ct_eq(&h, &sha256_hex(secret)).then_some(u)))
}

pub async fn touch_token(
    db: &SqlitePool,
    user_id: i64,
    version: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE upload_tokens SET last_seen_at = ?,
           shortcut_version = COALESCE(?, shortcut_version) WHERE user_id = ?",
    )
    .bind(now())
    .bind(version)
    .bind(user_id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn revoke_token(db: &SqlitePool, user_id: i64) -> anyhow::Result<bool> {
    Ok(sqlx::query("DELETE FROM upload_tokens WHERE user_id = ?")
        .bind(user_id)
        .execute(db)
        .await?
        .rows_affected()
        > 0)
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TokenInfo {
    pub device: Option<String>,
    pub shortcut_version: Option<String>,
    pub created_at: String,
    pub last_seen_at: Option<String>,
    pub last_upload_at: Option<String>,
}

pub async fn token_info(db: &SqlitePool, user_id: i64) -> anyhow::Result<Option<TokenInfo>> {
    Ok(sqlx::query_as(
        "SELECT device, shortcut_version, created_at, last_seen_at, last_upload_at
         FROM upload_tokens WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?)
}

// ---------------------------------------------------------------- config

/// `a < b` for dotted versions like 0.1.0.
pub fn version_lt(a: &str, b: &str) -> bool {
    let p = |s: &str| -> Vec<u64> {
        s.trim()
            .trim_start_matches('v')
            .split('.')
            .map(|x| x.parse().unwrap_or(0))
            .collect()
    };
    p(a) < p(b)
}

fn rfc(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Where each enabled metric should start: just before the newest sample
/// we already have (from the Shortcut or an export.xml import), but never
/// more than 30 days back; with no data at all, the last 7 days.
pub async fn since(
    db: &SqlitePool,
    user_id: i64,
    enabled: &[String],
    now: DateTime<Utc>,
) -> anyhow::Result<BTreeMap<String, String>> {
    let floor = now - Duration::days(MAX_LOOKBACK_DAYS);
    let mut out = BTreeMap::new();
    for key in enabled {
        let Some(m) = metric(key) else { continue };
        let cursor: Option<String> = sqlx::query_scalar(
            "SELECT last_end_utc FROM sync_cursors WHERE user_id = ? AND metric = ?",
        )
        .bind(user_id)
        .bind(key)
        .fetch_optional(db)
        .await?;
        let newest: Option<String> = match cursor {
            Some(c) => Some(c),
            None => {
                let like = if m.kind == Kind::Workout {
                    format!("{}%", m.hk)
                } else {
                    m.hk.to_string()
                };
                sqlx::query_scalar(
                    "SELECT MAX(end_utc) FROM readings WHERE user_id = ? AND type LIKE ?
                       AND end_utc <= ?",
                )
                .bind(user_id)
                .bind(like)
                .bind(rfc(now))
                .fetch_one(db)
                .await?
            }
        };
        let s = match newest.and_then(|t| DateTime::parse_from_rfc3339(&t).ok()) {
            Some(t) => (t.with_timezone(&Utc) - Duration::minutes(OVERLAP_MIN)).max(floor),
            None => now - Duration::days(DEFAULT_LOOKBACK_DAYS),
        };
        out.insert(key.clone(), rfc(s));
    }
    Ok(out)
}

pub async fn config_json(
    db: &SqlitePool,
    user_id: i64,
    max_request_bytes: usize,
) -> anyhow::Result<Value> {
    let s = settings(db, user_id).await?;
    let username: String = sqlx::query_scalar("SELECT username FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(db)
        .await?;
    let since = since(db, user_id, &s.enabled, Utc::now()).await?;
    let units: BTreeMap<&str, &str> = s
        .enabled
        .iter()
        .filter_map(|k| metric(k))
        .filter(|m| m.kind == Kind::Quantity)
        .map(|m| (m.key, m.unit))
        .collect();
    Ok(serde_json::json!({
        "schema": SCHEMA,
        "user": username,
        "min_shortcut_version": MIN_SHORTCUT_VERSION,
        "latest_shortcut_version": SHORTCUT_VERSION,
        "enabled": s.enabled,
        "since": since,
        "units": units,
        "default_lookback_days": DEFAULT_LOOKBACK_DAYS,
        "max_lookback_days": MAX_LOOKBACK_DAYS,
        "max_samples_per_metric": MAX_SAMPLES_PER_METRIC,
        "max_request_bytes": max_request_bytes,
        "notify": s.notify,
        "trigger": s.trigger,
    }))
}

// ---------------------------------------------------------------- ingest

#[derive(Debug, Deserialize)]
pub struct Upload {
    #[serde(default)]
    pub schema: Option<Value>,
    #[serde(default)]
    pub shortcut_version: Option<String>,
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default)]
    pub metrics: BTreeMap<String, Value>,
    #[serde(default)]
    pub workouts: Option<Value>,
}

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct IngestResult {
    pub accepted: u64,
    pub duplicates: u64,
    /// Samples that could not be read (bad date or value).
    pub invalid: u64,
    pub rejected: BTreeMap<String, String>,
    pub next_since: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

#[derive(Debug)]
pub enum IngestError {
    /// Body is not valid JSON of the expected shape.
    BadJson(String),
    /// A metric key healthvibe doesn't know (422).
    UnknownMetric(String),
}

/// Parse a date the way a Shortcut sends it: ISO 8601 with offset
/// (`Format Date → ISO 8601`), or Apple's export style `2026-11-02 07:00:00 -0500`.
pub fn parse_ts(s: &str) -> Option<DateTime<FixedOffset>> {
    let s = s.trim();
    DateTime::parse_from_rfc3339(s)
        .or_else(|_| DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S %z"))
        .or_else(|_| DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z"))
        .or_else(|_| DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f%z"))
        .ok()
}

/// A number, or text starting with one ("812", "64 count/min", "0,5").
fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let t: String = s
                .trim()
                .chars()
                .take_while(|c| {
                    c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+' | 'e' | 'E')
                })
                .collect();
            // "1,234.5" → 1234.5; "0,5" (comma decimal) → 0.5
            if t.contains('.') {
                t.replace(',', "").parse().ok()
            } else {
                t.replace(',', ".").parse().ok()
            }
        }
        _ => None,
    }
    .filter(|x: &f64| x.is_finite())
}

fn fmt_num(x: f64) -> String {
    // Trim float noise from unit conversion (0.45359237 * 180 …).
    let r = (x * 1e6).round() / 1e6;
    format!("{r}")
}

/// Map a category value from the Shortcut ("Core", "asleepCore", "In Bed",
/// "stood", …) to the HealthKit value stored by export.xml.
pub fn category_value(m: &Metric, v: &Value) -> Option<String> {
    let raw = match v {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        Value::Null => String::new(),
        _ => return None,
    };
    if raw.starts_with("HKCategoryValue") {
        return Some(raw);
    }
    let k: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    match m.key {
        "sleepAnalysis" => {
            let s = match k.as_str() {
                "inbed" | "0" => "InBed",
                "asleep" | "asleepunspecified" | "unspecified" | "1" => "AsleepUnspecified",
                "awake" | "2" => "Awake",
                "core" | "asleepcore" | "3" => "AsleepCore",
                "deep" | "asleepdeep" | "4" => "AsleepDeep",
                "rem" | "asleeprem" | "5" => "AsleepREM",
                _ => return None,
            };
            Some(format!("HKCategoryValueSleepAnalysis{s}"))
        }
        "appleStandHour" => match k.as_str() {
            "stood" | "0" => Some("HKCategoryValueAppleStandHourStood".into()),
            "idle" | "1" => Some("HKCategoryValueAppleStandHourIdle".into()),
            _ => None,
        },
        _ => Some(if raw.is_empty() {
            "HKCategoryValueNotApplicable".into()
        } else {
            raw
        }),
    }
}

struct Sample {
    kind: String,
    value: Option<String>,
    unit: Option<String>,
    start: DateTime<FixedOffset>,
    end: DateTime<FixedOffset>,
    source: Option<String>,
}

fn text(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Samples may come as `{"unit":..,"samples":[..]}` or as a bare array.
fn sample_list(v: &Value) -> Option<&Vec<Value>> {
    match v {
        Value::Array(a) => Some(a),
        Value::Object(o) => match o.get("samples") {
            Some(Value::Array(a)) => Some(a),
            None | Some(Value::Null) => None,
            _ => None,
        },
        _ => None,
    }
}

/// Convert a value sent in the phone's display unit (US phones often say
/// lb, mi, °F) to the unit healthvibe stores. Unknown or missing units are
/// taken to be the metric's own unit already.
pub fn convert(x: f64, from: &str, to: &str) -> f64 {
    let f = from.trim().to_lowercase().replace(['°', ' '], "");
    let f = f.as_str();
    match (f, to) {
        ("lb" | "lbs" | "pound" | "pounds", "kg") => x * 0.453_592_37,
        ("g", "kg") => x / 1000.0,
        ("mi" | "mile" | "miles", "km") => x * 1.609_344,
        ("m", "km") => x / 1000.0,
        ("ft", "cm") => x * 30.48,
        ("in", "cm") => x * 2.54,
        ("m", "cm") => x * 100.0,
        ("yd" | "yds", "m") => x * 0.9144,
        ("km", "m") => x * 1000.0,
        ("mi" | "mile" | "miles", "m") => x * 1_609.344,
        ("ft", "m") => x * 0.3048,
        ("degf" | "f", "degC") => (x - 32.0) * 5.0 / 9.0,
        ("kj", "kcal") => x / 4.184,
        // Apple shows kilocalories as "Cal".
        ("cal", "kcal") => x,
        ("mph" | "mi/hr", "km/hr") => x * 1.609_344,
        ("m/s", "km/hr") => x * 3.6,
        ("ft/s", "m/s") => x * 0.3048,
        ("km/hr" | "kph", "m/s") => x / 3.6,
        ("floz" | "fl_oz_us", "mL") => x * 29.573_53,
        ("l", "mL") => x * 1000.0,
        ("hr" | "h", "min") => x * 60.0,
        ("s" | "sec", "min") => x / 60.0,
        _ => x,
    }
}

fn to_sample(m: &Metric, s: &Value, unit_hint: Option<&str>) -> Result<Sample, String> {
    let o = s.as_object().ok_or("sample is not an object")?;
    let get = |k: &[&str]| k.iter().find_map(|k| o.get(*k));
    let start = text(get(&["start", "startDate", "Start Date"]))
        .and_then(|t| parse_ts(&t))
        .ok_or("bad or missing start date (use ISO 8601 with offset)")?;
    let end = match text(get(&["end", "endDate", "End Date"])) {
        Some(t) => parse_ts(&t).ok_or("bad end date (use ISO 8601 with offset)")?,
        None => start,
    };
    if end < start {
        return Err("end is before start".into());
    }
    let v = get(&["value", "Value"]).cloned().unwrap_or(Value::Null);
    let (value, unit) = match m.kind {
        Kind::Quantity => {
            let mut x = number(&v).ok_or("value is not a number")?;
            if let Some(u) = text(get(&["unit", "Unit"])).as_deref().or(unit_hint) {
                x = convert(x, u, m.unit);
            }
            if is_fraction(m) && x > 1.0 {
                x /= 100.0;
            }
            (Some(fmt_num(x)), Some(m.unit.to_string()))
        }
        Kind::Category => (
            Some(category_value(m, &v).ok_or("unknown category value")?),
            None,
        ),
        Kind::Workout => unreachable!(),
    };
    Ok(Sample {
        kind: m.hk.to_string(),
        value,
        unit,
        start,
        end,
        source: text(get(&["source", "sourceName", "Source"])),
    })
}

fn camel_to_pascal(s: &str) -> String {
    let mut c = s.trim().chars().filter(|c| c.is_alphanumeric());
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => "Other".into(),
    }
}

fn to_workout(w: &Value) -> Result<Sample, String> {
    let o = w.as_object().ok_or("workout is not an object")?;
    let start = text(o.get("start"))
        .and_then(|t| parse_ts(&t))
        .ok_or("bad or missing workout start")?;
    let end = text(o.get("end"))
        .and_then(|t| parse_ts(&t))
        .ok_or("bad or missing workout end")?;
    let ty = text(o.get("type")).ok_or("missing workout type")?;
    let ty = ty.trim_start_matches("HKWorkoutActivityType");
    let secs = o
        .get("duration_s")
        .and_then(number)
        .unwrap_or_else(|| (end - start).num_seconds() as f64);
    Ok(Sample {
        kind: format!("HKWorkoutActivityType{}", camel_to_pascal(ty)),
        value: Some(fmt_num((secs / 60.0 * 1000.0).round() / 1000.0)),
        unit: Some("min".into()),
        start,
        end,
        source: text(o.get("source")),
    })
}

fn utc(t: DateTime<FixedOffset>) -> String {
    t.with_timezone(&Utc)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

fn sample_hash(s: &Sample) -> String {
    crate::channel::reading_hash(
        crate::channel::SHORTCUT,
        &s.kind,
        s.source.as_deref().unwrap_or(""),
        &utc(s.start),
        &utc(s.end),
    )
}

/// Delete raw bodies whose debugging window has passed.
pub async fn purge_raw(db: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM ingest_raw WHERE expires_at IS NOT NULL AND expires_at < ?")
        .bind(now())
        .execute(db)
        .await?;
    Ok(())
}

/// Store the raw body, parse it, store the samples, then delete the raw body
/// (or keep it 24 h if anything failed).
pub async fn ingest(
    db: &SqlitePool,
    user_id: i64,
    body: &[u8],
) -> anyhow::Result<Result<IngestResult, IngestError>> {
    purge_raw(db).await?;
    let raw_id: i64 = sqlx::query_scalar(
        "INSERT INTO ingest_raw (user_id, received_at, body, status) VALUES (?, ?, ?, 'pending')
         RETURNING id",
    )
    .bind(user_id)
    .bind(now())
    .bind(body)
    .fetch_one(db)
    .await?;
    let outcome = parse_and_store(db, user_id, body).await;
    let failure = match &outcome {
        Err(e) => Some(e.to_string()),
        Ok(Err(IngestError::BadJson(e))) => Some(format!("bad json: {e}")),
        Ok(Err(IngestError::UnknownMetric(k))) => Some(format!("unknown metric: {k}")),
        Ok(Ok(r)) if r.invalid > 0 => Some(r.errors.join("; ")),
        Ok(Ok(_)) => None,
    };
    match failure {
        None => {
            sqlx::query("DELETE FROM ingest_raw WHERE id = ?")
                .bind(raw_id)
                .execute(db)
                .await?;
        }
        Some(err) => {
            let exp = rfc(Utc::now() + Duration::hours(RAW_KEEP_HOURS));
            sqlx::query(
                "UPDATE ingest_raw SET status = 'failed', error = ?, expires_at = ? WHERE id = ?",
            )
            .bind(err)
            .bind(exp)
            .bind(raw_id)
            .execute(db)
            .await?;
        }
    }
    outcome
}

async fn parse_and_store(
    db: &SqlitePool,
    user_id: i64,
    body: &[u8],
) -> anyhow::Result<Result<IngestResult, IngestError>> {
    let up: Upload = match serde_json::from_slice(body) {
        Ok(u) => u,
        Err(e) => return Ok(Err(IngestError::BadJson(e.to_string()))),
    };
    let _ = (&up.schema, &up.device);
    if let Some(k) = up.metrics.keys().find(|k| metric(k).is_none()) {
        return Ok(Err(IngestError::UnknownMetric(k.clone())));
    }
    let enabled = settings(db, user_id).await?.enabled;
    let mut res = IngestResult::default();
    // (metric key, samples)
    let mut batches: Vec<(&'static str, Vec<Sample>)> = vec![];
    let note = |res: &mut IngestResult, key: &str, e: String| {
        res.invalid += 1;
        if res.errors.len() < 5 {
            res.errors.push(format!("{key}: {e}"));
        }
    };
    for (key, v) in &up.metrics {
        let m = metric(key).expect("checked above");
        if !enabled.iter().any(|e| e == key) {
            res.rejected.insert(key.clone(), "not_enabled".into());
            continue;
        }
        if m.kind == Kind::Workout {
            res.rejected
                .insert(key.clone(), "send_workouts_in_workouts_array".into());
            continue;
        }
        let Some(list) = sample_list(v) else {
            if !v.is_null() {
                note(&mut res, key, "expected {\"samples\": [...]}".into());
            }
            continue;
        };
        let hint = v.get("unit").and_then(|u| u.as_str());
        let mut out = vec![];
        for s in list {
            match to_sample(m, s, hint) {
                Ok(x) => out.push(x),
                Err(e) => note(&mut res, key, e),
            }
        }
        batches.push((m.key, out));
    }
    if let Some(w) = &up.workouts {
        if !enabled.iter().any(|e| e == "workouts") {
            res.rejected.insert("workouts".into(), "not_enabled".into());
        } else {
            let list = match w {
                Value::Array(a) => a.clone(),
                _ => vec![],
            };
            let mut out = vec![];
            for x in &list {
                match to_workout(x) {
                    Ok(s) => out.push(s),
                    Err(e) => note(&mut res, "workouts", e),
                }
            }
            batches.push(("workouts", out));
        }
    }

    let mut tx = db.begin().await?;
    for (key, samples) in &batches {
        let mut newest: Option<DateTime<FixedOffset>> = None;
        for s in samples {
            let source = s.source.clone().unwrap_or_default();
            let r = sqlx::query(
                "INSERT INTO readings
                 (user_id, type, value, unit, start_utc, end_utc, tz_offset, source, channel, hash,
                  local_start, local_end)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT (user_id, type, channel, source, start_utc, end_utc) DO UPDATE SET
                   value = excluded.value, unit = excluded.unit,
                   tz_offset = excluded.tz_offset, hash = excluded.hash,
                   local_start = excluded.local_start, local_end = excluded.local_end
                 WHERE value IS NOT excluded.value
                    OR unit IS NOT excluded.unit
                    OR local_start IS NOT excluded.local_start
                    OR local_end IS NOT excluded.local_end",
            )
            .bind(user_id)
            .bind(&s.kind)
            .bind(&s.value)
            .bind(&s.unit)
            .bind(utc(s.start))
            .bind(utc(s.end))
            .bind(s.start.format("%z").to_string())
            .bind(&source)
            .bind(crate::channel::SHORTCUT)
            .bind(sample_hash(s))
            .bind(s.start.format("%Y-%m-%d %H:%M:%S").to_string())
            .bind(s.end.format("%Y-%m-%d %H:%M:%S").to_string())
            .execute(&mut *tx)
            .await?;
            if r.rows_affected() == 1 {
                res.accepted += 1;
            } else {
                res.duplicates += 1;
            }
            if newest.is_none_or(|n| s.end > n) {
                newest = Some(s.end);
            }
        }
        if let Some(n) = newest {
            let n = utc(n);
            sqlx::query(
                "INSERT INTO sync_cursors (user_id, metric, last_end_utc) VALUES (?, ?, ?)
                 ON CONFLICT (user_id, metric) DO UPDATE SET last_end_utc =
                   MAX(last_end_utc, excluded.last_end_utc)",
            )
            .bind(user_id)
            .bind(key)
            .bind(&n)
            .execute(&mut *tx)
            .await?;
        }
    }
    sqlx::query(
        "UPDATE upload_tokens SET last_upload_at = ?, last_seen_at = ?,
           shortcut_version = COALESCE(?, shortcut_version) WHERE user_id = ?",
    )
    .bind(now())
    .bind(now())
    .bind(up.shortcut_version.as_deref())
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let keys: Vec<String> = batches.iter().map(|(k, _)| k.to_string()).collect();
    res.next_since = since(db, user_id, &keys, Utc::now()).await?;
    Ok(Ok(res))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn units_and_categories() {
        assert!((convert(180.0, "lb", "kg") - 81.646_627).abs() < 1e-4);
        assert!((convert(98.6, "°F", "degC") - 37.0).abs() < 1e-9);
        assert_eq!(convert(5.0, "km", "km"), 5.0);
        let sleep = metric("sleepAnalysis").unwrap();
        for (v, want) in [
            ("Core", "AsleepCore"),
            ("asleepREM", "AsleepREM"),
            ("In Bed", "InBed"),
            ("Awake", "Awake"),
            ("Asleep", "AsleepUnspecified"),
        ] {
            assert_eq!(
                category_value(sleep, &json!(v)).unwrap(),
                format!("HKCategoryValueSleepAnalysis{want}")
            );
        }
        assert!(category_value(sleep, &json!("napping")).is_none());
        let fat = metric("bodyFatPercentage").unwrap();
        let s = to_sample(
            fat,
            &json!({"start": "2026-11-02T07:00:00-05:00", "value": "21.5"}),
            None,
        )
        .unwrap();
        assert_eq!(s.value.as_deref(), Some("0.215"));
        let w = metric("bodyMass").unwrap();
        let s = to_sample(
            w,
            &json!({"start": "2026-11-02 07:00:00 -0500", "value": 180, "unit": "lb"}),
            None,
        )
        .unwrap();
        assert_eq!(s.value.as_deref(), Some("81.646627"));
        assert_eq!(utc(s.start), "2026-11-02T12:00:00Z");
    }

    #[test]
    fn versions() {
        assert!(version_lt("0.0.9", "0.1.0"));
        assert!(!version_lt("0.1.0", "0.1.0"));
        assert!(!version_lt("0.10.0", "0.9.0"));
    }
}
