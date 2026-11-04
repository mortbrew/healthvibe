//! Phone handoff: one-time QR link that mints the Health Auto Export token
//! when opened. Single use, expiry, per-user isolation, no token leak.
//! Synthetic data only.

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

#[allow(dead_code)]
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


async fn user(app: &Router, admin: &str, name: &str) -> String {
    let (s, _, _) = call(app, "POST", "/api/v1/admin/users", Auth::Cookie(admin),
        Some(json!({"username": name, "temporary_password": "temporary pass"}).to_string())).await;
    assert_eq!(s, 201);
    let (_, _, c) = call(app, "POST", "/api/v1/login", Auth::None,
        Some(json!({"username": name, "password": "temporary pass"}).to_string())).await;
    let c = c.unwrap();
    let (s, _, c2) = call(app, "POST", "/api/v1/me/password", Auth::Cookie(&c),
        Some(json!({"current": "temporary pass", "new": format!("{name}'s own password")}).to_string())).await;
    assert_eq!(s, 200);
    c2.unwrap_or(c)
}

fn code_of(link: &str) -> String {
    link.rsplit('/').next().unwrap().to_string()
}

async fn claim(app: &Router, code: &str) -> (StatusCode, Value) {
    let (s, v, _) = call(app, "POST", "/api/v1/handoff/claim", Auth::None,
        Some(json!({ "code": code }).to_string())).await;
    (s, v)
}

#[tokio::test]
async fn handoff_single_use_and_token_works() {
    let (app, db) = app().await;
    let c = admin(&app).await;
    // needs a session
    let (s, _, _) = call(&app, "POST", "/api/v1/sync/hae/handoff", Auth::None, None).await;
    assert_eq!(s, 401);
    let (s, v, _) = call(&app, "POST", "/api/v1/sync/hae/handoff", Auth::Cookie(&c), None).await;
    assert_eq!(s, 200, "{v}");
    let link = v["link"].as_str().unwrap();
    assert!(link.starts_with("https://health.example.test/h/"), "{link}");
    assert!(v.get("token").is_none(), "no token before the phone opens it");
    let id = v["id"].as_str().unwrap().to_string();
    let code = code_of(link);
    // nothing about the code or a token is stored in plain text
    let stored: String = sqlx::query_scalar("SELECT code_hash FROM handoffs").fetch_one(&db).await.unwrap();
    assert_ne!(stored, code);
    let tokens: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM upload_tokens").fetch_one(&db).await.unwrap();
    assert_eq!(tokens, 0, "token is minted on claim, not on create");

    let (_, st, _) = call(&app, "GET", &format!("/api/v1/sync/hae/handoff/{id}"), Auth::Cookie(&c), None).await;
    assert_eq!(st["claimed"], false);

    let (s, v) = claim(&app, &code).await;
    assert_eq!(s, 200, "{v}");
    let tok = v["token"].as_str().unwrap().to_string();
    assert!(tok.starts_with("hvu_"));
    assert_eq!(v["url"], "https://health.example.test/api/v1/ingest/hae");
    // the revealed token uploads
    let (s, _, _) = call(&app, "POST", "/api/v1/ingest/hae", Auth::Bearer(&tok), Some(r#"{"data":{"metrics":[]}}"#.into())).await;
    assert_eq!(s, 200);

    // second open: friendly error, no token
    let (s, v) = claim(&app, &code).await;
    assert_eq!(s, 410);
    assert_eq!(v["error"], "handoff_invalid");
    assert!(!v.to_string().contains("hvu_"), "{v}");
    // garbage code
    let (s, _) = claim(&app, "zz").await;
    assert_eq!(s, 410);

    let (_, st, _) = call(&app, "GET", &format!("/api/v1/sync/hae/handoff/{id}"), Auth::Cookie(&c), None).await;
    assert_eq!(st["claimed"], true, "{st}");
    assert!(!st.to_string().contains("hvu_"), "status never carries the token");
    // a token is never kept in plain text
    let leaked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM upload_tokens WHERE secret_hash LIKE '%' || ? || '%'")
        .bind(tok.rsplit('_').next().unwrap()).fetch_one(&db).await.unwrap();
    assert_eq!(leaked, 0);
}

#[tokio::test]
async fn handoff_expires_and_new_one_replaces_old() {
    let (app, db) = app().await;
    let c = admin(&app).await;
    let (_, v, _) = call(&app, "POST", "/api/v1/sync/hae/handoff", Auth::Cookie(&c), None).await;
    let old = code_of(v["link"].as_str().unwrap());
    let (_, v, _) = call(&app, "POST", "/api/v1/sync/hae/handoff", Auth::Cookie(&c), None).await;
    let code = code_of(v["link"].as_str().unwrap());
    let id = v["id"].as_str().unwrap().to_string();
    // the older unopened link stopped working
    assert_eq!(claim(&app, &old).await.0, 410);
    // expire the current one
    sqlx::query("UPDATE handoffs SET expires_at = '2000-01-01T00:00:00Z'").execute(&db).await.unwrap();
    let (_, st, _) = call(&app, "GET", &format!("/api/v1/sync/hae/handoff/{id}"), Auth::Cookie(&c), None).await;
    assert_eq!(st["expired"], true, "{st}");
    let (s, v) = claim(&app, &code).await;
    assert_eq!(s, 410);
    assert!(v.get("token").is_none());
    let tokens: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM upload_tokens").fetch_one(&db).await.unwrap();
    assert_eq!(tokens, 0);
}

#[tokio::test]
async fn handoff_is_per_user() {
    let (app, db) = app().await;
    let a = admin(&app).await;
    let b = user(&app, &a, "sam@example.test").await;
    let (_, v, _) = call(&app, "POST", "/api/v1/sync/hae/handoff", Auth::Cookie(&b), None).await;
    let code = code_of(v["link"].as_str().unwrap());
    let id = v["id"].as_str().unwrap().to_string();
    // another user can't see its status
    let (s, _, _) = call(&app, "GET", &format!("/api/v1/sync/hae/handoff/{id}"), Auth::Cookie(&a), None).await;
    assert_eq!(s, 404);
    // a's own handoff doesn't cancel b's
    call(&app, "POST", "/api/v1/sync/hae/handoff", Auth::Cookie(&a), None).await;
    let (s, v) = claim(&app, &code).await;
    assert_eq!(s, 200, "{v}");
    let uid: i64 = sqlx::query_scalar("SELECT user_id FROM upload_tokens").fetch_one(&db).await.unwrap();
    let sam: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'sam@example.test'").fetch_one(&db).await.unwrap();
    assert_eq!(uid, sam, "token belongs to the user who made the link");
}
