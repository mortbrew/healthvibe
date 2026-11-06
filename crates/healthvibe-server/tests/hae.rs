//! Health Auto Export path: token from the web app, ingest with Bearer or
//! custom header, mapping, re-send updates instead of doubling, isolation,
//! onboarding flag. Synthetic data only.

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
    config.hae_max_request_bytes = 32 * 1024 * 1024;
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
    Header(&'a str, &'a str),
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
        Auth::Header(n, v) => req = req.header(n, v),
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

const FIXTURE: &str = include_str!("../../healthvibe-core/tests/fixtures/hae.json");

async fn count(db: &sqlx::SqlitePool, user: i64, kind: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM readings WHERE user_id = ? AND type = ?")
        .bind(user)
        .bind(kind)
        .fetch_one(db)
        .await
        .unwrap()
}

#[tokio::test]
async fn hae_end_to_end() {
    let (app, db) = app().await;
    let c = admin(&app).await;
    // onboarding starts open, can be finished
    let (_, v, _) = call(&app, "GET", "/api/v1/session", Auth::Cookie(&c), None).await;
    assert_eq!(v["user"]["onboarding_done"], false, "{v}");
    let (s, v, _) = call(&app, "GET", "/api/v1/sync/hae", Auth::Cookie(&c), None).await;
    assert_eq!(s, 200);
    assert_eq!(v["url"], "https://health.example.test/api/v1/ingest/hae");
    assert_eq!(v["has_token"], false);
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/sync/hae/token",
        Auth::Cookie(&c),
        None,
    )
    .await;
    assert_eq!(s, 200);
    let tok = v["token"].as_str().unwrap().to_string();
    assert!(tok.starts_with("hvu_"));
    // no token / bad token
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/ingest/hae",
        Auth::None,
        Some(FIXTURE.into()),
    )
    .await;
    assert_eq!(s, 401);
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/ingest/hae",
        Auth::Bearer("hvu_nope_nope"),
        Some(FIXTURE.into()),
    )
    .await;
    assert_eq!(s, 401);
    // Bearer
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/ingest/hae",
        Auth::Bearer(&tok),
        Some(FIXTURE.into()),
    )
    .await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["invalid"], 0, "{v}");
    let first = v["accepted"].as_u64().unwrap();
    assert!(first >= 15, "{v}");
    assert_eq!(count(&db, 1, "HKQuantityTypeIdentifierStepCount").await, 2);
    assert_eq!(
        count(&db, 1, "HKCategoryTypeIdentifierSleepAnalysis").await,
        6
    );
    // same body via custom header: nothing new
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/ingest/hae",
        Auth::Header("X-Healthvibe-Token", &tok),
        Some(FIXTURE.into()),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(v["accepted"], 0);
    assert_eq!(v["unchanged"].as_u64().unwrap(), first);
    // a grown value for the same reading is updated, not added
    let grown = FIXTURE.replace("\"qty\": 1200", "\"qty\": 1500");
    let (_, v, _) = call(
        &app,
        "POST",
        "/api/v1/ingest/hae",
        Auth::Header("api-key", &tok),
        Some(grown),
    )
    .await;
    assert_eq!(v["updated"], 1, "{v}");
    assert_eq!(count(&db, 1, "HKQuantityTypeIdentifierStepCount").await, 2);
    let total: f64 = sqlx::query_scalar("SELECT SUM(CAST(value AS REAL)) FROM readings WHERE type = 'HKQuantityTypeIdentifierStepCount'")
        .fetch_one(&db).await.unwrap();
    assert_eq!(total, 2345.5);
    // received counter for the onboarding check
    let (_, v, _) = call(&app, "GET", "/api/v1/sync/hae", Auth::Cookie(&c), None).await;
    assert_eq!(v["received"].as_u64().unwrap(), first * 3);
    assert_eq!(v["has_token"], true);
    // bad json
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/ingest/hae",
        Auth::Bearer(&tok),
        Some("nope".into()),
    )
    .await;
    assert_eq!(s, 400);
    assert_eq!(v["error"], "bad_json");
    // new token replaces the old one
    let (_, v, _) = call(
        &app,
        "POST",
        "/api/v1/sync/hae/token",
        Auth::Cookie(&c),
        None,
    )
    .await;
    assert_ne!(v["token"].as_str().unwrap(), tok);
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/ingest/hae",
        Auth::Bearer(&tok),
        Some(FIXTURE.into()),
    )
    .await;
    assert_eq!(s, 401);
    // onboarding done
    let (s, _, _) = call(
        &app,
        "PUT",
        "/api/v1/me/onboarding",
        Auth::Cookie(&c),
        Some(json!({"done": true}).to_string()),
    )
    .await;
    assert_eq!(s, 200);
    let (_, v, _) = call(&app, "GET", "/api/v1/session", Auth::Cookie(&c), None).await;
    assert_eq!(v["user"]["onboarding_done"], true);
}

#[tokio::test]
async fn hae_too_large() {
    let (app, _) = app().await;
    let c = admin(&app).await;
    let (_, v, _) = call(
        &app,
        "POST",
        "/api/v1/sync/hae/token",
        Auth::Cookie(&c),
        None,
    )
    .await;
    let tok = v["token"].as_str().unwrap().to_string();
    let big = format!(
        "{{\"data\":{{\"metrics\":[]}},\"pad\":\"{}\"}}",
        "x".repeat(33 * 1024 * 1024)
    );
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/ingest/hae",
        Auth::Bearer(&tok),
        Some(big),
    )
    .await;
    assert_eq!(s, 413);
}

/// All-metrics style batch: a week of per-minute heart rate plus 40 other
/// metrics, ~37k points, a few MB. Synthetic.
#[tokio::test]
async fn hae_big_payload() {
    let (app, db) = app().await;
    let c = admin(&app).await;
    let (_, v, _) = call(
        &app,
        "POST",
        "/api/v1/sync/hae/token",
        Auth::Cookie(&c),
        None,
    )
    .await;
    let tok = v["token"].as_str().unwrap().to_string();
    let t0 = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00-05:00").unwrap();
    let fmt = |m: i64| {
        (t0 + chrono::Duration::minutes(m))
            .format("%Y-%m-%d %H:%M:%S %z")
            .to_string()
    };
    let hr: Vec<Value> = (0..7 * 24 * 60)
        .map(|m| json!({"date": fmt(m), "Min": 60, "Avg": 60 + m % 30, "Max": 95, "source": "Synthetic Watch"}))
        .collect();
    let mut metrics = vec![json!({"name": "heart_rate", "units": "count/min", "data": hr})];
    for k in 0..40 {
        let pts: Vec<Value> = (0..7 * 24)
            .map(|h| json!({"date": fmt(h * 60), "qty": h as f64 + 0.5}))
            .collect();
        metrics
            .push(json!({"name": format!("synthetic_metric_{k}"), "units": "count", "data": pts}));
    }
    let body = json!({"data": {"metrics": metrics}}).to_string();
    assert!(body.len() > 1024 * 1024, "{}", body.len());
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/ingest/hae",
        Auth::Bearer(&tok),
        Some(body.clone()),
    )
    .await;
    assert_eq!(s, 200, "{v}");
    let expect = 7 * 24 * 60 * 3 + 40 * 7 * 24;
    assert_eq!(
        v["accepted"].as_u64().unwrap(),
        expect as u64,
        "{}",
        v["errors"]
    );
    // unknown metrics are kept as generic readings
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM readings WHERE type = 'healthvibe.hae.synthetic_metric_7'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(n, 7 * 24);
    let (_, v, _) = call(
        &app,
        "POST",
        "/api/v1/ingest/hae",
        Auth::Bearer(&tok),
        Some(body),
    )
    .await;
    assert_eq!(v["accepted"], 0);
    assert_eq!(v["unchanged"].as_u64().unwrap(), expect as u64);
}
