//! Browse records + pinned metrics, per-user isolation. Synthetic data only.

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
    (
        router(AppState {
            config,
            db: db.clone(),
        }),
        db,
    )
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value, Option<String>) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string())),
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
    let v = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, v, cookie)
}

/// Admin set up, plus a second user "sam" who has changed their password.
async fn two_users(app: &Router) -> (String, String) {
    let (s, v, admin) = call(
        app,
        "POST",
        "/api/v1/setup",
        None,
        Some(json!({"username": "Toby", "password": "admin password 1"})),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(v["user"]["username"], "toby");
    assert_eq!(v["user"]["is_admin"], true);
    let admin = admin.unwrap();
    let (s, _, _) = call(
        app,
        "POST",
        "/api/v1/admin/users",
        Some(&admin),
        Some(json!({"username": "sam", "temporary_password": "temporary pass"})),
    )
    .await;
    assert_eq!(s, 201);
    let (_, _, sam) = call(
        app,
        "POST",
        "/api/v1/login",
        None,
        Some(json!({"username": "sam", "password": "temporary pass"})),
    )
    .await;
    let sam = sam.unwrap();
    let (s, _, _) = call(
        app,
        "POST",
        "/api/v1/me/password",
        Some(&sam),
        Some(json!({"current": "temporary pass", "new": "sam's own password"})),
    )
    .await;
    assert_eq!(s, 200);
    (admin, sam)
}

async fn reading(db: &sqlx::SqlitePool, uid: i64, kind: &str, v: &str, day: &str, n: i64) {
    sqlx::query(
        "INSERT INTO readings (user_id, type, value, unit, start_utc, end_utc, hash, local_start, local_end)
         VALUES (?, ?, ?, 'count', ?, ?, ?, ?, ?)",
    )
    .bind(uid).bind(kind).bind(v)
    .bind(format!("{day}T08:00:00Z")).bind(format!("{day}T08:10:00Z"))
    .bind(format!("{uid}-{kind}-{day}-{n}"))
    .bind(format!("{day} 08:0{n}:00")).bind(format!("{day} 08:1{n}:00"))
    .execute(db).await.unwrap();
}

const STEPS: &str = "HKQuantityTypeIdentifierStepCount";

#[tokio::test]
async fn browse_records_paginate_and_are_per_user() {
    let (app, db) = app().await;
    let (admin, sam) = two_users(&app).await;
    for i in 0..5 {
        reading(
            &db,
            1,
            STEPS,
            &format!("{}", 100 + i),
            &format!("2026-01-0{}", i + 1),
            1,
        )
        .await;
    }
    let (s, v, _) = call(&app, "GET", "/api/v1/browse", Some(&admin), None).await;
    assert_eq!(s, 200);
    let m = &v["metrics"][0];
    assert_eq!(m["category"], "activity");
    assert_eq!(m["latest_value"], "104");

    let uri = format!("/api/v1/records?metric={STEPS}&limit=3");
    let (s, v, _) = call(&app, "GET", &uri, Some(&admin), None).await;
    assert_eq!(s, 200);
    assert_eq!(v["records"].as_array().unwrap().len(), 3);
    assert_eq!(v["records"][0]["value"], "104");
    let next = v["next"].as_i64().unwrap();
    let (_, v, _) = call(
        &app,
        "GET",
        &format!("{uri}&before={next}"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(v["records"].as_array().unwrap().len(), 2);
    assert!(v["next"].is_null());

    let (_, v, _) = call(&app, "GET", &uri, Some(&sam), None).await;
    assert_eq!(
        v["records"].as_array().unwrap().len(),
        0,
        "sam sees none of admin's records"
    );
    let (_, v, _) = call(&app, "GET", "/api/v1/browse", Some(&sam), None).await;
    assert_eq!(v["metrics"].as_array().unwrap().len(), 0);
    assert_eq!(call(&app, "GET", &uri, None, None).await.0, 401);

    let (s, v, _) = call(
        &app,
        "GET",
        &format!("/api/v1/hourly?metric={STEPS}&date=2026-01-05"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(v["values"][8], 104.0);
    let (_, v, _) = call(
        &app,
        "GET",
        &format!("/api/v1/hourly?metric={STEPS}"),
        Some(&sam),
        None,
    )
    .await;
    assert_eq!(v["values"][8], 0.0);
    for c in [&admin, &sam] {
        let b = Some(json!({ "enabled": true }));
        assert_eq!(
            call(&app, "PUT", "/api/v1/me/awards", Some(c), b).await.0,
            200
        );
    }
    let (s, v, _) = call(&app, "GET", "/api/v1/awards", Some(&admin), None).await;
    assert_eq!(s, 200);
    assert!(v["awards"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["key"] == "record_steps"));
    let (_, v, _) = call(&app, "GET", "/api/v1/awards", Some(&sam), None).await;
    assert!(v["awards"]
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["earned"].is_null()));

    reading(
        &db,
        1,
        "HKQuantityTypeIdentifierActiveEnergyBurned",
        "321",
        "2026-01-03",
        2,
    )
    .await;
    let r = "/api/v1/rings?start=2026-01-01&end=2026-01-07";
    let (s, v, _) = call(&app, "GET", r, Some(&admin), None).await;
    assert_eq!(s, 200);
    assert_eq!(v["days"].as_array().unwrap().len(), 7);
    assert_eq!(v["days"][2]["move_kcal"], 321.0);
    let (_, v, _) = call(&app, "GET", r, Some(&sam), None).await;
    assert_eq!(v["days"][2]["move_kcal"], 0.0);
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/v1/rings?start=x&end=y",
            Some(&admin),
            None
        )
        .await
        .0,
        400
    );
}

#[tokio::test]
async fn pinned_round_trip_and_isolation() {
    let (app, _) = app().await;
    let (admin, sam) = two_users(&app).await;
    let (s, v, _) = call(
        &app,
        "PUT",
        "/api/v1/pinned",
        Some(&admin),
        Some(json!({"pinned": [STEPS, "b", STEPS]})),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(v["pinned"], json!([STEPS, "b"]));
    let (_, v, _) = call(&app, "GET", "/api/v1/pinned", Some(&sam), None).await;
    assert_eq!(v["pinned"], json!([]));
    let many: Vec<String> = (0..31).map(|i| format!("m{i}")).collect();
    let (s, _, _) = call(
        &app,
        "PUT",
        "/api/v1/pinned",
        Some(&admin),
        Some(json!({"pinned": many})),
    )
    .await;
    assert_eq!(s, 400);
}
