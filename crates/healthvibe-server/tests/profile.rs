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
async fn awards_feature_is_gone() {
    let (app, db) = app().await;
    let (admin, _sam) = two_users(&app).await;
    for d in 5..=14 {
        reading(&db, 1, ENERGY, "2600", &format!("2026-01-{d:02}"), 0).await;
    }
    for (m, u) in [
        ("GET", "/api/v1/awards"),
        ("PUT", "/api/v1/awards/apple/monthly_challenge"),
        ("PUT", "/api/v1/me/awards"),
    ] {
        let (s, v, _) = call(
            &app,
            m,
            u,
            Some(&admin),
            Some(json!({"earned": true, "enabled": true})),
        )
        .await;
        assert_ne!(s, StatusCode::OK, "{m} {u}");
        assert!(v.get("apple_only").is_none() && v.get("awards").is_none());
    }
    let (s, v, _) = call(&app, "GET", "/api/v1/session", Some(&admin), None).await;
    assert_eq!(
        (s, &v["user"]["username"]),
        (StatusCode::OK, &json!("toby"))
    );
    assert!(v["user"].get("awards_enabled").is_none());
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE name LIKE '%award%'")
            .fetch_all(&db)
            .await
            .unwrap();
    assert!(tables.is_empty(), "{tables:?}");
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
    // Goals are imported only: that day's ActivitySummary, else the latest
    // earlier imported one (labelled with its date), never a default.
    sqlx::query("INSERT INTO activity_goals (user_id, date, stand_hours) VALUES (1, '2026-01-01', 10), (1, '2026-01-06', 11)")
        .execute(&db)
        .await
        .unwrap();
    reading(&db, 1, ENERGY, "10", "2026-01-05", 0).await;
    let (_, v, _) = call(
        &app,
        "GET",
        "/api/v1/rings?start=2026-01-05&end=2026-01-06",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(v["days"][0]["stand_goal"], 10.0);
    assert_eq!(v["days"][0]["stand_goal_from"], "2026-01-01");
    assert!(v["days"][0]["move_goal"].is_null());
    assert!(v["days"][0]["move_goal_from"].is_null());
    assert_eq!(v["days"][0]["move_kcal"], 10.0);
    assert_eq!(v["days"][1]["stand_goal"], 11.0);
    assert!(v["days"][1]["stand_goal_from"].is_null());
    let (_, v, _) = call(
        &app,
        "GET",
        "/api/v1/dashboard/activity?end=2026-01-05",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(v["rings"]["stand_goal"], 10.0);
    assert_eq!(v["rings"]["stand_goal_from"], "2026-01-01");
    assert_eq!(v["rings"]["move_kcal"], 10.0);
    // Before the first imported goal there is none.
    let (_, v, _) = call(
        &app,
        "GET",
        "/api/v1/dashboard/activity?end=2025-12-31",
        Some(&admin),
        None,
    )
    .await;
    assert!(v["rings"]["stand_goal"].is_null());
    assert!(v["rings"]["stand_goal_from"].is_null());
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
    for table in ["workouts", "ingest_raw"] {
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

#[tokio::test]
async fn csv_export_includes_workouts() {
    let (app, db) = app().await;
    let (admin, _sam) = two_users(&app).await;
    reading(&db, 1, ENERGY, "10", "2026-01-05", 0).await;
    for (uid, dur) in [(1, 30.0), (2, 99.0)] {
        sqlx::query(
            "INSERT INTO workouts (user_id, channel, source, start_utc, end_utc, activity_type, duration_min, distance_m)
             VALUES (?, 'export_xml', 'Synthetic', '2026-01-01T10:00:00Z', '2026-01-01T10:30:00Z', 'HKWorkoutActivityTypeRunning', ?, 5000)",
        )
        .bind(uid)
        .bind(dur)
        .execute(&db)
        .await
        .unwrap();
    }
    let req = Request::builder()
        .uri("/api/v1/me/export.csv")
        .header(header::COOKIE, &admin)
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let csv = String::from_utf8(
        axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[0], "type,value,unit,start_utc,end_utc,source");
    let w = "2026-01-01T10:00:00Z,2026-01-01T10:30:00Z,Synthetic";
    assert!(lines.contains(&format!("Workout:HKWorkoutActivityTypeRunning,,,{w}").as_str()));
    assert!(lines
        .contains(&format!("Workout:HKWorkoutActivityTypeRunning:duration,30,min,{w}").as_str()));
    assert!(lines
        .contains(&format!("Workout:HKWorkoutActivityTypeRunning:distance,5000,m,{w}").as_str()));
    // Absent values get no row; other users' workouts are not included.
    assert!(!csv.contains(":avg_heart_rate") && !csv.contains(",99,"));
    assert_eq!(lines.len(), 1 + 1 + 3);
}
