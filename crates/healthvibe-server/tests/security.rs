//! Upload size limit, staging cleanup, security headers, trusted proxies.
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

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("hv-sec-{tag}-{:016x}", rand_u64()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn rand_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
    h.finish()
}

async fn app_with(f: impl FnOnce(&mut Config)) -> (Router, sqlx::SqlitePool, Config) {
    let db = healthvibe_core::db::open_memory().await.unwrap();
    let mut config = Config::from_env();
    config.web_dir = std::env::temp_dir().join("hv-no-web");
    f(&mut config);
    (
        router(AppState {
            config: config.clone(),
            db: db.clone(),
        }),
        db,
        config,
    )
}

async fn setup(app: &Router) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/setup")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": "toby", "password": "admin password 1"}).to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), 200);
    res.headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

fn multipart(file: &[u8]) -> (String, Vec<u8>) {
    let b = "hvboundary";
    let mut body = format!(
        "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"export.zip\"\r\nContent-Type: application/zip\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(file);
    body.extend_from_slice(format!("\r\n--{b}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={b}"), body)
}

async fn upload(app: &Router, cookie: &str, file: &[u8], with_len: bool) -> (StatusCode, Value) {
    let (ct, body) = multipart(file);
    let mut req = Request::builder()
        .method("POST")
        .uri("/api/v1/import")
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, ct);
    if with_len {
        req = req.header(header::CONTENT_LENGTH, body.len());
    }
    let res = app
        .clone()
        .oneshot(req.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let s = res.status();
    let b = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

fn staged(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("upload-"))
        .collect()
}

#[tokio::test]
async fn upload_over_limit_is_413_and_leaves_no_file() {
    let dir = temp_dir("limit");
    let (app, _db, _) = app_with(|c| {
        c.staging_dir = dir.clone();
        c.max_import_bytes = 1000;
    })
    .await;
    let cookie = setup(&app).await;
    // Announced too large: rejected before reading.
    let big = vec![b'x'; 200_000];
    let (s, v) = upload(&app, &cookie, &big, true).await;
    assert_eq!((s, v["error"].clone()), (StatusCode::PAYLOAD_TOO_LARGE, json!("upload_too_large")));
    // No Content-Length (streamed): rejected while writing, partial file removed.
    let (s, v) = upload(&app, &cookie, &vec![b'x'; 5000], false).await;
    assert_eq!((s, v["error"].clone()), (StatusCode::PAYLOAD_TOO_LARGE, json!("upload_too_large")));
    assert!(staged(&dir).is_empty());
    // Under the limit is accepted (the bad zip then fails in the import job).
    let (s, v) = upload(&app, &cookie, b"not really a zip", true).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert!(v["job_id"].is_i64());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn default_import_limit_is_4_gib() {
    assert_eq!(
        healthvibe_core::config::DEFAULT_MAX_IMPORT_BYTES,
        4 * 1024 * 1024 * 1024
    );
}

#[tokio::test]
async fn startup_cleanup_removes_leftovers_only() {
    let dir = temp_dir("cleanup");
    let db = healthvibe_core::db::open_memory().await.unwrap();
    for n in ["upload-00000000deadbeef.zip", "x.tmp", "y.part"] {
        std::fs::write(dir.join(n), b"synthetic").unwrap();
    }
    std::fs::write(dir.join("keep.txt"), b"not ours").unwrap();
    std::fs::create_dir(dir.join("upload-dir")).unwrap();
    sqlx::query("INSERT INTO users (username, password_hash) VALUES ('toby', 'x')")
        .execute(&db)
        .await
        .unwrap();
    let job = healthvibe_core::import::create_job(&db, 1).await.unwrap();
    let n = healthvibe_core::import::cleanup_staging(&db, &dir).await.unwrap();
    assert_eq!(n, 3);
    let mut left: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, vec!["keep.txt", "upload-dir"]);
    let status: String = sqlx::query_scalar("SELECT status FROM import_jobs WHERE id = ?")
        .bind(job)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(status, "failed");
    let _ = std::fs::remove_dir_all(&dir);
}

async fn get(app: &Router, uri: &str) -> axum::http::Response<Body> {
    app.clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn security_headers_on_api_and_spa() {
    let web = temp_dir("web");
    std::fs::write(web.join("index.html"), "<!doctype html><div id=root></div>").unwrap();
    let (app, _db, _) = app_with(|c| {
        c.web_dir = web.clone();
        c.hsts_max_age = None;
    })
    .await;
    for uri in ["/healthz", "/api/v1/session", "/", "/some/client/route"] {
        let res = get(&app, uri).await;
        let h = res.headers();
        let csp = h[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
        assert!(csp.contains("default-src 'self'"), "{uri}");
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(csp.contains("object-src 'none'"));
        assert!(!csp.contains("unsafe-inline") && !csp.contains("unsafe-eval"));
        assert_eq!(h[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(h[header::REFERRER_POLICY], "no-referrer");
        assert_eq!(h[header::X_FRAME_OPTIONS], "DENY");
        assert!(h.contains_key("permissions-policy"));
        assert!(!h.contains_key(header::STRICT_TRANSPORT_SECURITY), "HSTS is opt-in");
    }
    let (app, _db, _) = app_with(|c| c.hsts_max_age = Some(31536000)).await;
    let res = get(&app, "/healthz").await;
    assert_eq!(res.headers()[header::STRICT_TRANSPORT_SECURITY], "max-age=31536000");
    let _ = std::fs::remove_dir_all(&web);
}
