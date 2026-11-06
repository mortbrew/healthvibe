//! Ingest channel tagging and day-level source selection.
//!
//! Every reading carries:
//! - `channel`: how it arrived (`export_xml`, `health_auto_export`, `shortcut`,
//!   `demo`, or `unknown` for rows written before this existed)
//! - `source`: device or app name when the payload had one (Watch, Phone, …)
//!
//! ## One channel per metric per day
//!
//! For additive daily totals (steps, energy, …) we never sum across channels.
//! Preference order (lower rank wins):
//!
//! 1. `export_xml` — authoritative for any local day the last Apple export
//!    covers. Preferring it stops HAE re-syncs of the same closed day from
//!    double-counting.
//! 2. `health_auto_export` — ongoing sync for days after (or without) an
//!    export.xml import.
//! 3. `shortcut` — the Shortcuts path, when used.
//! 4. `demo` — synthetic seed data.
//! 5. `unknown` — legacy rows.
//!
//! Within the chosen channel, additive metrics still take the device `source`
//! with the highest daily total (Watch vs Phone), matching Apple's usual
//! priority stand-in.
//!
//! Partial HAE batches only upsert the metrics they contain; they never delete
//! or null out other metrics for that day.

pub const EXPORT_XML: &str = "export_xml";
pub const HEALTH_AUTO_EXPORT: &str = "health_auto_export";
pub const SHORTCUT: &str = "shortcut";
pub const DEMO: &str = "demo";
pub const UNKNOWN: &str = "unknown";

/// Sort key for channel preference (1 = best). Unknown channels sort last.
pub fn rank(channel: &str) -> i64 {
    match channel {
        EXPORT_XML => 1,
        HEALTH_AUTO_EXPORT => 2,
        SHORTCUT => 3,
        DEMO => 4,
        _ => 5,
    }
}

/// SQL `CASE` expression that yields [`rank`] for a `channel` column.
pub const RANK_SQL: &str = "CASE channel \
    WHEN 'export_xml' THEN 1 \
    WHEN 'health_auto_export' THEN 2 \
    WHEN 'shortcut' THEN 3 \
    WHEN 'demo' THEN 4 \
    ELSE 5 END";

/// Natural-key content hash (value is intentionally excluded so a corrected
/// re-send updates the same row).
pub fn reading_hash(
    channel: &str,
    kind: &str,
    source: &str,
    start_utc: &str,
    end_utc: &str,
) -> String {
    crate::auth::sha256_hex(&format!(
        "{channel}\x1f{kind}\x1f{source}\x1f{start_utc}\x1f{end_utc}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_order() {
        assert!(rank(EXPORT_XML) < rank(HEALTH_AUTO_EXPORT));
        assert!(rank(HEALTH_AUTO_EXPORT) < rank(SHORTCUT));
        assert!(rank(SHORTCUT) < rank(DEMO));
        assert!(rank(DEMO) < rank(UNKNOWN));
        assert_eq!(rank("weird"), rank(UNKNOWN));
    }

    #[test]
    fn hash_ignores_value() {
        let a = reading_hash(EXPORT_XML, "T", "Watch", "a", "b");
        let b = reading_hash(EXPORT_XML, "T", "Watch", "a", "b");
        assert_eq!(a, b);
        assert_ne!(a, reading_hash(HEALTH_AUTO_EXPORT, "T", "Watch", "a", "b"));
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use crate::db::open_memory;
    use sqlx::SqlitePool;

    #[allow(clippy::too_many_arguments)]
    async fn add(
        db: &SqlitePool,
        u: i64,
        kind: &str,
        channel: &str,
        source: &str,
        day: &str,
        value: &str,
        n: u32,
    ) {
        // Distinct start/end per `n` so natural keys do not collide.
        let start = format!("{day} 08:{:02}:00", n);
        let end = format!("{day} 08:{:02}:00", n + 1);
        let su = format!("{day}T12:{:02}:00Z", n);
        let eu = format!("{day}T12:{:02}:00Z", n + 1);
        let hash = reading_hash(channel, kind, source, &su, &eu);
        sqlx::query(
            "INSERT INTO readings
             (user_id, type, value, unit, start_utc, end_utc, source, channel, hash,
              local_start, local_end)
             VALUES (?, ?, ?, 'count', ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(u)
        .bind(kind)
        .bind(value)
        .bind(&su)
        .bind(&eu)
        .bind(source)
        .bind(channel)
        .bind(hash)
        .bind(&start)
        .bind(&end)
        .execute(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn daily_sum_prefers_export_xml_over_hae() {
        let db = open_memory().await.unwrap();
        let steps = "HKQuantityTypeIdentifierStepCount";
        // Same day: HAE Watch 5000, export.xml Watch 8000 → use export.
        add(
            &db,
            1,
            steps,
            HEALTH_AUTO_EXPORT,
            "Watch",
            "2024-06-01",
            "2500",
            0,
        )
        .await;
        add(
            &db,
            1,
            steps,
            HEALTH_AUTO_EXPORT,
            "Watch",
            "2024-06-01",
            "2500",
            1,
        )
        .await;
        add(&db, 1, steps, EXPORT_XML, "Watch", "2024-06-01", "4000", 2).await;
        add(&db, 1, steps, EXPORT_XML, "Watch", "2024-06-01", "4000", 3).await;
        // Phone export would be lower total; still prefer export channel's max source.
        add(&db, 1, steps, EXPORT_XML, "Phone", "2024-06-01", "100", 4).await;
        let days = crate::stats::daily_sum(&db, 1, steps, "2024-06-01", 1)
            .await
            .unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].value, 8000.0);
    }

    #[tokio::test]
    async fn daily_sum_uses_hae_when_no_export_that_day() {
        let db = open_memory().await.unwrap();
        let steps = "HKQuantityTypeIdentifierStepCount";
        add(
            &db,
            1,
            steps,
            HEALTH_AUTO_EXPORT,
            "Watch",
            "2024-06-02",
            "3000",
            0,
        )
        .await;
        // Export covers a different day only.
        add(&db, 1, steps, EXPORT_XML, "Watch", "2024-06-01", "9000", 1).await;
        let days = crate::stats::daily_sum(&db, 1, steps, "2024-06-02", 1)
            .await
            .unwrap();
        assert_eq!(days[0].value, 3000.0);
    }

    #[tokio::test]
    async fn natural_key_upsert_updates_value() {
        let db = open_memory().await.unwrap();
        let body1 = br#"{"data":{"metrics":[{"name":"step_count","units":"count","data":[
            {"date":"2024-06-01 08:00:00 -0400","qty":100,"source":"Watch"}
        ]}]}}"#;
        let body2 = br#"{"data":{"metrics":[{"name":"step_count","units":"count","data":[
            {"date":"2024-06-01 08:00:00 -0400","qty":150,"source":"Watch"}
        ]}]}}"#;
        let r1 = crate::hae::ingest(&db, 1, body1).await.unwrap().unwrap();
        assert_eq!(r1.accepted, 1);
        let r2 = crate::hae::ingest(&db, 1, body2).await.unwrap().unwrap();
        assert_eq!(r2.updated, 1);
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM readings WHERE user_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(n, 1);
        let v: String = sqlx::query_scalar("SELECT value FROM readings WHERE user_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(v, "150");
    }

    #[tokio::test]
    async fn partial_hae_batch_does_not_wipe_other_metrics() {
        let db = open_memory().await.unwrap();
        let steps = br#"{"data":{"metrics":[{"name":"step_count","units":"count","data":[
            {"date":"2024-06-01 08:00:00 -0400","qty":1000,"source":"Watch"}
        ]}]}}"#;
        let hr = br#"{"data":{"metrics":[{"name":"heart_rate","units":"count/min","data":[
            {"date":"2024-06-01 09:00:00 -0400","qty":72,"source":"Watch"}
        ]}]}}"#;
        crate::hae::ingest(&db, 1, steps).await.unwrap().unwrap();
        crate::hae::ingest(&db, 1, hr).await.unwrap().unwrap();
        // Re-send steps only — heart rate must remain.
        let steps2 = br#"{"data":{"metrics":[{"name":"step_count","units":"count","data":[
            {"date":"2024-06-01 08:00:00 -0400","qty":1100,"source":"Watch"}
        ]}]}}"#;
        crate::hae::ingest(&db, 1, steps2).await.unwrap().unwrap();
        let kinds: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT type FROM readings WHERE user_id = 1 ORDER BY type",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert!(kinds.iter().any(|k| k.contains("StepCount")));
        assert!(kinds.iter().any(|k| k.contains("HeartRate")));
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM readings WHERE user_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(n, 2);
    }

    #[tokio::test]
    async fn activity_goals_prefer_one_channel_per_day() {
        let db = open_memory().await.unwrap();
        sqlx::query(
            "INSERT INTO activity_goals (user_id, date, channel, move_kcal, exercise_min, stand_hours) VALUES
             (1, '2024-06-01', 'demo', 500, 30, 12),
             (1, '2024-06-01', 'export_xml', 650, NULL, 10),
             (1, '2024-06-01', 'health_auto_export', 700, 45, 11),
             (1, '2024-06-02', 'unknown', 400, 20, 8),
             (1, '2024-06-02', 'shortcut', 420, 25, 9),
             (1, '2024-06-03', 'unknown', 300, 10, 6)",
        )
        .execute(&db)
        .await
        .unwrap();
        let rows: Vec<(String, String, Option<f64>, Option<f64>)> = sqlx::query_as(
            "SELECT date, channel, move_kcal, exercise_min FROM activity_goals_best
             WHERE user_id = 1 ORDER BY date",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        // Whole row from the best channel; a NULL field is not filled from another channel.
        assert_eq!(
            rows,
            vec![
                ("2024-06-01".into(), "export_xml".into(), Some(650.0), None),
                (
                    "2024-06-02".into(),
                    "shortcut".into(),
                    Some(420.0),
                    Some(25.0)
                ),
                (
                    "2024-06-03".into(),
                    "unknown".into(),
                    Some(300.0),
                    Some(10.0)
                ),
            ]
        );
    }

    #[tokio::test]
    async fn activity_goals_view_matches_rank() {
        let db = open_memory().await.unwrap();
        let chans = [
            EXPORT_XML,
            HEALTH_AUTO_EXPORT,
            SHORTCUT,
            DEMO,
            UNKNOWN,
            "other",
        ];
        for (i, a) in chans.iter().enumerate() {
            for b in &chans[i + 1..] {
                let day = format!("2024-01-{:02}", 1 + (rank(a) * 6 + rank(b)) % 28);
                sqlx::query("DELETE FROM activity_goals")
                    .execute(&db)
                    .await
                    .unwrap();
                for c in [a, b] {
                    sqlx::query("INSERT INTO activity_goals (user_id, date, channel, move_kcal) VALUES (1, ?, ?, 1)")
                        .bind(&day)
                        .bind(c)
                        .execute(&db)
                        .await
                        .unwrap();
                }
                let got: String =
                    sqlx::query_scalar("SELECT channel FROM activity_goals_best WHERE user_id = 1")
                        .fetch_one(&db)
                        .await
                        .unwrap();
                let want = if rank(a) <= rank(b) { a } else { b };
                assert_eq!(rank(&got), rank(want), "{a} vs {b}");
            }
        }
    }
}
