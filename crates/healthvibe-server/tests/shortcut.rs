//! iPhone Shortcut path: metrics first, setup code → token, config,
//! ingest (dedupe, not-enabled metrics, isolation, raw body cleanup).

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use healthvibe_core::config::Config;
use healthvibe_server::{router, AppState};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn app() -> (Router, sqlx::SqlitePool) {
    let db = healthvibe_core::db::open_memory().await.unwrap();
    let mut config = Config::from_env();
    config.web_dir = std::env::temp_dir().join("hv-no-web");
    config.public_url = Some("https://health.example.test".into());
    config.max_request_bytes = 64 * 1024;
    (
        router(AppState {
            config,
            db: db.clone(),
        }),
        db,
    )
}

enum Auth<'a> {
    None,
    Cookie(&'a str),
    Bearer(&'a str),
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    auth: Auth<'_>,
    body: Option<String>,
) -> (StatusCode, Value, Option<String>) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("x-forwarded-for", "203.0.113.7");
    match auth {
        Auth::Cookie(c) => req = req.header(header::COOKIE, c),
        Auth::Bearer(t) => req = req.header(header::AUTHORIZATION, format!("Bearer {t}")),
        Auth::None => {}
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b)),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string());
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        cookie,
    )
}

async fn admin(app: &Router) -> String {
    let (s, _, c) = call(
        app,
        "POST",
        "/api/v1/setup",
        Auth::None,
        Some(json!({"username": "toby", "password": "admin password 1"}).to_string()),
    )
    .await;
    assert_eq!(s, 200);
    c.unwrap()
}

async fn pair(app: &Router, cookie: &str, enabled: Value) -> String {
    let (s, _, _) = call(
        app,
        "PUT",
        "/api/v1/sync",
        Auth::Cookie(cookie),
        Some(json!({"enabled": enabled}).to_string()),
    )
    .await;
    assert_eq!(s, 200);
    let (s, v, _) = call(
        app,
        "POST",
        "/api/v1/sync/setup-code",
        Auth::Cookie(cookie),
        None,
    )
    .await;
    assert_eq!(s, 200);
    let code = v["code"].as_str().unwrap().to_string();
    assert_eq!(
        v["link"],
        format!("healthvibe://setup?server=https%3A%2F%2Fhealth.example.test&code={code}")
    );
    let claim =
        json!({"setup_code": code, "device_name": "Test iPhone", "shortcut_version": "0.1.0"});
    let (s, v, _) = call(
        app,
        "POST",
        "/api/v1/shortcut/claim",
        Auth::None,
        Some(claim.to_string()),
    )
    .await;
    assert_eq!(s, 200, "{v}");
    // single use
    let (s, _, _) = call(
        app,
        "POST",
        "/api/v1/shortcut/claim",
        Auth::None,
        Some(claim.to_string()),
    )
    .await;
    assert_eq!(s, 401);
    v["upload_token"].as_str().unwrap().to_string()
}

fn upload() -> String {
    json!({
        "schema": 1, "shortcut_version": "0.1.0", "device": "Test iPhone",
        "run_id": "r1", "batch": {"index": 0, "count": 1}, "tz": "America/New_York",
        "metrics": {
            "stepCount": {"unit": "count", "samples": [
                {"start": "2026-11-02T07:00:00-05:00", "end": "2026-11-02T07:10:00-05:00", "value": 812, "source": "Synthetic Watch"},
                {"start": "2026-11-02T07:10:00-05:00", "end": "2026-11-02T07:20:00-05:00", "value": "430", "source": "Synthetic Watch"}
            ]},
            "sleepAnalysis": {"samples": [
                {"start": "2026-11-01 23:30:00 -0500", "end": "2026-11-02 03:00:00 -0500", "value": "Core", "source": "Synthetic Watch"},
                {"start": "2026-11-02T03:00:00-05:00", "end": "2026-11-02T04:00:00-05:00", "value": "REM", "source": "Synthetic Watch"}
            ]},
            "bodyFatPercentage": {"unit": "%", "samples": [
                {"start": "2026-11-02T07:00:00-05:00", "end": "2026-11-02T07:00:00-05:00", "value": 21.5, "source": "Synthetic Scale"}
            ]}
        },
        "workouts": [
            {"type": "running", "start": "2026-11-02T18:00:00-05:00", "end": "2026-11-02T18:30:00-05:00",
             "duration_s": 1800, "active_energy_kcal": 300, "distance_km": 5.0, "source": "Synthetic Watch"}
        ]
    })
    .to_string()
}

#[tokio::test]
async fn qr_needs_metrics_first() {
    let (app, _) = app().await;
    let c = admin(&app).await;
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/sync/setup-code",
        Auth::Cookie(&c),
        None,
    )
    .await;
    assert_eq!(s, 409);
    assert_eq!(v["error"], "choose_metrics_first");
    let (s, _, _) = call(
        &app,
        "PUT",
        "/api/v1/sync",
        Auth::Cookie(&c),
        Some(json!({"enabled": ["nope"]}).to_string()),
    )
    .await;
    assert_eq!(s, 400);
}

#[tokio::test]
async fn pair_config_ingest_dedupe_and_cleanup() {
    let (app, db) = app().await;
    let c = admin(&app).await;
    let tok = pair(&app, &c, json!(["stepCount", "sleepAnalysis", "workouts"])).await;
    assert!(tok.starts_with("hvu_"));
    // The secret is never stored.
    let secret = tok.rsplit('_').next().unwrap();
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM upload_tokens WHERE secret_hash = ? OR prefix = ?",
    )
    .bind(secret)
    .bind(secret)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(n, 0);

    let (s, cfg, _) = call(
        &app,
        "GET",
        "/api/v1/shortcut/config",
        Auth::Bearer(&tok),
        None,
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(
        cfg["enabled"],
        json!(["sleepAnalysis", "stepCount", "workouts"])
    );
    assert!(cfg["since"]["stepCount"].is_string());
    assert_eq!(cfg["max_request_bytes"], 64 * 1024);

    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/ingest",
        Auth::Bearer(&tok),
        Some(upload()),
    )
    .await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["accepted"], 5);
    assert_eq!(v["duplicates"], 0);
    assert_eq!(v["rejected"]["bodyFatPercentage"], "not_enabled");
    assert_eq!(v["next_since"]["stepCount"], "2026-11-02T12:10:00Z");

    // Same upload again: all duplicates.
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/ingest",
        Auth::Bearer(&tok),
        Some(upload()),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(v["accepted"], 0);
    assert_eq!(v["duplicates"], 5);

    let core: String = sqlx::query_scalar(
        "SELECT value FROM readings WHERE type = 'HKCategoryTypeIdentifierSleepAnalysis' ORDER BY start_utc LIMIT 1",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(core, "HKCategoryValueSleepAnalysisAsleepCore");
    let w: (String, String) =
        sqlx::query_as("SELECT type, value FROM readings WHERE type LIKE 'HKWorkout%'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(w, ("HKWorkoutActivityTypeRunning".into(), "30".into()));

    // Parsed raw bodies are deleted.
    let raw: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ingest_raw")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(raw, 0);

    // A bad sample keeps the raw body (24 h) for debugging.
    let bad = json!({"metrics": {"stepCount": {"samples": [{"start": "yesterday", "value": 1}]}}});
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/ingest",
        Auth::Bearer(&tok),
        Some(bad.to_string()),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(v["invalid"], 1);
    let kept: (String, Option<String>) =
        sqlx::query_as("SELECT status, expires_at FROM ingest_raw")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(kept.0, "failed");
    assert!(kept.1.is_some());

    // Unknown metric → 422, bad JSON → 400, too big → 413.
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/ingest",
        Auth::Bearer(&tok),
        Some(json!({"metrics": {"fooBar": {"samples": []}}}).to_string()),
    )
    .await;
    assert_eq!(s, 422);
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/ingest",
        Auth::Bearer(&tok),
        Some("{".into()),
    )
    .await;
    assert_eq!(s, 400);
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/ingest",
        Auth::Bearer(&tok),
        Some("x".repeat(70_000)),
    )
    .await;
    assert_eq!(s, 413);

    // Revoking the token locks the phone out.
    let (s, _, _) = call(&app, "DELETE", "/api/v1/sync/token", Auth::Cookie(&c), None).await;
    assert_eq!(s, 200);
    let (s, _, _) = call(
        &app,
        "GET",
        "/api/v1/shortcut/config",
        Auth::Bearer(&tok),
        None,
    )
    .await;
    assert_eq!(s, 401);
}

#[tokio::test]
async fn bad_tokens_and_old_shortcuts() {
    let (app, _) = app().await;
    let c = admin(&app).await;
    let tok = pair(&app, &c, json!(["stepCount"])).await;
    let forged = format!("{}x", &tok[..tok.len() - 1]);
    let (s, _, _) = call(
        &app,
        "GET",
        "/api/v1/shortcut/config",
        Auth::Bearer(&forged),
        None,
    )
    .await;
    assert_eq!(s, 401);
    let (s, _, _) = call(&app, "GET", "/api/v1/shortcut/config", Auth::None, None).await;
    assert_eq!(s, 401);
    let req = Request::get("/api/v1/shortcut/config")
        .header(header::AUTHORIZATION, format!("Bearer {tok}"))
        .header("x-hv-version", "0.0.9")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(req).await.unwrap().status(), 426);
    // Re-pairing replaces the old token.
    let tok2 = pair(&app, &c, json!(["stepCount"])).await;
    let (s, _, _) = call(
        &app,
        "GET",
        "/api/v1/shortcut/config",
        Auth::Bearer(&tok),
        None,
    )
    .await;
    assert_eq!(s, 401);
    let (s, _, _) = call(
        &app,
        "GET",
        "/api/v1/shortcut/config",
        Auth::Bearer(&tok2),
        None,
    )
    .await;
    assert_eq!(s, 200);
}

#[tokio::test]
async fn uploads_stay_with_their_owner() {
    let (app, db) = app().await;
    let c = admin(&app).await;
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/admin/users",
        Auth::Cookie(&c),
        Some(json!({"username": "sam", "temporary_password": "temporary pass"}).to_string()),
    )
    .await;
    assert_eq!(s, 201);
    let tok = pair(&app, &c, json!(["stepCount", "sleepAnalysis", "workouts"])).await;
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/ingest",
        Auth::Bearer(&tok),
        Some(upload()),
    )
    .await;
    assert_eq!(s, 200);
    let sam: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM readings r JOIN users u ON u.id = r.user_id WHERE u.username = 'sam'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(sam, 0);
}
