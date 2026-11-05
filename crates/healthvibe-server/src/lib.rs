//! healthvibe-server: the Axum application.

mod api;
mod sync;

use axum::{
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::Response,
    routing::get,
    Json, Router,
};
use healthvibe_core::config::Config;
use serde_json::json;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: sqlx::SqlitePool,
}

pub fn router(state: AppState) -> Router {
    let hsts = state
        .config
        .hsts_max_age
        .and_then(|s| HeaderValue::from_str(&format!("max-age={s}")).ok());
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .nest("/api/v1", api::routes())
        .fallback_service(spa(&state.config.web_dir))
        .with_state(state)
        .layer(axum::middleware::map_response(move |res: Response| {
            let hsts = hsts.clone();
            async move { security_headers(res, hsts) }
        }))
}

/// Content-Security-Policy for the built SPA: everything from this origin, no
/// inline or eval'd script, no framing, no plugins. `data:`/`blob:` images
/// cover the inline QR code and any generated images.
pub const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; \
img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self'; \
object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'";

/// Security headers on every response (API and SPA alike).
fn security_headers(mut res: Response, hsts: Option<HeaderValue>) -> Response {
    let h = res.headers_mut();
    let set = |h: &mut axum::http::HeaderMap, k: header::HeaderName, v: &'static str| {
        h.entry(k).or_insert(HeaderValue::from_static(v));
    };
    set(h, header::CONTENT_SECURITY_POLICY, CSP);
    set(h, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(h, header::REFERRER_POLICY, "no-referrer");
    set(h, header::X_FRAME_OPTIONS, "DENY");
    set(
        h,
        header::HeaderName::from_static("permissions-policy"),
        "camera=(), microphone=(), geolocation=(), payment=(), usb=(), interest-cohort=()",
    );
    set(
        h,
        header::HeaderName::from_static("cross-origin-opener-policy"),
        "same-origin",
    );
    if let Some(v) = hsts {
        h.entry(header::STRICT_TRANSPORT_SECURITY).or_insert(v);
    }
    res
}

/// Serve the built SPA; unknown paths get index.html (client-side routing).
fn spa(dir: &std::path::Path) -> ServeDir<ServeFile> {
    ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")))
}

/// Liveness: the process is up.
async fn healthz() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

/// Readiness: the database answers.
async fn readyz(State(st): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    match sqlx::query("SELECT 1").execute(&st.db).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "ready" }))),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "db unavailable" })),
        ),
    }
}

pub async fn serve(config: Config) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!(bind = %config.bind, "healthvibe listening");
    let db = healthvibe_core::db::open(&config).await?;
    let removed = healthvibe_core::import::cleanup_staging(&db, &config.staging_dir).await?;
    if removed > 0 {
        tracing::info!(removed, "removed leftover staged uploads");
    }
    let app = router(AppState { config, db });
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn healthz_ok() {
        let app = router(AppState {
            config: Config::from_env(),
            db: healthvibe_core::db::open_memory().await.unwrap(),
        });
        let res = app
            .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
    }
}
