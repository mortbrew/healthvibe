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
        "INSERT INTO readings (user_id, type, value, unit, start_utc, end_utc, source, channel, hash, local_start, local_end)
         VALUES (?, ?, ?, 'count', ?, ?, '', 'unknown', ?, ?, ?)",
    )
    .bind(uid).bind(kind).bind(v)
    .bind(format!("{day}T08:{n:02}:00Z")).bind(format!("{day}T08:{n:02}:10Z"))
    .bind(format!("{uid}-{kind}-{day}-{n}"))
    .bind(format!("{day} 08:{n:02}:00")).bind(format!("{day} 08:{n:02}:10"))
    .execute(db).await.unwrap();
}

const ENERGY: &str = "HKQuantityTypeIdentifierActiveEnergyBurned";

#[tokio::test]
async fn no_awards_are_computed_from_data() {
    let (app, db) = app().await;
    let (admin, _sam) = two_users(&app).await;
    // A week of big Move days and a workout: nothing here becomes an award.
    for d in 5..=14 {
        reading(&db, 1, ENERGY, "2600", &format!("2026-01-{d:02}"), 0).await;
    }
    reading(&db, 1, "HKWorkoutActivityTypeRunning", "30", "2026-01-06", 1).await;
    let (s, v, _) = call(&app, "GET", "/api/v1/awards", Some(&admin), None).await;
    assert_eq!(s, 200);
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, vec!["apple_only"]);
    assert!(v["apple_only"]
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["earned"] == false));
    // The old opt-in switch and its flag are gone.
    let (s, _, _) = call(
        &app,
        "PUT",
        "/api/v1/me/awards",
        Some(&admin),
        Some(json!({"enabled": true})),
    )
    .await;
    assert_ne!(s, StatusCode::OK);
    let (s, v, _) = call(&app, "GET", "/api/v1/session", Some(&admin), None).await;
    assert_eq!((s, &v["user"]["username"]), (StatusCode::OK, &json!("toby")));
    assert!(v["user"].get("awards_enabled").is_none());
    assert_eq!(
        call(&app, "GET", "/api/v1/awards", None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn apple_only_manual_checks_are_per_user() {
    let (app, _db) = app().await;
    let (admin, sam) = two_users(&app).await;
    let u = "/api/v1/awards/apple/monthly_challenge";
    assert_eq!(
        call(
            &app,
            "PUT",
            u,
            Some(&admin),
            Some(json!({"earned": true, "date": "2026-03-31"}))
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &app,
            "PUT",
            "/api/v1/awards/apple/nope",
            Some(&admin),
            Some(json!({"earned": true}))
        )
        .await
        .0,
        404
    );
    assert_eq!(
        call(
            &app,
            "PUT",
            u,
            Some(&admin),
            Some(json!({"earned": true, "date": "bad"}))
        )
        .await
        .0,
        400
    );
    let (_, v, _) = call(&app, "GET", "/api/v1/awards", Some(&admin), None).await;
    let m = v["apple_only"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["key"] == "monthly_challenge")
        .unwrap()
        .clone();
    assert_eq!(
        (m["earned"].clone(), m["earned_date"].clone()),
        (json!(true), json!("2026-03-31"))
    );
    let (_, v, _) = call(&app, "GET", "/api/v1/awards", Some(&sam), None).await;
    assert!(v["apple_only"]
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["earned"] == false));
    call(&app, "PUT", u, Some(&admin), Some(json!({"earned": false}))).await;
    let (_, v, _) = call(&app, "GET", "/api/v1/awards", Some(&admin), None).await;
    assert!(v["apple_only"]
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["earned"] == false));
}

#[tokio::test]
async fn profile_round_trip_isolation_and_validation() {
    let (app, db) = app().await;
    let (admin, sam) = two_users(&app).await;
    let (_, v, _) = call(&app, "GET", "/api/v1/profile", Some(&admin), None).await;
    assert!(v["profile"]["display_name"].is_null());
    let p = json!({"display_name": " Toby ", "birth_date": "1990-02-03", "sex": "male", "height_cm": 180.5,
        "wheelchair": false, "avatar": {"face": 1, "hair": 3}, "units": {"distance": "km"}});
    let (s, v, _) = call(&app, "PUT", "/api/v1/profile", Some(&admin), Some(p)).await;
    assert_eq!(s, 200);
    assert_eq!(v["profile"]["display_name"], "Toby");
    let (_, v, _) = call(&app, "GET", "/api/v1/profile", Some(&admin), None).await;
    assert_eq!(v["profile"]["avatar"]["hair"], 3);
    assert_eq!(v["profile"]["height_cm"], 180.5);
    assert!(v["profile"].get("stand_goal").is_none());
    let (_, v, _) = call(&app, "GET", "/api/v1/profile", Some(&sam), None).await;
    assert!(v["profile"]["display_name"].is_null());
    for bad in [
        json!({"sex": "robot"}),
        json!({"birth_date": "02/03/1990"}),
        json!({"height_cm": 900}),
        json!({"avatar": "x"}),
    ] {
        assert_eq!(
            call(&app, "PUT", "/api/v1/profile", Some(&sam), Some(bad))
                .await
                .0,
            400
        );
    }
    // Goals come only from ActivitySummary data: an earlier day's goal carries forward.
    sqlx::query("INSERT INTO activity_goals (user_id, date, stand_hours) VALUES (1, '2026-01-01', 10)")
        .execute(&db)
        .await
        .unwrap();
    reading(&db, 1, ENERGY, "10", "2026-01-05", 0).await;
    let (_, v, _) = call(
        &app,
        "GET",
        "/api/v1/rings?start=2026-01-05&end=2026-01-05",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(v["days"][0]["stand_goal"], 10.0);
    let (_, v, _) = call(
        &app,
        "GET",
        "/api/v1/dashboard/activity?end=2026-01-05",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(v["rings"]["stand_goal"], 10.0);
}

#[tokio::test]
async fn privacy_export_delete_and_sessions() {
    let (app, db) = app().await;
    let (admin, sam) = two_users(&app).await;
    reading(&db, 1, ENERGY, "10", "2026-01-05", 0).await;
    reading(&db, 2, ENERGY, "20", "2026-01-05", 0).await;
    let req = Request::builder()
        .uri("/api/v1/me/export.csv")
        .header(header::COOKIE, &admin)
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), 200);
    let csv = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(csv.lines().count(), 2);
    assert!(csv.contains(",10,") && !csv.contains(",20,"));

    for uid in [1, 2] {
        sqlx::query(
            "INSERT INTO workouts (user_id, channel, source, start_utc, end_utc, activity_type)
             VALUES (?, 'export_xml', 'Synthetic', '2026-01-01T10:00:00Z', '2026-01-01T10:30:00Z', 'HKWorkoutActivityTypeRunning')",
        )
        .bind(uid)
        .execute(&db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO apple_awards (user_id, key, earned_date) VALUES (?, 'sharing', NULL)")
            .bind(uid)
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ingest_raw (user_id, received_at, body, status) VALUES (?, '2026-01-01T00:00:00Z', x'00', 'failed')")
            .bind(uid)
            .execute(&db)
            .await
            .unwrap();
    }
    assert_eq!(
        call(
            &app,
            "DELETE",
            "/api/v1/me/data",
            Some(&admin),
            Some(json!({"confirm": "sam"}))
        )
        .await
        .0,
        400
    );
    assert_eq!(
        call(
            &app,
            "DELETE",
            "/api/v1/me/data",
            Some(&admin),
            Some(json!({"confirm": "toby"}))
        )
        .await
        .0,
        200
    );
    let left: Vec<i64> = sqlx::query_scalar("SELECT user_id FROM readings")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(left, vec![2]);
    for table in ["workouts", "ingest_raw", "apple_awards"] {
        let left: Vec<i64> = sqlx::query_scalar(&format!("SELECT user_id FROM {table}"))
            .fetch_all(&db)
            .await
            .unwrap();
        assert_eq!(left, vec![2], "{table}");
    }

    let (_, _, sam2) = call(
        &app,
        "POST",
        "/api/v1/login",
        None,
        Some(json!({"username": "sam", "password": "sam's own password"})),
    )
    .await;
    let sam2 = sam2.unwrap();
    let (s, v, _) = call(
        &app,
        "DELETE",
        "/api/v1/me/sessions/others",
        Some(&sam2),
        None,
    )
    .await;
    assert_eq!((s, v["revoked"].clone()), (StatusCode::OK, json!(1)));
    assert_eq!(
        call(&app, "GET", "/api/v1/profile", Some(&sam), None)
            .await
            .0,
        401
    );
    assert_eq!(
        call(&app, "GET", "/api/v1/profile", Some(&sam2), None)
            .await
            .0,
        200
    );
    assert_eq!(
        call(&app, "GET", "/api/v1/profile", Some(&admin), None)
            .await
            .0,
        200
    );
}
