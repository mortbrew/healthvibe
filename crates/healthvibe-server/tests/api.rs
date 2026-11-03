//! End-to-end API tests: first-run admin, temporary passwords that must be
//! changed (enforced server-side), and per-user data isolation.

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

#[tokio::test]
async fn first_user_is_admin_and_setup_runs_once() {
    let (app, _) = app().await;
    let (_, v, _) = call(&app, "GET", "/api/v1/session", None, None).await;
    assert_eq!(v["needs_setup"], true);
    two_users(&app).await;
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/setup",
        None,
        Some(json!({"username": "evil", "password": "another password"})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
}

#[tokio::test]
async fn temporary_password_blocks_everything_until_changed() {
    let (app, _) = app().await;
    let (admin, _) = two_users(&app).await;
    call(
        &app,
        "POST",
        "/api/v1/admin/users",
        Some(&admin),
        Some(json!({"username": "kim", "temporary_password": "temporary pass"})),
    )
    .await;
    let (_, v, kim) = call(
        &app,
        "POST",
        "/api/v1/login",
        None,
        Some(json!({"username": "kim", "password": "temporary pass"})),
    )
    .await;
    assert_eq!(v["user"]["must_change_password"], true);
    let kim = kim.unwrap();
    for uri in [
        "/api/v1/dashboard/activity",
        "/api/v1/dashboard/heart",
        "/api/v1/dashboard/sleep",
        "/api/v1/dashboard/body",
        "/api/v1/imports",
        "/api/v1/import/1",
        "/api/v1/admin/users",
    ] {
        let (s, v, _) = call(&app, "GET", uri, Some(&kim), None).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{uri}");
        assert_eq!(v["error"], "password_change_required");
    }
    let (s, _, _) = call(
        &app,
        "PUT",
        "/api/v1/me/theme",
        Some(&kim),
        Some(json!({"mode": "dark", "light": "paper", "dark": "midnight"})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    // Session is readable so the SPA can show the change-password screen.
    let (s, _, _) = call(&app, "GET", "/api/v1/session", Some(&kim), None).await;
    assert_eq!(s, 200);
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/me/password",
        Some(&kim),
        Some(json!({"current": "temporary pass", "new": "temporary pass"})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "must pick a new password");
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/me/password",
        Some(&kim),
        Some(json!({"current": "temporary pass", "new": "kim's real password"})),
    )
    .await;
    assert_eq!(s, 200);
    let (s, _, _) = call(&app, "GET", "/api/v1/dashboard/activity", Some(&kim), None).await;
    assert_eq!(s, 200);
    // Non-admins cannot manage users.
    let (s, _, _) = call(&app, "GET", "/api/v1/admin/users", Some(&kim), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn unauthenticated_is_rejected() {
    let (app, _) = app().await;
    let (s, _, _) = call(&app, "GET", "/api/v1/dashboard/activity", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _, _) = call(
        &app,
        "GET",
        "/api/v1/dashboard/activity",
        Some("healthvibe_session=forged"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn users_never_see_each_others_data() {
    let (app, db) = app().await;
    let (admin, sam) = two_users(&app).await;
    // Admin (user 1) has readings and an import job.
    for (i, (t, v)) in [
        ("HKQuantityTypeIdentifierStepCount", "4321"),
        ("HKQuantityTypeIdentifierHeartRate", "61"),
        ("HKQuantityTypeIdentifierBodyMass", "80"),
    ]
    .iter()
    .enumerate()
    {
        sqlx::query(
            "INSERT INTO readings (user_id, type, value, unit, start_utc, end_utc,
             hash, local_start, local_end) VALUES (1, ?, ?, 'u', '2024-01-02T08:00:00Z',
             '2024-01-02T08:10:00Z', ?, '2024-01-02 08:00:00', '2024-01-02 08:10:00')",
        )
        .bind(t)
        .bind(v)
        .bind(i.to_string())
        .execute(&db)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO readings (user_id, type, value, start_utc, end_utc, hash,
         local_start, local_end) VALUES (1, 'HKCategoryTypeIdentifierSleepAnalysis',
         'HKCategoryValueSleepAnalysisAsleepCore', '2024-01-02T00:00:00Z',
         '2024-01-02T06:00:00Z', 's', '2024-01-02 00:00:00', '2024-01-02 06:00:00')",
    )
    .execute(&db)
    .await
    .unwrap();
    let job = healthvibe_core::import::create_job(&db, 1).await.unwrap();

    let (_, a, _) = call(
        &app,
        "GET",
        "/api/v1/dashboard/activity",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(a["rings"]["steps"], 4321.0);
    let (s, _, _) = call(
        &app,
        "GET",
        &format!("/api/v1/import/{job}"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(s, 200);

    let (_, b, _) = call(&app, "GET", "/api/v1/dashboard/activity", Some(&sam), None).await;
    assert!(b["rings"].is_null(), "{b}");
    let (_, b, _) = call(
        &app,
        "GET",
        "/api/v1/dashboard/activity?end=2024-01-02",
        Some(&sam),
        None,
    )
    .await;
    assert_eq!(b["rings"]["steps"], 0.0);
    let (_, h, _) = call(
        &app,
        "GET",
        "/api/v1/dashboard/heart?end=2024-01-02",
        Some(&sam),
        None,
    )
    .await;
    assert_eq!(h["daily"], json!([]));
    let (_, sl, _) = call(
        &app,
        "GET",
        "/api/v1/dashboard/sleep?end=2024-01-02",
        Some(&sam),
        None,
    )
    .await;
    assert_eq!(sl["nights"], json!([]));
    let (_, bo, _) = call(
        &app,
        "GET",
        "/api/v1/dashboard/body?end=2024-01-02",
        Some(&sam),
        None,
    )
    .await;
    assert_eq!(bo["weight"]["points"], json!([]));
    let (s, _, _) = call(
        &app,
        "GET",
        &format!("/api/v1/import/{job}"),
        Some(&sam),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (_, l, _) = call(&app, "GET", "/api/v1/imports", Some(&sam), None).await;
    assert_eq!(l["imports"], json!([]));
}

#[tokio::test]
async fn form_posts_are_rejected() {
    // CSRF: only JSON bodies are accepted on state-changing endpoints.
    let (app, _) = app().await;
    let req = Request::post("/api/v1/setup")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=x&password=yyyyyyyyyyyyyy"))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn usernames_can_be_email_addresses() {
    let (app, _) = app().await;
    let p = |u: &str, pw: &str| json!({"username": u, "password": pw});
    // First-run setup with a mixed-case email: stored trimmed and lowercased.
    let (s, v, admin) = call(
        &app,
        "POST",
        "/api/v1/setup",
        None,
        Some(p("  Toby.Moon+HV@Example.com ", "admin password 1")),
    )
    .await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["user"]["username"], "toby.moon+hv@example.com");
    let admin = admin.unwrap();
    // Login matches case-insensitively.
    for u in [
        "toby.moon+hv@example.com",
        "TOBY.MOON+HV@EXAMPLE.COM",
        " Toby.Moon+hv@example.COM",
    ] {
        let (s, _, c) = call(
            &app,
            "POST",
            "/api/v1/login",
            None,
            Some(p(u, "admin password 1")),
        )
        .await;
        assert_eq!(s, 200, "{u}");
        assert!(c.is_some());
    }
    // Admin creates an email user; a different-case duplicate is refused.
    let new = |u: &str| json!({"username": u, "temporary_password": "temporary pass"});
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/admin/users",
        Some(&admin),
        Some(new("Sam_O-Neil@mail.example.org")),
    )
    .await;
    assert_eq!(s, 201, "{v}");
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/admin/users",
        Some(&admin),
        Some(new("SAM_O-NEIL@MAIL.EXAMPLE.ORG")),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/login",
        None,
        Some(p("Sam_O-Neil@Mail.Example.org", "temporary pass")),
    )
    .await;
    assert_eq!(s, 200);
    // 254 characters is the limit; bad characters and double '@' are rejected.
    let max = format!("{}@example.com", "a".repeat(242));
    let (s, _, _) = call(
        &app,
        "POST",
        "/api/v1/admin/users",
        Some(&admin),
        Some(new(&max)),
    )
    .await;
    assert_eq!(s, 201);
    for bad in [
        format!("{max}x"),
        "a@b@c.com".into(),
        "with space@example.com".into(),
        "semi;colon".into(),
    ] {
        let (s, v, _) = call(
            &app,
            "POST",
            "/api/v1/admin/users",
            Some(&admin),
            Some(new(&bad)),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad}");
        assert_eq!(v["error"], "invalid_username");
    }
}
