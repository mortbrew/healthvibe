//! Minimal server-rendered pages: first-run setup, login, upload, import status.
//! (The React SPA replaces these in PR13.)

use crate::AppState;
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Router,
};
use healthvibe_core::{auth, import};
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

const COOKIE: &str = "healthvibe_session";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/setup", post(setup))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route(
            "/upload",
            // Apple exports can exceed 1 GB; the body is streamed to disk.
            post(upload).layer(DefaultBodyLimit::disable()),
        )
        .route("/import/{id}", get(import_status))
}

fn page(title: &str, body: &str) -> Html<String> {
    Html(format!(
        r#"<!doctype html><html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title} · healthvibe</title>
<style>body{{font-family:system-ui,sans-serif;max-width:40rem;margin:2rem auto;padding:0 1rem}}
input,button{{font-size:1rem;padding:.5rem;margin:.25rem 0}}table{{border-collapse:collapse}}
td,th{{padding:.25rem .75rem;border-bottom:1px solid #ccc;text-align:left}}</style>
</head><body><h1>healthvibe</h1>{body}</body></html>"#
    ))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn cookie_token(h: &HeaderMap) -> Option<String> {
    h.get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE)
        .map(|(_, v)| v.to_string())
}

async fn current_user(st: &AppState, h: &HeaderMap) -> Option<i64> {
    let t = cookie_token(h)?;
    auth::session_user(&st.db, &t).await.ok().flatten()
}

fn err500(e: impl std::fmt::Display) -> Response {
    tracing::error!(error = %e, "request failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

fn set_cookie(st: &AppState, value: &str, max_age: i64) -> String {
    let secure = if st.config.allow_http { "" } else { "; Secure" };
    format!("{COOKIE}={value}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}{secure}")
}

async fn index(State(st): State<AppState>, h: HeaderMap) -> Response {
    match auth::needs_setup(&st.db).await {
        Ok(true) => {
            return page(
                "Setup",
                r#"<h2>First run: set the admin password</h2>
<form method="post" action="/setup">
<p>Username: <b>admin</b></p>
<input type="password" name="password" placeholder="Password (12+ characters)" minlength="12" required autocomplete="new-password"><br>
<input type="password" name="confirm" placeholder="Repeat password" minlength="12" required autocomplete="new-password"><br>
<button>Set password</button></form>"#,
            )
            .into_response()
        }
        Err(e) => return err500(e),
        _ => {}
    }
    let Some(uid) = current_user(&st, &h).await else {
        return page(
            "Log in",
            r#"<form method="post" action="/login">
<input name="username" value="admin" autocomplete="username"><br>
<input type="password" name="password" placeholder="Password" required autocomplete="current-password"><br>
<button>Log in</button></form>"#,
        )
        .into_response();
    };
    let jobs: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, status, created_at FROM import_jobs WHERE user_id = ? ORDER BY id DESC LIMIT 10",
    )
    .bind(uid)
    .fetch_all(&st.db)
    .await
    .unwrap_or_default();
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM readings WHERE user_id = ?")
        .bind(uid)
        .fetch_one(&st.db)
        .await
        .unwrap_or(0);
    let list: String = jobs
        .iter()
        .map(|(id, s, c)| {
            format!(r#"<li><a href="/import/{id}">Import #{id}</a> — {s} — {c}</li>"#)
        })
        .collect();
    page(
        "Upload",
        &format!(
            r#"<p>Stored readings: <b>{total}</b></p>
<h2>Upload Apple Health export</h2>
<p>On iPhone: Health → profile picture → Export All Health Data, save <code>export.zip</code> to Files, then pick it here.</p>
<form method="post" action="/upload" enctype="multipart/form-data">
<input type="file" name="file" accept=".zip,application/zip" required><br>
<button>Upload and import</button></form>
<p>Large exports take a while to upload; keep this page open until it finishes.</p>
<h2>Recent imports</h2><ul>{list}</ul>
<form method="post" action="/logout"><button>Log out</button></form>"#
        ),
    )
    .into_response()
}

#[derive(Deserialize)]
struct SetupForm {
    password: String,
    confirm: String,
}

async fn setup(State(st): State<AppState>, Form(f): Form<SetupForm>) -> Response {
    if f.password != f.confirm || f.password.chars().count() < 12 {
        return page(
            "Setup",
            r#"<p>Passwords must match and be at least 12 characters. <a href="/">Back</a></p>"#,
        )
        .into_response();
    }
    match auth::setup_admin(&st.db, &f.password).await {
        Ok(true) => match auth::create_session(&st.db, 1).await {
            Ok(t) => login_redirect(&st, &t),
            Err(e) => err500(e),
        },
        Ok(false) => Redirect::to("/").into_response(),
        Err(e) => err500(e),
    }
}

fn login_redirect(st: &AppState, token: &str) -> Response {
    (
        [(
            header::SET_COOKIE,
            set_cookie(st, token, auth::SESSION_DAYS * 86400),
        )],
        Redirect::to("/"),
    )
        .into_response()
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

async fn login(State(st): State<AppState>, Form(f): Form<LoginForm>) -> Response {
    match auth::login(&st.db, &f.username, &f.password).await {
        Ok(Some(uid)) => match auth::create_session(&st.db, uid).await {
            Ok(t) => login_redirect(&st, &t),
            Err(e) => err500(e),
        },
        Ok(None) => (
            StatusCode::UNAUTHORIZED,
            page(
                "Log in",
                r#"<p>Wrong username or password. <a href="/">Try again</a></p>"#,
            ),
        )
            .into_response(),
        Err(e) => err500(e),
    }
}

async fn logout(State(st): State<AppState>, h: HeaderMap) -> Response {
    if let Some(t) = cookie_token(&h) {
        let _ = auth::delete_session(&st.db, &t).await;
    }
    (
        [(header::SET_COOKIE, set_cookie(&st, "", 0))],
        Redirect::to("/"),
    )
        .into_response()
}

async fn upload(State(st): State<AppState>, h: HeaderMap, mut mp: Multipart) -> Response {
    let Some(uid) = current_user(&st, &h).await else {
        return Redirect::to("/").into_response();
    };
    let path = st
        .config
        .staging_dir
        .join(format!("upload-{:016x}.zip", rand::random::<u64>()));
    let res: anyhow::Result<u64> = async {
        let mut written = 0u64;
        while let Some(mut field) = mp.next_field().await? {
            if field.name() != Some("file") {
                continue;
            }
            let mut f = tokio::fs::File::create(&path).await?;
            while let Some(chunk) = field.chunk().await? {
                f.write_all(&chunk).await?;
                written += chunk.len() as u64;
            }
            f.flush().await?;
        }
        Ok(written)
    }
    .await;
    match res {
        Ok(n) if n > 0 => {}
        other => {
            let _ = tokio::fs::remove_file(&path).await;
            if let Err(e) = other {
                tracing::warn!(error = %e, "upload failed");
            }
            return (
                StatusCode::BAD_REQUEST,
                page(
                    "Upload",
                    r#"<p>Upload failed or was empty. <a href="/">Back</a></p>"#,
                ),
            )
                .into_response();
        }
    }
    let job = match import::create_job(&st.db, uid).await {
        Ok(j) => j,
        Err(e) => {
            let _ = tokio::fs::remove_file(&path).await;
            return err500(e);
        }
    };
    let db = st.db.clone();
    tokio::spawn(async move { import::run_job(&db, job, uid, &path).await });
    Redirect::to(&format!("/import/{job}")).into_response()
}

async fn import_status(State(st): State<AppState>, h: HeaderMap, Path(id): Path<i64>) -> Response {
    let Some(uid) = current_user(&st, &h).await else {
        return Redirect::to("/").into_response();
    };
    let row: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT status, counts_json, error FROM import_jobs WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(uid)
    .fetch_optional(&st.db)
    .await
    .unwrap_or(None);
    let Some((status, counts, error)) = row else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    let body = match status.as_str() {
        "running" => r#"<meta http-equiv="refresh" content="3"><p>Importing… this page refreshes every 3 seconds.</p>"#.to_string(),
        "failed" => format!("<p>Import failed: {}</p>", esc(error.as_deref().unwrap_or("unknown"))),
        _ => {
            let c: import::Counts = counts
                .and_then(|j| serde_json::from_str(&j).ok())
                .unwrap_or_default();
            let (ti, td) = c.values().fold((0, 0), |a, t| (a.0 + t.inserted, a.1 + t.duplicates));
            let rows: String = c
                .iter()
                .map(|(k, t)| {
                    format!("<tr><td>{}</td><td>{}</td><td>{}</td></tr>", esc(k), t.inserted, t.duplicates)
                })
                .collect();
            format!(
                "<p>Done. New: <b>{ti}</b>, already stored: <b>{td}</b>. The uploaded zip has been deleted.</p>
<table><tr><th>Type</th><th>New</th><th>Duplicates</th></tr>{rows}</table>"
            )
        }
    };
    page(
        &format!("Import #{id}"),
        &format!(r#"<h2>Import #{id}</h2>{body}<p><a href="/">Home</a></p>"#),
    )
    .into_response()
}
