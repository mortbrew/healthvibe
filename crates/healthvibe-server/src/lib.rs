//! healthvibe-server: the Axum application.

mod api;

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use healthvibe_core::config::Config;
use serde_json::json;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: sqlx::SqlitePool,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .nest("/api/v1", api::routes())
        .fallback_service(spa(&state.config.web_dir))
        .with_state(state)
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
    let app = router(AppState { config, db });
    axum::serve(listener, app)
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
