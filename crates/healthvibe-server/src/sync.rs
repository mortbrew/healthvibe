//! iPhone Shortcut sync (docs/shortcut-design.md).
//!
//! Web app (session cookie):
//! - `GET/PUT /sync`: metric catalog and what this user syncs.
//! - `POST /sync/setup-code`: one-time code + `healthvibe://setup` link for QR code 2.
//! - `DELETE /sync/token`: forget the paired iPhone.
//!
//! Shortcut (no cookie):
//! - `POST /shortcut/claim`: setup code → upload token (rate limited per IP).
//! - `GET /shortcut/config`: bearer token → enabled metrics, `since`, limits.
//! - `POST /ingest`: bearer token → samples (rate limited per token).

use crate::api::{bad, ApiError, ApiResult, User};
use crate::AppState;
use axum::{
    body::Bytes,
    extract::{ConnectInfo, DefaultBodyLimit, FromRequestParts, State},
    http::{header, request::Parts, HeaderMap, StatusCode},
    routing::{delete, get, post, put},
    Json, Router,
};
use healthvibe_core::shortcut::{self as sc, IngestError, SettingsError};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sync", get(get_sync).put(put_sync))
        .route("/sync/setup-code", post(setup_code))
        .route("/sync/token", delete(revoke))
        .route("/shortcut/claim", post(claim))
        .route("/shortcut/config", get(config))
        // The size limit is applied inside `ingest` from the config.
        .route("/ingest", post(ingest).layer(DefaultBodyLimit::disable()))
        // Health Auto Export (v1 sync path).
        .route("/sync/hae", get(hae_info))
        .route("/sync/hae/token", post(hae_token))
        .route("/me/onboarding", put(onboarding))
        // Phone handoff (QR code → copy page on the iPhone).
        .route("/sync/hae/handoff", post(handoff_create))
        .route("/sync/hae/handoff/{id}", get(handoff_status))
        .route("/handoff/claim", post(handoff_claim))
        .route("/ingest/hae", post(hae_ingest).layer(DefaultBodyLimit::disable()))
}

// ------------------------------------------------------------ rate limits

/// Fixed-window counter: at most `max` hits per `window` per key.
fn allow(key: String, max: u32, window: Duration) -> bool {
    static HITS: OnceLock<Mutex<HashMap<String, (Instant, u32)>>> = OnceLock::new();
    let mut m = HITS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    if m.len() > 10_000 {
        m.retain(|_, (t, _)| now.duration_since(*t) < window);
    }
    let e = m.entry(key).or_insert((now, 0));
    if now.duration_since(e.0) >= window {
        *e = (now, 0);
    }
    e.1 += 1;
    e.1 <= max
}

const CLAIMS_PER_10_MIN: u32 = 10;
const UPLOADS_PER_HOUR: u32 = 120;

fn too_many() -> ApiError {
    ApiError(StatusCode::TOO_MANY_REQUESTS, "rate_limited")
}

/// Client address: the last X-Forwarded-For hop (added by the reverse
/// proxy, so it can't be forged by the client) or the TCP peer.
fn client_ip(p: &Parts) -> String {
    if let Some(ip) = p
        .headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        return ip;
    }
    p.extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip().to_string())
        .unwrap_or_else(|| "unknown".into())
}

pub struct ClientIp(String);

impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = std::convert::Infallible;
    async fn from_request_parts(p: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(ClientIp(client_ip(p)))
    }
}

/// A request from the Shortcut carrying `Authorization: Bearer hvu_…`.
pub struct Device {
    user_id: i64,
    token_prefix: String,
    version: Option<String>,
}

impl FromRequestParts<AppState> for Device {
    type Rejection = ApiError;
    async fn from_request_parts(p: &mut Parts, st: &AppState) -> Result<Self, ApiError> {
        let unauth = ApiError(StatusCode::UNAUTHORIZED, "invalid_token");
        // Authorization: Bearer hvu_…, or X-Healthvibe-Token / api-key: hvu_…
        // for apps that only let you add a plain header.
        let h = |n: &str| p.headers.get(n).and_then(|v| v.to_str().ok());
        let tok = h(header::AUTHORIZATION.as_str())
            .and_then(|v| v.strip_prefix("Bearer ").or(v.strip_prefix("bearer ")))
            .or_else(|| h("x-healthvibe-token"))
            .or_else(|| h("api-key"))
            .map(str::trim)
            .ok_or(unauth)?;
        let version = p
            .headers
            .get("x-hv-version")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.trim().to_string());
        if let Some(v) = &version {
            if sc::version_lt(v, sc::MIN_SHORTCUT_VERSION) {
                return Err(ApiError(StatusCode::UPGRADE_REQUIRED, "shortcut_too_old"));
            }
        }
        let user_id = sc::token_user(&st.db, tok)
            .await?
            .ok_or(ApiError(StatusCode::UNAUTHORIZED, "invalid_token"))?;
        Ok(Device {
            user_id,
            token_prefix: tok.chars().take(16).collect(),
            version,
        })
    }
}

// ------------------------------------------------------------ web app

/// Address to put in the QR code.
fn server_url(st: &AppState, h: &HeaderMap) -> (String, bool) {
    if let Some(u) = &st.config.public_url {
        return (u.clone(), true);
    }
    let host = h
        .get("x-forwarded-host")
        .or(h.get(header::HOST))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost:8000");
    let scheme = h
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(if st.config.allow_http {
            "http"
        } else {
            "https"
        });
    (format!("{scheme}://{host}"), false)
}

async fn get_sync(State(st): State<AppState>, User(u): User, h: HeaderMap) -> ApiResult {
    let s = sc::settings(&st.db, u.id).await?;
    let device = sc::token_info(&st.db, u.id).await?;
    let (server, public_url_set) = server_url(&st, &h);
    Ok(Json(json!({
        "metrics": sc::METRICS,
        "configured": s.configured,
        "enabled": s.enabled,
        "trigger": s.trigger,
        "notify": s.notify,
        "triggers": sc::TRIGGERS,
        "device": device,
        "server": server,
        "public_url_set": public_url_set,
        "shortcut_version": sc::SHORTCUT_VERSION,
    })))
}

#[derive(Deserialize)]
struct SyncIn {
    enabled: Vec<String>,
    trigger: Option<String>,
    notify: Option<String>,
}

async fn put_sync(State(st): State<AppState>, User(u): User, Json(b): Json<SyncIn>) -> ApiResult {
    let cur = sc::settings(&st.db, u.id).await?;
    let trigger = b.trigger.unwrap_or(cur.trigger);
    let notify = b.notify.unwrap_or(cur.notify);
    match sc::save_settings(&st.db, u.id, &b.enabled, &trigger, &notify).await? {
        Ok(()) => {}
        Err(SettingsError::NoMetrics) => return Err(bad("no_metrics")),
        Err(SettingsError::UnknownMetric(_)) => return Err(bad("unknown_metric")),
        Err(SettingsError::BadTrigger) => return Err(bad("bad_trigger")),
        Err(SettingsError::BadNotify) => return Err(bad("bad_notify")),
    }
    let s = sc::settings(&st.db, u.id).await?;
    Ok(Json(
        json!({ "configured": s.configured, "enabled": s.enabled,
                    "trigger": s.trigger, "notify": s.notify }),
    ))
}

fn pct(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                o.push(b as char)
            }
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

async fn setup_code(State(st): State<AppState>, User(u): User, h: HeaderMap) -> ApiResult {
    if !sc::settings(&st.db, u.id).await?.configured {
        return Err(ApiError(StatusCode::CONFLICT, "choose_metrics_first"));
    }
    let (code, expires_at) = sc::new_setup_code(&st.db, u.id).await?;
    let (server, public_url_set) = server_url(&st, &h);
    let link = format!("healthvibe://setup?server={}&code={}", pct(&server), code);
    Ok(Json(
        json!({ "code": code, "expires_at": expires_at, "server": server,
                    "public_url_set": public_url_set, "link": link }),
    ))
}

async fn revoke(State(st): State<AppState>, User(u): User) -> ApiResult {
    let revoked = sc::revoke_token(&st.db, u.id).await?;
    Ok(Json(json!({ "revoked": revoked })))
}

// ------------------------------------------------------------ Shortcut

#[derive(Deserialize)]
struct ClaimIn {
    setup_code: String,
    device_name: Option<String>,
    shortcut_version: Option<String>,
}

async fn claim(State(st): State<AppState>, ClientIp(ip): ClientIp, body: Bytes) -> ApiResult {
    if !allow(
        format!("claim:{ip}"),
        CLAIMS_PER_10_MIN,
        Duration::from_secs(600),
    ) {
        return Err(too_many());
    }
    let b: ClaimIn = serde_json::from_slice(&body).map_err(|_| bad("bad_json"))?;
    let version = b
        .shortcut_version
        .unwrap_or_else(|| sc::SHORTCUT_VERSION.into());
    if sc::version_lt(&version, sc::MIN_SHORTCUT_VERSION) {
        return Err(ApiError(StatusCode::UPGRADE_REQUIRED, "shortcut_too_old"));
    }
    let device = b.device_name.unwrap_or_else(|| "iPhone".into());
    let Some((uid, token)) = sc::claim(&st.db, &b.setup_code, &device, &version).await? else {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "invalid_or_expired_code",
        ));
    };
    tracing::info!(user = uid, "shortcut paired");
    let config = sc::config_json(&st.db, uid, st.config.max_request_bytes).await?;
    Ok(Json(json!({ "upload_token": token, "config": config })))
}

async fn config(State(st): State<AppState>, d: Device) -> ApiResult {
    sc::touch_token(&st.db, d.user_id, d.version.as_deref()).await?;
    Ok(Json(
        sc::config_json(&st.db, d.user_id, st.config.max_request_bytes).await?,
    ))
}

async fn ingest(
    State(st): State<AppState>,
    d: Device,
    body: axum::body::Body,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let body = axum::body::to_bytes(body, st.config.max_request_bytes)
        .await
        .map_err(|_| ApiError(StatusCode::PAYLOAD_TOO_LARGE, "request_too_large"))?;
    if !allow(
        format!("ingest:{}", d.token_prefix),
        UPLOADS_PER_HOUR,
        Duration::from_secs(3600),
    ) {
        return Err(too_many());
    }
    match sc::ingest(&st.db, d.user_id, &body).await? {
        Ok(r) => Ok((StatusCode::OK, Json(serde_json::to_value(r)?))),
        Err(IngestError::BadJson(e)) => Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "bad_json", "detail": e })),
        )),
        Err(IngestError::UnknownMetric(k)) => Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": "unknown_metric", "metric": k })),
        )),
    }
}

// ------------------------------------------------------------ Health Auto Export

use healthvibe_core::hae;

const HAE_UPLOADS_PER_HOUR: u32 = 2000;

fn hae_url(st: &AppState, h: &HeaderMap) -> (String, bool) {
    let (server, set) = server_url(st, h);
    (format!("{}{}", server.trim_end_matches('/'), hae::PATH), set)
}

async fn hae_info(State(st): State<AppState>, User(u): User, h: HeaderMap) -> ApiResult {
    let device = sc::token_info(&st.db, u.id).await?;
    let received = hae::received(&st.db, u.id).await?;
    let (url, public_url_set) = hae_url(&st, &h);
    Ok(Json(json!({
        "url": url,
        "public_url_set": public_url_set,
        "header": hae::TOKEN_HEADER,
        "has_token": device.is_some(),
        "device": device,
        "received": received.unwrap_or(0),
        "max_request_bytes": st.config.hae_max_request_bytes,
    })))
}

/// Create a new token (replaces any earlier one). Shown once.
async fn hae_token(State(st): State<AppState>, User(u): User, h: HeaderMap) -> ApiResult {
    let token = sc::new_token(&st.db, u.id, hae::DEVICE).await?;
    let (url, public_url_set) = hae_url(&st, &h);
    tracing::info!(user = u.id, "health auto export token created");
    Ok(Json(json!({
        "token": token,
        "url": url,
        "public_url_set": public_url_set,
        "header": hae::TOKEN_HEADER,
    })))
}

#[derive(Deserialize)]
struct OnboardingIn {
    done: bool,
}

async fn onboarding(State(st): State<AppState>, User(u): User, Json(b): Json<OnboardingIn>) -> ApiResult {
    sqlx::query("UPDATE users SET onboarding_done = ? WHERE id = ?")
        .bind(b.done)
        .bind(u.id)
        .execute(&st.db)
        .await
        .map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "onboarding_done": b.done })))
}

async fn hae_ingest(
    State(st): State<AppState>,
    d: Device,
    body: axum::body::Body,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let body = axum::body::to_bytes(body, st.config.hae_max_request_bytes)
        .await
        .map_err(|_| ApiError(StatusCode::PAYLOAD_TOO_LARGE, "request_too_large"))?;
    if !allow(
        format!("hae:{}", d.token_prefix),
        HAE_UPLOADS_PER_HOUR,
        Duration::from_secs(3600),
    ) {
        return Err(too_many());
    }
    match hae::ingest(&st.db, d.user_id, &body).await? {
        Ok(r) => {
            tracing::info!(user = d.user_id, accepted = r.accepted, updated = r.updated, "health auto export upload");
            Ok((StatusCode::OK, Json(json!(r))))
        }
        Err(e) => Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "bad_json", "detail": e })),
        )),
    }
}

// ------------------------------------------------------------ phone handoff

const HANDOFFS_PER_10_MIN: u32 = 10;

async fn handoff_create(State(st): State<AppState>, User(u): User, h: HeaderMap) -> ApiResult {
    if !allow(format!("handoff:{}", u.id), HANDOFFS_PER_10_MIN, Duration::from_secs(600)) {
        return Err(too_many());
    }
    let n = healthvibe_core::handoff::create(&st.db, u.id).await?;
    let (server, public_url_set) = server_url(&st, &h);
    let link = format!("{}/h/{}", server.trim_end_matches('/'), n.code);
    Ok(Json(json!({
        "id": n.id, "link": link, "expires_at": n.expires_at, "public_url_set": public_url_set,
    })))
}

async fn handoff_status(
    State(st): State<AppState>,
    User(u): User,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> ApiResult {
    match healthvibe_core::handoff::status(&st.db, u.id, &id).await? {
        Some(s) => Ok(Json(json!(s))),
        None => Err(ApiError(StatusCode::NOT_FOUND, "not_found")),
    }
}

#[derive(Deserialize)]
struct HandoffIn {
    code: String,
}

async fn handoff_claim(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    h: HeaderMap,
    body: Bytes,
) -> ApiResult<([(header::HeaderName, &'static str); 1], Json<Value>)> {
    if !allow(format!("handoff-claim:{ip}"), CLAIMS_PER_10_MIN, Duration::from_secs(600)) {
        return Err(too_many());
    }
    let b: HandoffIn = serde_json::from_slice(&body).map_err(|_| bad("bad_json"))?;
    use healthvibe_core::handoff::Claim;
    match healthvibe_core::handoff::claim(&st.db, b.code.trim()).await? {
        Claim::Ok(uid, token) => {
            tracing::info!(user = uid, "phone handoff opened; new health auto export token");
            let (url, public_url_set) = hae_url(&st, &h);
            Ok((
                [(header::CACHE_CONTROL, "no-store")],
                Json(json!({
                    "token": token, "url": url, "public_url_set": public_url_set,
                    "header": hae::TOKEN_HEADER,
                })),
            ))
        }
        Claim::Invalid => Err(ApiError(StatusCode::GONE, "handoff_invalid")),
    }
}
