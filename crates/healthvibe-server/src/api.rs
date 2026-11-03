//! JSON API under `/api/v1`, used by the SPA.
//!
//! Auth is a session cookie (HttpOnly, SameSite=Strict). State-changing
//! requests must also be JSON or multipart (not form posts), which together
//! with SameSite=Strict blocks cross-site request forgery.
//!
//! Two extractors enforce access:
//! - [`AnyUser`]: any signed-in user, even one who still has to replace a
//!   temporary password. Only `session`, `me/password` and `logout` use it.
//! - [`User`]: everything else. Returns 403 `password_change_required` until
//!   the temporary password is changed.

use crate::AppState;
use axum::{
    extract::{DefaultBodyLimit, FromRequestParts, Multipart, Path, Query, State},
    http::{header, request::Parts, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use healthvibe_core::{auth, import, stats};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

pub const COOKIE: &str = "healthvibe_session";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/session", get(session))
        .route("/setup", post(setup))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me/password", post(change_password))
        .route("/me/theme", put(set_theme))
        .route("/admin/users", get(list_users).post(create_user))
        .route(
            "/import",
            // Apple exports can exceed 1 GB; the body is streamed to disk.
            post(upload).layer(DefaultBodyLimit::disable()),
        )
        .route("/imports", get(list_imports))
        .route("/import/{id}", get(import_status))
        .route("/dashboard/activity", get(activity))
        .route("/dashboard/heart", get(heart))
        .route("/dashboard/sleep", get(sleep))
        .route("/dashboard/body", get(body))
}

pub struct ApiError(StatusCode, &'static str);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl<E: std::fmt::Display> From<E> for ApiError {
    fn from(e: E) -> Self {
        tracing::error!(error = %e, "request failed");
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, "internal")
    }
}

type ApiResult<T = Json<Value>> = Result<T, ApiError>;

fn bad(code: &'static str) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, code)
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

fn set_cookie(st: &AppState, value: &str, max_age: i64) -> String {
    let secure = if st.config.allow_http { "" } else { "; Secure" };
    format!("{COOKIE}={value}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}{secure}")
}

/// Any signed-in user, including one who must still change their password.
pub struct AnyUser(pub auth::User);
/// A signed-in user who is allowed to use the app.
pub struct User(pub auth::User);

impl FromRequestParts<AppState> for AnyUser {
    type Rejection = ApiError;
    async fn from_request_parts(p: &mut Parts, st: &AppState) -> Result<Self, ApiError> {
        let unauth = ApiError(StatusCode::UNAUTHORIZED, "unauthenticated");
        let t = cookie_token(&p.headers).ok_or(unauth)?;
        let id = auth::session_user(&st.db, &t)
            .await?
            .ok_or(ApiError(StatusCode::UNAUTHORIZED, "unauthenticated"))?;
        let u = auth::get_user(&st.db, id)
            .await?
            .ok_or(ApiError(StatusCode::UNAUTHORIZED, "unauthenticated"))?;
        Ok(AnyUser(u))
    }
}

impl FromRequestParts<AppState> for User {
    type Rejection = ApiError;
    async fn from_request_parts(p: &mut Parts, st: &AppState) -> Result<Self, ApiError> {
        let AnyUser(u) = AnyUser::from_request_parts(p, st).await?;
        if u.must_change_password {
            return Err(ApiError(StatusCode::FORBIDDEN, "password_change_required"));
        }
        Ok(User(u))
    }
}

async fn session(State(st): State<AppState>, h: HeaderMap) -> ApiResult {
    let needs_setup = auth::needs_setup(&st.db).await?;
    let user = match cookie_token(&h) {
        Some(t) => match auth::session_user(&st.db, &t).await? {
            Some(id) => auth::get_user(&st.db, id).await?,
            None => None,
        },
        None => None,
    };
    Ok(Json(json!({ "needs_setup": needs_setup, "user": user })))
}

#[derive(Deserialize)]
struct Credentials {
    username: String,
    password: String,
}

async fn start_session(st: &AppState, id: i64) -> ApiResult<Response> {
    let tok = auth::create_session(&st.db, id).await?;
    let user = auth::get_user(&st.db, id).await?;
    Ok((
        [(header::SET_COOKIE, set_cookie(st, &tok, 60 * 60 * 24 * 30))],
        Json(json!({ "user": user })),
    )
        .into_response())
}

async fn setup(State(st): State<AppState>, Json(c): Json<Credentials>) -> ApiResult<Response> {
    let username = c.username.trim().to_lowercase();
    if !auth::valid_username(&username) {
        return Err(bad("invalid_username"));
    }
    if c.password.chars().count() < auth::MIN_PASSWORD {
        return Err(bad("password_too_short"));
    }
    if !auth::setup_admin(&st.db, &username, &c.password).await? {
        return Err(ApiError(StatusCode::CONFLICT, "already_set_up"));
    }
    start_session(&st, 1).await
}

async fn login(State(st): State<AppState>, Json(c): Json<Credentials>) -> ApiResult<Response> {
    match auth::login(&st.db, c.username.trim(), &c.password).await? {
        Some(id) => start_session(&st, id).await,
        None => Err(ApiError(StatusCode::UNAUTHORIZED, "invalid_credentials")),
    }
}

async fn logout(State(st): State<AppState>, h: HeaderMap, _u: AnyUser) -> ApiResult<Response> {
    if let Some(t) = cookie_token(&h) {
        auth::delete_session(&st.db, &t).await?;
    }
    Ok((
        [(header::SET_COOKIE, set_cookie(&st, "", 0))],
        Json(json!({ "ok": true })),
    )
        .into_response())
}

#[derive(Deserialize)]
struct PasswordChange {
    current: String,
    new: String,
}

async fn change_password(
    State(st): State<AppState>,
    AnyUser(u): AnyUser,
    Json(b): Json<PasswordChange>,
) -> ApiResult {
    if b.new.chars().count() < auth::MIN_PASSWORD {
        return Err(bad("password_too_short"));
    }
    if !auth::change_password(&st.db, u.id, &b.current, &b.new).await? {
        return Err(bad("wrong_password_or_unchanged"));
    }
    Ok(Json(json!({ "user": auth::get_user(&st.db, u.id).await? })))
}

#[derive(Deserialize)]
struct Theme {
    mode: String,
    light: String,
    dark: String,
}

const LIGHT: &[&str] = &["daylight", "paper", "mint"];
const DARK: &[&str] = &["midnight", "fitness", "graphite"];

async fn set_theme(State(st): State<AppState>, User(u): User, Json(t): Json<Theme>) -> ApiResult {
    if !["system", "light", "dark"].contains(&t.mode.as_str())
        || !LIGHT.contains(&t.light.as_str())
        || !DARK.contains(&t.dark.as_str())
    {
        return Err(bad("invalid_theme"));
    }
    sqlx::query("UPDATE users SET theme_mode = ?, theme_light = ?, theme_dark = ? WHERE id = ?")
        .bind(&t.mode)
        .bind(&t.light)
        .bind(&t.dark)
        .bind(u.id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "user": auth::get_user(&st.db, u.id).await? })))
}

fn admin(u: &auth::User) -> Result<(), ApiError> {
    if u.is_admin {
        Ok(())
    } else {
        Err(ApiError(StatusCode::FORBIDDEN, "admin_only"))
    }
}

async fn list_users(State(st): State<AppState>, User(u): User) -> ApiResult {
    admin(&u)?;
    Ok(Json(json!({ "users": auth::list_users(&st.db).await? })))
}

#[derive(Deserialize)]
struct NewUser {
    username: String,
    temporary_password: String,
    #[serde(default)]
    is_admin: bool,
}

async fn create_user(
    State(st): State<AppState>,
    User(u): User,
    Json(n): Json<NewUser>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    admin(&u)?;
    let username = n.username.trim().to_lowercase();
    if !auth::valid_username(&username) {
        return Err(bad("invalid_username"));
    }
    if n.temporary_password.chars().count() < auth::MIN_PASSWORD {
        return Err(bad("password_too_short"));
    }
    match auth::create_user(&st.db, &username, &n.temporary_password, n.is_admin).await? {
        Some(id) => Ok((
            StatusCode::CREATED,
            Json(json!({ "user": auth::get_user(&st.db, id).await? })),
        )),
        None => Err(ApiError(StatusCode::CONFLICT, "username_taken")),
    }
}

async fn upload(State(st): State<AppState>, User(u): User, mut mp: Multipart) -> ApiResult {
    let path = st
        .config
        .staging_dir
        .join(format!("upload-{:016x}.zip", rand::random::<u64>()));
    let mut got = false;
    let res: anyhow::Result<()> = async {
        while let Some(mut field) = mp.next_field().await? {
            if field.name() != Some("file") {
                continue;
            }
            let mut f = tokio::fs::File::create(&path).await?;
            while let Some(chunk) = field.chunk().await? {
                f.write_all(&chunk).await?;
            }
            f.flush().await?;
            got = true;
            break;
        }
        Ok(())
    }
    .await;
    if let Err(e) = res {
        let _ = tokio::fs::remove_file(&path).await;
        tracing::warn!(error = %e, "upload failed");
        return Err(bad("upload_failed"));
    }
    if !got {
        return Err(bad("missing_file"));
    }
    let job = import::create_job(&st.db, u.id).await?;
    let db = st.db.clone();
    let uid = u.id;
    tokio::spawn(async move { import::run_job(&db, job, uid, &path).await });
    Ok(Json(json!({ "job_id": job })))
}

fn job_json(
    r: (
        i64,
        String,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
    ),
) -> Value {
    let (id, status, counts, error, created_at, finished_at) = r;
    let counts: Option<Value> = counts.and_then(|c| serde_json::from_str(&c).ok());
    json!({ "id": id, "status": status, "counts": counts, "error": error,
            "created_at": created_at, "finished_at": finished_at })
}

const JOB_COLS: &str = "id, status, counts_json, error, created_at, finished_at";

async fn import_status(
    State(st): State<AppState>,
    User(u): User,
    Path(id): Path<i64>,
) -> ApiResult {
    let row = sqlx::query_as(&format!(
        "SELECT {JOB_COLS} FROM import_jobs WHERE id = ? AND user_id = ?"
    ))
    .bind(id)
    .bind(u.id)
    .fetch_optional(&st.db)
    .await?
    .ok_or(ApiError(StatusCode::NOT_FOUND, "not_found"))?;
    Ok(Json(job_json(row)))
}

async fn list_imports(State(st): State<AppState>, User(u): User) -> ApiResult {
    let rows = sqlx::query_as(&format!(
        "SELECT {JOB_COLS} FROM import_jobs WHERE user_id = ? ORDER BY id DESC LIMIT 20"
    ))
    .bind(u.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(
        json!({ "imports": rows.into_iter().map(job_json).collect::<Vec<_>>() }),
    ))
}

#[derive(Deserialize)]
struct Range {
    /// Last day to include (YYYY-MM-DD); defaults to the latest day with data.
    end: Option<String>,
    days: Option<i64>,
}

impl Range {
    fn days(&self, default: i64) -> i64 {
        self.days.unwrap_or(default).clamp(1, 366)
    }
    fn end_valid(&self) -> Result<Option<String>, ApiError> {
        match &self.end {
            Some(e) if chrono_ok(e) => Ok(Some(e.clone())),
            Some(_) => Err(bad("invalid_date")),
            None => Ok(None),
        }
    }
    async fn end(&self, st: &AppState, uid: i64) -> Result<Option<String>, ApiError> {
        match self.end_valid()? {
            Some(e) => Ok(Some(e)),
            None => Ok(stats::latest_day(&st.db, uid).await?),
        }
    }
}

fn chrono_ok(s: &str) -> bool {
    s.len() == 10
        && s.chars().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        })
}

async fn activity(State(st): State<AppState>, User(u): User, Query(r): Query<Range>) -> ApiResult {
    Ok(Json(json!(
        stats::activity(&st.db, u.id, r.end_valid()?).await?
    )))
}

async fn heart(State(st): State<AppState>, User(u): User, Query(r): Query<Range>) -> ApiResult {
    let Some(end) = r.end(&st, u.id).await? else {
        return Ok(Json(json!({ "daily": [], "resting": [], "hrv": [] })));
    };
    Ok(Json(json!(
        stats::heart(&st.db, u.id, &end, r.days(30)).await?
    )))
}

async fn sleep(State(st): State<AppState>, User(u): User, Query(r): Query<Range>) -> ApiResult {
    // Sleep is keyed by wake date, so default to the latest sleep sample.
    let end =
        match r.end_valid()? {
            Some(e) => Some(e),
            None => sqlx::query_scalar::<_, Option<String>>(
                "SELECT substr(MAX(local_end), 1, 10) FROM readings WHERE user_id = ? AND type = ?",
            )
            .bind(u.id)
            .bind(stats::SLEEP)
            .fetch_one(&st.db)
            .await?,
        };
    let Some(end) = end else {
        return Ok(Json(json!({ "nights": [], "last_night": [] })));
    };
    Ok(Json(json!(
        stats::sleep(&st.db, u.id, &end, r.days(14)).await?
    )))
}

async fn body(State(st): State<AppState>, User(u): User, Query(r): Query<Range>) -> ApiResult {
    let Some(end) = r.end(&st, u.id).await? else {
        return Ok(Json(json!({
            "weight": { "unit": null, "points": [] },
            "body_fat": { "unit": null, "points": [] },
            "lean_mass": { "unit": null, "points": [] } })));
    };
    Ok(Json(json!(
        stats::body(&st.db, u.id, &end, r.days(90)).await?
    )))
}
