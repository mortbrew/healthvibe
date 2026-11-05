//! Apple Health `export.zip` import.
//!
//! The zip is read from disk (never fully into memory); `export.xml` is
//! streamed through quick-xml on a blocking thread and handed to the database
//! in batches. Every row is tagged `channel = export_xml`. Re-sents upsert on
//! the natural key `(user_id, type, channel, source, start_utc, end_utc)`.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Seek};
use std::path::Path;

const BATCH: usize = 2000;

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub kind: String,
    pub value: Option<String>,
    pub unit: Option<String>,
    pub start_utc: String,
    pub end_utc: String,
    pub tz_offset: Option<String>,
    pub local_start: String,
    pub local_end: String,
    pub source: Option<String>,
    pub hash: String,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TypeCount {
    pub inserted: u64,
    pub duplicates: u64,
}

/// One day's Activity ring goals (`<ActivitySummary>`).
#[derive(Debug, Clone, PartialEq)]
pub struct Goal {
    pub date: String,
    pub move_kcal: Option<f64>,
    pub exercise_min: Option<f64>,
    pub stand_hours: Option<f64>,
}

/// Something parsed out of export.xml.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Row(Row),
    Goal(Goal),
}

pub const ACTIVITY_SUMMARY: &str = "ActivitySummary";

fn to_goal(a: &BTreeMap<String, String>) -> Option<Goal> {
    let date = a.get("dateComponents")?;
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let num = |k: &str| {
        a.get(k)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| *v > 0.0)
    };
    let g = Goal {
        date: date.clone(),
        move_kcal: num("activeEnergyBurnedGoal"),
        exercise_min: num("appleExerciseTimeGoal"),
        stand_hours: num("appleStandHoursGoal"),
    };
    (g.move_kcal.is_some() || g.exercise_min.is_some() || g.stand_hours.is_some()).then_some(g)
}

pub type Counts = BTreeMap<String, TypeCount>;

/// Apple dates look like `2024-01-02 07:30:00 -0500`.
/// Returns (UTC RFC3339, original offset, local wall-clock time).
fn parse_date(s: &str) -> Option<(String, String, String)> {
    let dt = chrono::DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S %z").ok()?;
    let off = s.rsplit(' ').next().unwrap_or("").to_string();
    Some((
        dt.with_timezone(&chrono::Utc)
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string(),
        off,
        dt.format("%Y-%m-%d %H:%M:%S").to_string(),
    ))
}

fn attrs(e: &BytesStart) -> BTreeMap<String, String> {
    e.attributes()
        .flatten()
        .filter_map(|a| {
            let k = String::from_utf8_lossy(a.key.as_ref()).into_owned();
            let v = a.unescape_value().ok()?.into_owned();
            Some((k, v))
        })
        .collect()
}

fn to_row(tag: &[u8], a: &BTreeMap<String, String>) -> Option<Row> {
    let (kind, value, unit) = match tag {
        b"Record" => (
            a.get("type")?.clone(),
            a.get("value").cloned(),
            a.get("unit").cloned(),
        ),
        b"Workout" => (
            a.get("workoutActivityType")?.clone(),
            a.get("duration").cloned(),
            a.get("durationUnit").cloned(),
        ),
        _ => return None,
    };
    let (start_utc, off, local_start) = parse_date(a.get("startDate")?)?;
    let (end_utc, _, local_end) = parse_date(a.get("endDate")?)?;
    let source = a.get("sourceName").cloned().unwrap_or_default();
    let hash = crate::channel::reading_hash(
        crate::channel::EXPORT_XML,
        &kind,
        &source,
        &start_utc,
        &end_utc,
    );
    Some(Row {
        hash,
        kind,
        value,
        unit,
        start_utc,
        end_utc,
        tz_offset: Some(off),
        local_start,
        local_end,
        source: Some(source).filter(|s| !s.is_empty()),
    })
}

/// Stream-parse export.xml, calling `sink` with batches of rows.
pub fn parse_export<R: BufRead>(
    r: R,
    mut sink: impl FnMut(Vec<Item>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut reader = Reader::from_reader(r);
    let mut buf = Vec::with_capacity(4096);
    let mut batch = Vec::with_capacity(BATCH);
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) => {
                let name = e.name();
                let a = attrs(&e);
                let item = if name.as_ref() == b"ActivitySummary" {
                    to_goal(&a).map(Item::Goal)
                } else {
                    to_row(name.as_ref(), &a).map(Item::Row)
                };
                if let Some(item) = item {
                    batch.push(item);
                    if batch.len() >= BATCH {
                        sink(std::mem::replace(&mut batch, Vec::with_capacity(BATCH)))?;
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    if !batch.is_empty() {
        sink(batch)?;
    }
    Ok(())
}

/// Find export.xml inside the zip (not export_cda.xml) and stream it.
pub fn parse_zip<R: Read + Seek>(
    r: R,
    sink: impl FnMut(Vec<Item>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut zip = zip::ZipArchive::new(r)?;
    let idx = (0..zip.len())
        .find(|&i| {
            zip.name_for_index(i)
                .map(|n| n == "export.xml" || n.ends_with("/export.xml"))
                .unwrap_or(false)
        })
        .ok_or_else(|| anyhow::anyhow!("no export.xml found in the zip"))?;
    let f = zip.by_index(idx)?;
    parse_export(BufReader::with_capacity(1 << 20, f), sink)
}

async fn insert_batch(
    db: &SqlitePool,
    user_id: i64,
    items: Vec<Item>,
    counts: &mut Counts,
) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;
    for item in items {
        let r = match item {
            Item::Row(r) => r,
            Item::Goal(g) => {
                let res = sqlx::query(
                    "INSERT INTO activity_goals (user_id, date, move_kcal, exercise_min, stand_hours)
                     VALUES (?, ?, ?, ?, ?)
                     ON CONFLICT (user_id, date) DO UPDATE SET
                       move_kcal = excluded.move_kcal,
                       exercise_min = excluded.exercise_min,
                       stand_hours = excluded.stand_hours
                     WHERE move_kcal IS NOT excluded.move_kcal
                        OR exercise_min IS NOT excluded.exercise_min
                        OR stand_hours IS NOT excluded.stand_hours",
                )
                .bind(user_id)
                .bind(&g.date)
                .bind(g.move_kcal)
                .bind(g.exercise_min)
                .bind(g.stand_hours)
                .execute(&mut *tx)
                .await?;
                let c = counts.entry(ACTIVITY_SUMMARY.to_string()).or_default();
                if res.rows_affected() == 1 {
                    c.inserted += 1;
                } else {
                    c.duplicates += 1;
                }
                continue;
            }
        };
        let source = r.source.clone().unwrap_or_default();
        let res = sqlx::query(
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
        .bind(&r.kind)
        .bind(&r.value)
        .bind(&r.unit)
        .bind(&r.start_utc)
        .bind(&r.end_utc)
        .bind(&r.tz_offset)
        .bind(&source)
        .bind(crate::channel::EXPORT_XML)
        .bind(&r.hash)
        .bind(&r.local_start)
        .bind(&r.local_end)
        .execute(&mut *tx)
        .await?;
        let c = counts.entry(r.kind).or_default();
        // rows_affected: 1 = insert, 2 = update (SQLite), 0 = unchanged conflict.
        if res.rows_affected() == 1 {
            c.inserted += 1;
        } else {
            c.duplicates += 1;
        }
    }
    tx.commit().await?;
    Ok(())
}

/// Import a zip file on disk for a user. Returns per-type counts.
pub async fn import_zip(db: &SqlitePool, user_id: i64, path: &Path) -> anyhow::Result<Counts> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<Item>>(4);
    let path = path.to_path_buf();
    let parser = tokio::task::spawn_blocking(move || {
        let f = std::fs::File::open(path)?;
        parse_zip(BufReader::new(f), |b| {
            tx.blocking_send(b)
                .map_err(|_| anyhow::anyhow!("import aborted"))
        })
    });
    let mut counts = Counts::new();
    let mut insert_err = None;
    while let Some(batch) = rx.recv().await {
        if let Err(e) = insert_batch(db, user_id, batch, &mut counts).await {
            insert_err = Some(e);
            break;
        }
    }
    drop(rx);
    if let Some(e) = insert_err {
        return Err(e);
    }
    parser.await??;
    Ok(counts)
}

/// Run an import job: record progress in import_jobs and always delete the
/// staged zip afterwards (success or failure).
pub async fn run_job(db: &SqlitePool, job_id: i64, user_id: i64, path: &Path) {
    let result = import_zip(db, user_id, path).await;
    let _ = std::fs::remove_file(path);
    let now = chrono::Utc::now().to_rfc3339();
    let q = match &result {
        Ok(c) => sqlx::query(
            "UPDATE import_jobs SET status='done', counts_json=?, finished_at=? WHERE id=?",
        )
        .bind(serde_json::to_string(c).unwrap_or_default()),
        Err(e) => {
            tracing::warn!(job_id, error = %e, "import failed");
            sqlx::query("UPDATE import_jobs SET status='failed', error=?, finished_at=? WHERE id=?")
                .bind(e.to_string())
        }
    };
    let _ = q.bind(now).bind(job_id).execute(db).await;
    tracing::info!(job_id, ok = result.is_ok(), "import finished");
}

pub async fn create_job(db: &SqlitePool, user_id: i64) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "INSERT INTO import_jobs (user_id, status) VALUES (?, 'running') RETURNING id",
    )
    .bind(user_id)
    .fetch_one(db)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    pub const FIXTURE: &str = include_str!("../tests/fixtures/export.xml");

    fn fixture_zip() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("hv-test-{}.zip", rand::random::<u64>()));
        let mut z = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("apple_health_export/export_cda.xml", o)
            .unwrap();
        z.write_all(b"<ClinicalDocument/>").unwrap();
        z.start_file("apple_health_export/export.xml", o).unwrap();
        z.write_all(FIXTURE.as_bytes()).unwrap();
        z.finish().unwrap();
        p
    }

    #[test]
    fn parses_records_and_workouts() {
        let mut rows = vec![];
        parse_export(FIXTURE.as_bytes(), |b| {
            rows.extend(b);
            Ok(())
        })
        .unwrap();
        let goals: Vec<Goal> = rows
            .iter()
            .filter_map(|i| match i {
                Item::Goal(g) => Some(g.clone()),
                _ => None,
            })
            .collect();
        let rows: Vec<Row> = rows
            .into_iter()
            .filter_map(|i| match i {
                Item::Row(r) => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(
            goals,
            vec![Goal {
                date: "2024-01-02".into(),
                move_kcal: Some(650.0),
                exercise_min: Some(45.0),
                stand_hours: Some(10.0),
            }],
            "summary without goals is skipped"
        );
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[0].kind, "HKQuantityTypeIdentifierStepCount");
        assert_eq!(rows[0].start_utc, "2024-01-02T12:00:00Z");
        assert_eq!(rows[0].tz_offset.as_deref(), Some("-0500"));
        assert!(rows
            .iter()
            .any(|r| r.kind == "HKWorkoutActivityTypeRunning"));
    }

    #[tokio::test]
    async fn import_dedupes_and_counts() {
        let db = crate::db::open_memory().await.unwrap();
        let p = fixture_zip();
        let c = import_zip(&db, 1, &p).await.unwrap();
        let steps = &c["HKQuantityTypeIdentifierStepCount"];
        // fixture contains one exact duplicate step record
        assert_eq!((steps.inserted, steps.duplicates), (2, 1));
        let again = import_zip(&db, 1, &p).await.unwrap();
        assert!(again.values().all(|t| t.inserted == 0));
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM readings WHERE user_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(n, 5);
        let g: (f64, f64, f64) = sqlx::query_as(
            "SELECT move_kcal, exercise_min, stand_hours FROM activity_goals
             WHERE user_id = 1 AND date = '2024-01-02'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(g, (650.0, 45.0, 10.0));
        let job = create_job(&db, 1).await.unwrap();
        run_job(&db, job, 1, &p).await;
        assert!(!p.exists(), "staged zip deleted after import");
    }
}
