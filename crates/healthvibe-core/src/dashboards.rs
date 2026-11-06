//! Custom dashboards and the generic per-metric queries behind their widgets.
//! Every query is scoped by `user_id`.

use crate::stats::{self, DayValue};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::BTreeMap;

pub const MAX_WIDGETS: usize = 40;
pub const MAX_DASHBOARDS: i64 = 50;
pub const KINDS: &[&str] = &["line", "bar", "stat", "activity", "sleep", "body", "heart"];

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Widget {
    pub id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default = "d30")]
    pub days: i64,
    #[serde(default = "dday")]
    pub bucket: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agg: Option<String>,
    #[serde(default = "d2")]
    pub size: i64,
}
fn d30() -> i64 {
    30
}
fn d2() -> i64 {
    2
}
fn dday() -> String {
    "day".into()
}

/// Checks a widget list; returns an error code for the API.
pub fn validate(ws: &[Widget]) -> Result<(), &'static str> {
    if ws.len() > MAX_WIDGETS {
        return Err("too_many_widgets");
    }
    for w in ws {
        let chart = matches!(w.kind.as_str(), "line" | "bar" | "stat");
        if !KINDS.contains(&w.kind.as_str())
            || w.id.is_empty()
            || w.id.len() > 40
            || !(1..=4).contains(&w.size)
            || !(1..=3650).contains(&w.days)
            || !["day", "week", "month"].contains(&w.bucket.as_str())
            || w.agg
                .as_deref()
                .is_some_and(|a| !["sum", "avg", "count"].contains(&a))
            || w.title.as_deref().is_some_and(|t| t.chars().count() > 80)
            || (chart
                && w.metric
                    .as_deref()
                    .is_none_or(|m| m.is_empty() || m.len() > 200))
        {
            return Err("invalid_widget");
        }
    }
    Ok(())
}

/// The old auto-seeded Summary layout. New dashboards are blank. This list is
/// only the fingerprint of a row nobody has edited, so migration can clear it.
pub fn default_widgets() -> Vec<Widget> {
    let w = |id: &str, kind: &str, metric: Option<&str>, size: i64| Widget {
        id: id.into(),
        kind: kind.into(),
        metric: metric.map(Into::into),
        title: None,
        days: 30,
        bucket: "day".into(),
        agg: None,
        size,
    };
    vec![
        w("activity", "activity", None, 2),
        Widget {
            title: Some("Steps".into()),
            ..w("steps", "bar", Some(stats::STEPS), 2)
        },
        w("heart", "heart", None, 2),
        w("sleep", "sleep", None, 2),
        w("body", "body", None, 4),
    ]
}

#[derive(Debug, Serialize)]
pub struct Dashboard {
    pub id: i64,
    pub name: String,
    pub position: i64,
    pub widgets: Vec<Widget>,
    /// `summary` for the protected built-in dashboard. Custom dashboards are null.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builtin: Option<String>,
}

type Row = (
    i64,
    String,
    i64,
    String,
    Option<String>,
    String,
    String,
    i64,
);

fn from_row((id, name, position, w, builtin, _created, _updated, _pins): Row) -> Dashboard {
    Dashboard {
        id,
        name,
        position,
        widgets: serde_json::from_str(&w).unwrap_or_default(),
        builtin,
    }
}

const SELECT: &str =
    "SELECT id, name, position, widgets_json, builtin, created_at, updated_at, pins_applied
     FROM dashboards WHERE user_id = ? ORDER BY position, id";

/// Untouched copy of the old auto-seeded Summary (same widgets, never saved again).
pub fn is_untouched_seed(name: &str, widgets: &[Widget], created: &str, updated: &str) -> bool {
    name == "Summary" && created == updated && widgets == default_widgets().as_slice()
}

pub async fn list(db: &SqlitePool, uid: i64) -> anyhow::Result<Vec<Dashboard>> {
    prepare_user(db, uid).await?;
    let rows: Vec<Row> = sqlx::query_as(SELECT).bind(uid).fetch_all(db).await?;
    Ok(rows.into_iter().map(from_row).collect())
}

pub async fn get(db: &SqlitePool, uid: i64, id: i64) -> anyhow::Result<Option<Dashboard>> {
    let r: Option<Row> = sqlx::query_as(
        "SELECT id, name, position, widgets_json, builtin, created_at, updated_at, pins_applied
         FROM dashboards WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(uid)
    .fetch_optional(db)
    .await?;
    Ok(r.map(from_row))
}

pub async fn count(db: &SqlitePool, uid: i64) -> anyhow::Result<i64> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM dashboards WHERE user_id = ?")
            .bind(uid)
            .fetch_one(db)
            .await?,
    )
}

pub async fn create(db: &SqlitePool, uid: i64, name: &str, ws: &[Widget]) -> anyhow::Result<i64> {
    create_with(db, uid, name, ws, None).await
}

async fn create_with(
    db: &SqlitePool,
    uid: i64,
    name: &str,
    ws: &[Widget],
    builtin: Option<&str>,
) -> anyhow::Result<i64> {
    let pos: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM dashboards WHERE user_id = ?",
    )
    .bind(uid)
    .fetch_one(db)
    .await?;
    Ok(sqlx::query(
        "INSERT INTO dashboards (user_id, name, position, widgets_json, builtin, pins_applied)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(uid)
    .bind(name)
    .bind(pos)
    .bind(serde_json::to_string(ws)?)
    .bind(builtin)
    .bind(if builtin == Some("summary") { 0 } else { 1 })
    .execute(db)
    .await?
    .last_insert_rowid())
}

pub async fn builtin_of(db: &SqlitePool, uid: i64, id: i64) -> anyhow::Result<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT builtin FROM dashboards WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(uid)
            .fetch_optional(db)
            .await?
            .flatten(),
    )
}

pub async fn update(
    db: &SqlitePool,
    uid: i64,
    id: i64,
    name: Option<&str>,
    ws: Option<&[Widget]>,
) -> anyhow::Result<Result<bool, &'static str>> {
    if name.is_some() && builtin_of(db, uid, id).await?.is_some() {
        return Ok(Err("protected"));
    }
    let ws = ws.map(serde_json::to_string).transpose()?;
    let ok = sqlx::query(
        "UPDATE dashboards SET name = COALESCE(?, name), widgets_json = COALESCE(?, widgets_json),
         updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id = ? AND user_id = ?",
    )
    .bind(name)
    .bind(ws)
    .bind(id)
    .bind(uid)
    .execute(db)
    .await?
    .rows_affected()
        == 1;
    Ok(Ok(ok))
}

/// `Ok(true)` deleted, `Ok(false)` missing, `Err("protected")` is the built-in Summary.
pub async fn delete(
    db: &SqlitePool,
    uid: i64,
    id: i64,
) -> anyhow::Result<Result<bool, &'static str>> {
    if builtin_of(db, uid, id).await?.is_some() {
        return Ok(Err("protected"));
    }
    let ok = sqlx::query("DELETE FROM dashboards WHERE id = ? AND user_id = ? AND builtin IS NULL")
        .bind(id)
        .bind(uid)
        .execute(db)
        .await?
        .rows_affected()
        == 1;
    if ok {
        let _ = remove_dashboard_nav(db, uid, id).await;
    }
    Ok(Ok(ok))
}

// ---- summary migration and sidebar ---------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct NavItem {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
}

async fn raw_rows(db: &SqlitePool, uid: i64) -> anyhow::Result<Vec<Row>> {
    Ok(sqlx::query_as(SELECT).bind(uid).fetch_all(db).await?)
}

async fn prepare_user(db: &SqlitePool, uid: i64) -> anyhow::Result<()> {
    let rows = raw_rows(db, uid).await?;
    if rows.is_empty() {
        // No dashboard row yet. Still import an existing pin list — that was
        // the old Summary — then leave the board blank when there are no pins.
        let id = create_with(db, uid, "Summary", &[], Some("summary")).await?;
        import_pins_once(db, uid, id).await?;
        save_nav(db, uid, &default_nav(&[])).await?;
        return Ok(());
    }
    let mut summary_id: Option<i64> = rows
        .iter()
        .find(|r| r.4.as_deref() == Some("summary"))
        .map(|r| r.0);
    if summary_id.is_none() {
        if let Some(seed) = rows.iter().find(|r| {
            let widgets = serde_json::from_str::<Vec<Widget>>(&r.3).unwrap_or_default();
            is_untouched_seed(&r.1, &widgets, &r.5, &r.6)
        }) {
            sqlx::query(
                "UPDATE dashboards SET widgets_json = '[]', builtin = 'summary',
                 updated_at = created_at WHERE id = ? AND user_id = ?",
            )
            .bind(seed.0)
            .bind(uid)
            .execute(db)
            .await?;
            summary_id = Some(seed.0);
        } else if let Some(named) = rows.iter().find(|r| r.1 == "Summary") {
            sqlx::query("UPDATE dashboards SET builtin = 'summary' WHERE id = ? AND user_id = ?")
                .bind(named.0)
                .bind(uid)
                .execute(db)
                .await?;
            summary_id = Some(named.0);
        } else {
            summary_id = Some(create_with(db, uid, "Summary", &[], Some("summary")).await?);
        }
    }
    if let Some(id) = summary_id {
        import_pins_once(db, uid, id).await?;
    }
    let custom: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM dashboards WHERE user_id = ? AND builtin IS NULL ORDER BY position, id",
    )
    .bind(uid)
    .fetch_all(db)
    .await?;
    reconcile_nav(db, uid, &custom).await?;
    Ok(())
}

/// Copy stored pins onto Summary once.
///
/// Pins are the old Health Summary customization. They are appended when that
/// metric is not already a card, including onto a Summary the user already
/// edited. A later empty board is not refilled. Pin rows themselves are never
/// deleted. An untouched seeded layout is cleared by the caller before this
/// runs; a re-saved or renamed copy of that layout is left in place.
async fn import_pins_once(db: &SqlitePool, uid: i64, summary_id: i64) -> anyhow::Result<()> {
    let row: Option<(String, i64)> = sqlx::query_as(
        "SELECT widgets_json, pins_applied FROM dashboards WHERE id = ? AND user_id = ?",
    )
    .bind(summary_id)
    .bind(uid)
    .fetch_optional(db)
    .await?;
    let Some((json, applied)) = row else {
        return Ok(());
    };
    if applied != 0 {
        return Ok(());
    }
    let mut widgets: Vec<Widget> = serde_json::from_str(&json).unwrap_or_default();
    let pins = crate::browse::pinned(db, uid).await?;
    let mut n = widgets.len();
    for metric in &pins {
        if widgets.len() >= MAX_WIDGETS {
            break;
        }
        if widgets
            .iter()
            .any(|w| w.metric.as_deref() == Some(metric.as_str()))
        {
            continue;
        }
        widgets.push(Widget {
            id: format!("pin{n}"),
            kind: "bar".into(),
            metric: Some(metric.clone()),
            title: None,
            days: 14,
            bucket: "day".into(),
            agg: None,
            size: 2,
        });
        n += 1;
    }
    sqlx::query(
        "UPDATE dashboards SET widgets_json = ?, pins_applied = 1 WHERE id = ? AND user_id = ?",
    )
    .bind(serde_json::to_string(&widgets)?)
    .bind(summary_id)
    .bind(uid)
    .execute(db)
    .await?;
    Ok(())
}

fn default_nav(custom_ids: &[i64]) -> Vec<NavItem> {
    let mut items = vec![
        NavItem {
            kind: "summary".into(),
            id: None,
        },
        NavItem {
            kind: "fitness".into(),
            id: None,
        },
        NavItem {
            kind: "health".into(),
            id: None,
        },
    ];
    for id in custom_ids {
        items.push(NavItem {
            kind: "dashboard".into(),
            id: Some(*id),
        });
    }
    items.push(NavItem {
        kind: "add".into(),
        id: None,
    });
    items
}

async fn save_nav(db: &SqlitePool, uid: i64, items: &[NavItem]) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO nav_layout (user_id, items_json) VALUES (?, ?)
         ON CONFLICT(user_id) DO UPDATE SET items_json = excluded.items_json",
    )
    .bind(uid)
    .bind(serde_json::to_string(items)?)
    .execute(db)
    .await?;
    Ok(())
}

async fn reconcile_nav(db: &SqlitePool, uid: i64, custom_ids: &[i64]) -> anyhow::Result<()> {
    let stored: Option<String> =
        sqlx::query_scalar("SELECT items_json FROM nav_layout WHERE user_id = ?")
            .bind(uid)
            .fetch_optional(db)
            .await?;
    let Some(json) = stored else {
        return save_nav(db, uid, &default_nav(custom_ids)).await;
    };
    let parsed: Vec<NavItem> = serde_json::from_str(&json).unwrap_or_default();
    let mut items = Vec::new();
    let mut seen_sum = false;
    let mut seen_fit = false;
    let mut seen_health = false;
    let mut seen_add = false;
    let mut seen_ids = std::collections::BTreeSet::new();
    for item in parsed {
        match item.kind.as_str() {
            "summary" if !seen_sum => {
                seen_sum = true;
                items.push(NavItem {
                    kind: "summary".into(),
                    id: None,
                });
            }
            "fitness" if !seen_fit => {
                seen_fit = true;
                items.push(NavItem {
                    kind: "fitness".into(),
                    id: None,
                });
            }
            "health" if !seen_health => {
                seen_health = true;
                items.push(NavItem {
                    kind: "health".into(),
                    id: None,
                });
            }
            "add" if !seen_add => {
                seen_add = true;
                items.push(NavItem {
                    kind: "add".into(),
                    id: None,
                });
            }
            "dashboard" => {
                if let Some(id) = item.id {
                    if custom_ids.contains(&id) && seen_ids.insert(id) {
                        items.push(NavItem {
                            kind: "dashboard".into(),
                            id: Some(id),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    if !seen_sum {
        items.insert(
            0,
            NavItem {
                kind: "summary".into(),
                id: None,
            },
        );
    }
    if !seen_fit {
        items.insert(
            1.min(items.len()),
            NavItem {
                kind: "fitness".into(),
                id: None,
            },
        );
    }
    if !seen_health {
        items.insert(
            2.min(items.len()),
            NavItem {
                kind: "health".into(),
                id: None,
            },
        );
    }
    for id in custom_ids {
        if !seen_ids.contains(id) {
            let at = items
                .iter()
                .position(|i| i.kind == "add")
                .unwrap_or(items.len());
            items.insert(
                at,
                NavItem {
                    kind: "dashboard".into(),
                    id: Some(*id),
                },
            );
        }
    }
    if !seen_add {
        items.push(NavItem {
            kind: "add".into(),
            id: None,
        });
    }
    save_nav(db, uid, &items).await
}

pub async fn nav(db: &SqlitePool, uid: i64) -> anyhow::Result<Vec<NavItem>> {
    prepare_user(db, uid).await?;
    let json: String = sqlx::query_scalar("SELECT items_json FROM nav_layout WHERE user_id = ?")
        .bind(uid)
        .fetch_one(db)
        .await?;
    Ok(serde_json::from_str(&json).unwrap_or_else(|_| default_nav(&[])))
}

pub fn validate_nav(items: &[NavItem], custom_ids: &[i64]) -> Result<(), &'static str> {
    if items.len() > 80 {
        return Err("invalid_nav");
    }
    let mut sum = 0;
    let mut fit = 0;
    let mut health = 0;
    let mut add = 0;
    let mut ids = std::collections::BTreeSet::new();
    for item in items {
        match item.kind.as_str() {
            "summary" => sum += 1,
            "fitness" => fit += 1,
            "health" => health += 1,
            "add" => add += 1,
            "dashboard" => {
                let id = item.id.ok_or("invalid_nav")?;
                if !custom_ids.contains(&id) || !ids.insert(id) {
                    return Err("invalid_nav");
                }
            }
            _ => return Err("invalid_nav"),
        }
    }
    if sum != 1 || fit != 1 || health != 1 || add != 1 || ids.len() != custom_ids.len() {
        return Err("invalid_nav");
    }
    Ok(())
}

pub async fn set_nav(
    db: &SqlitePool,
    uid: i64,
    items: &[NavItem],
) -> anyhow::Result<Result<(), &'static str>> {
    prepare_user(db, uid).await?;
    let custom: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM dashboards WHERE user_id = ? AND builtin IS NULL ORDER BY position, id",
    )
    .bind(uid)
    .fetch_all(db)
    .await?;
    if let Err(code) = validate_nav(items, &custom) {
        return Ok(Err(code));
    }
    save_nav(db, uid, items).await?;
    Ok(Ok(()))
}

pub async fn insert_dashboard_nav(db: &SqlitePool, uid: i64, id: i64) -> anyhow::Result<()> {
    let mut items = nav(db, uid).await?;
    if items
        .iter()
        .any(|i| i.kind == "dashboard" && i.id == Some(id))
    {
        return Ok(());
    }
    let at = items
        .iter()
        .position(|i| i.kind == "add")
        .unwrap_or(items.len());
    items.insert(
        at,
        NavItem {
            kind: "dashboard".into(),
            id: Some(id),
        },
    );
    save_nav(db, uid, &items).await
}

async fn remove_dashboard_nav(db: &SqlitePool, uid: i64, id: i64) -> anyhow::Result<()> {
    let items: Vec<NavItem> = nav(db, uid)
        .await?
        .into_iter()
        .filter(|i| !(i.kind == "dashboard" && i.id == Some(id)))
        .collect();
    save_nav(db, uid, &items).await
}

// ---- metrics -------------------------------------------------------------

/// Metrics where a day's value is the total (otherwise we average).
const ADDITIVE: &[&str] = &[
    "StepCount",
    "ActiveEnergyBurned",
    "BasalEnergyBurned",
    "AppleExerciseTime",
    "AppleStandTime",
    "AppleMoveTime",
    "FlightsClimbed",
    "DistanceWalkingRunning",
    "DistanceCycling",
    "DistanceSwimming",
    "DistanceWheelchair",
    "DistanceDownhillSnowSports",
    "SwimmingStrokeCount",
    "PushCount",
    "TimeInDaylight",
    "NumberOfTimesFallen",
    "Dietary",
];

/// Default aggregation for a metric type: sum, avg, or count (categories, workouts).
pub fn default_agg(kind: &str) -> &'static str {
    // Sleep Analysis: sum stage durations (minutes). Other categories stay count
    // unless they are workouts (sum duration).
    if kind == crate::stats::SLEEP {
        return "sum";
    }
    if kind.starts_with("HKCategoryTypeIdentifier") || kind.starts_with("HKWorkoutActivityType") {
        return if kind.starts_with("HKWorkout") {
            "sum"
        } else {
            "count"
        };
    }
    let base = kind
        .trim_start_matches("HKQuantityTypeIdentifier")
        .trim_start_matches("healthvibe.hae.");
    let lower = base.to_ascii_lowercase();
    if ADDITIVE.iter().any(|a| base.starts_with(a))
        || [
            "step", "distance", "energy", "flights", "dietary", "exercise", "daylight", "count",
        ]
        .iter()
        .any(|w| lower.contains(w))
            && !lower.contains("rate")
    {
        "sum"
    } else {
        "avg"
    }
}

/// Human name from a type id: "HKQuantityTypeIdentifierRestingHeartRate" -> "Resting Heart Rate".
pub fn display_name(kind: &str) -> String {
    let base = [
        "HKQuantityTypeIdentifier",
        "HKCategoryTypeIdentifier",
        "healthvibe.hae.",
    ]
    .iter()
    .find_map(|p| kind.strip_prefix(p));
    if let Some(w) = kind.strip_prefix("HKWorkoutActivityType") {
        return format!("Workout: {}", split_words(w));
    }
    match base {
        Some(b) if kind.starts_with("healthvibe.hae.") => b
            .split('_')
            .filter(|s| !s.is_empty())
            .map(|s| {
                let mut c = s.chars();
                c.next()
                    .map(|f| f.to_uppercase().chain(c).collect::<String>())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" "),
        Some(b) => split_words(b),
        None => kind.to_string(),
    }
}

fn split_words(s: &str) -> String {
    let mut out = String::new();
    let cs: Vec<char> = s.chars().collect();
    for (i, &c) in cs.iter().enumerate() {
        if i > 0
            && c.is_uppercase()
            && (cs[i - 1].is_lowercase() || cs.get(i + 1).is_some_and(|n| n.is_lowercase()))
        {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

#[derive(Debug, Serialize)]
pub struct MetricInfo {
    pub metric: String,
    pub name: String,
    pub unit: Option<String>,
    pub count: i64,
    pub first: Option<String>,
    pub last: Option<String>,
    pub agg: &'static str,
}

/// Metrics this user actually has, with the most common unit and counts.
pub async fn metrics(db: &SqlitePool, uid: i64) -> anyhow::Result<Vec<MetricInfo>> {
    type MetricRow = (String, Option<String>, i64, Option<String>, Option<String>);
    let rows: Vec<MetricRow> = sqlx::query_as(
        "SELECT type,
           (SELECT unit FROM readings r2 WHERE r2.user_id = r.user_id AND r2.type = r.type AND unit IS NOT NULL
            GROUP BY unit ORDER BY COUNT(*) DESC LIMIT 1),
           COUNT(*), substr(MIN(local_start), 1, 10), substr(MAX(local_start), 1, 10)
         FROM readings r WHERE user_id = ? GROUP BY type ORDER BY type",
    ).bind(uid).fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .map(|(metric, unit, count, first, last)| MetricInfo {
            name: display_name(&metric),
            agg: default_agg(&metric),
            metric,
            unit,
            count,
            first,
            last,
        })
        .collect())
}

fn shift(date: &str, days: i64) -> String {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|d| (d + chrono::Duration::days(days)).to_string())
        .unwrap_or_else(|_| date.to_string())
}

/// Daily values for any metric; `agg` is sum (best source per day), avg, or count.
pub async fn daily(
    db: &SqlitePool,
    uid: i64,
    kind: &str,
    agg: &str,
    end: &str,
    days: i64,
) -> anyhow::Result<Vec<DayValue>> {
    // Sleep: sum of asleep minutes per night (local end date), never sample count.
    if kind == stats::SLEEP {
        let s = stats::sleep(db, uid, end, days).await?;
        return Ok(s
            .nights
            .into_iter()
            .map(|n| DayValue {
                date: n.date,
                value: n.asleep_min,
            })
            .collect());
    }
    match agg {
        "sum" => stats::daily_sum(db, uid, kind, end, days).await,
        "avg" => stats::daily_avg(db, uid, kind, end, days).await,
        _ => {
            let rows: Vec<(String, f64)> = sqlx::query_as(
                "SELECT substr(local_start, 1, 10) AS d, CAST(COUNT(*) AS REAL) FROM readings
                 WHERE user_id = ? AND type = ? AND local_start >= ? AND local_start < ?
                 GROUP BY d ORDER BY d",
            )
            .bind(uid)
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
    }
}

/// Groups daily values into weeks (starting Monday) or months. Sums and
/// counts add up; averages are the mean of the daily values.
pub fn bucketize(daily: Vec<DayValue>, bucket: &str, agg: &str) -> Vec<DayValue> {
    if bucket == "day" {
        return daily;
    }
    let mut m: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    for d in daily {
        let Ok(date) = chrono::NaiveDate::parse_from_str(&d.date, "%Y-%m-%d") else {
            continue;
        };
        let key = if bucket == "week" {
            use chrono::Datelike;
            (date - chrono::Duration::days(date.weekday().num_days_from_monday() as i64))
                .to_string()
        } else {
            format!("{}-01", &d.date[..7])
        };
        let e = m.entry(key).or_insert((0.0, 0.0));
        e.0 += d.value;
        e.1 += 1.0;
    }
    m.into_iter()
        .map(|(date, (s, n))| DayValue {
            date,
            value: if agg == "avg" { s / n } else { s },
        })
        .collect()
}

pub async fn series(
    db: &SqlitePool,
    uid: i64,
    kind: &str,
    agg: &str,
    bucket: &str,
    end: Option<String>,
    days: i64,
) -> anyhow::Result<Value> {
    let unit: Option<String> = sqlx::query_scalar(
        "SELECT unit FROM readings WHERE user_id = ? AND type = ? AND unit IS NOT NULL GROUP BY unit ORDER BY COUNT(*) DESC LIMIT 1",
    ).bind(uid).bind(kind).fetch_optional(db).await?.flatten();
    let end = match end {
        Some(e) => Some(e),
        None => sqlx::query_scalar::<_, Option<String>>(
            "SELECT substr(MAX(local_start), 1, 10) FROM readings WHERE user_id = ? AND type = ?",
        )
        .bind(uid)
        .bind(kind)
        .fetch_one(db)
        .await?,
    };
    let points = match &end {
        Some(e) => bucketize(daily(db, uid, kind, agg, e, days).await?, bucket, agg),
        None => vec![],
    };
    Ok(
        json!({ "metric": kind, "name": display_name(kind), "unit": if agg == "count" { Some("count".into()) } else { unit },
               "agg": agg, "bucket": bucket, "end": end, "points": points }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_and_aggs() {
        assert_eq!(
            display_name("HKQuantityTypeIdentifierRestingHeartRate"),
            "Resting Heart Rate"
        );
        assert_eq!(
            display_name("healthvibe.hae.tooth_brushing"),
            "Tooth Brushing"
        );
        assert_eq!(default_agg(stats::STEPS), "sum");
        assert_eq!(default_agg(stats::RESTING_HR), "avg");
        assert_eq!(default_agg(stats::WEIGHT), "avg");
        assert_eq!(
            default_agg("HKQuantityTypeIdentifierDietaryEnergyConsumed"),
            "sum"
        );
        assert_eq!(
            default_agg("HKQuantityTypeIdentifierWalkingHeartRateAverage"),
            "avg"
        );
        assert_eq!(default_agg(stats::SLEEP), "sum");
    }
    #[test]
    fn buckets() {
        let d = |date: &str, value| DayValue {
            date: date.into(),
            value,
        };
        let v = vec![
            d("2026-01-05", 1.0),
            d("2026-01-06", 3.0),
            d("2026-01-12", 5.0),
        ];
        let w = bucketize(v.clone(), "week", "sum");
        assert_eq!(w, vec![d("2026-01-05", 4.0), d("2026-01-12", 5.0)]);
        assert_eq!(bucketize(v, "month", "avg"), vec![d("2026-01-01", 3.0)]);
    }
}
