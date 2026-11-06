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
use healthvibe_core::{auth, browse, dashboards, import, manual_goals, profile, stats};
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
        .route("/dashboard/sleep/vitals", get(sleep_vitals))
        .route("/dashboard/body", get(body))
        .route("/metrics", get(metrics))
        .route("/series", get(series))
        .route("/browse", get(browse_list))
        .route("/records", get(records))
        .route("/hourly", get(hourly))
        .route("/profile", get(get_profile).put(put_profile))
        .route("/me/export.csv", get(export_csv))
        .route("/me/data", axum::routing::delete(delete_data))
        .route("/me/sessions/others", axum::routing::delete(revoke_others))
        .route(
            "/me/manual-goals",
            get(list_manual_goals).post(create_manual_goal),
        )
        .route(
            "/me/manual-goals/{id}",
            put(update_manual_goal).delete(delete_manual_goal),
        )
        .route("/workouts", get(workouts_list))
        .route("/workouts/{id}", get(workout_one))
        .route("/rings", get(rings))
        .route("/pinned", get(get_pinned).put(put_pinned))
        .route("/dashboards", get(list_dashboards).post(create_dashboard))
        .route(
            "/dashboards/{id}",
            get(get_dashboard)
                .put(update_dashboard)
                .delete(delete_dashboard),
        )
        .route("/nav", get(get_nav).put(put_nav))
        .merge(crate::sync::routes())
}

pub struct ApiError(pub StatusCode, pub &'static str);

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

pub type ApiResult<T = Json<Value>> = Result<T, ApiError>;

pub fn bad(code: &'static str) -> ApiError {
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
    let username = auth::normalize_username(&c.username);
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

async fn login(
    State(st): State<AppState>,
    client: crate::client::Client,
    Json(c): Json<Credentials>,
) -> ApiResult<Response> {
    let ip = client.ip;
    if !crate::sync::allow(
        format!("login-ip:{ip}"),
        60,
        std::time::Duration::from_secs(600),
    ) {
        return Err(ApiError(StatusCode::TOO_MANY_REQUESTS, "rate_limited"));
    }
    match auth::login(&st.db, &auth::normalize_username(&c.username), &c.password).await? {
        auth::LoginResult::Ok(id) => start_session(&st, id).await,
        auth::LoginResult::BadCredentials => {
            Err(ApiError(StatusCode::UNAUTHORIZED, "invalid_credentials"))
        }
        auth::LoginResult::Locked { .. } => {
            Err(ApiError(StatusCode::TOO_MANY_REQUESTS, "account_locked"))
        }
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

const LIGHT: &[&str] = &["health", "daylight", "paper", "mint"];
const DARK: &[&str] = &["health", "midnight", "fitness", "graphite"];

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
    let username = auth::normalize_username(&n.username);
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

async fn upload(
    State(st): State<AppState>,
    User(u): User,
    h: HeaderMap,
    mut mp: Multipart,
) -> ApiResult {
    let limit = st.config.max_import_bytes;
    let too_large = ApiError(StatusCode::PAYLOAD_TOO_LARGE, "upload_too_large");
    // Cheap early reject when the client announces the size (allowing a little
    // for multipart framing; the file bytes themselves are checked below).
    let announced = h
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if announced.is_some_and(|n| n > limit.saturating_add(64 * 1024)) {
        return Err(too_large);
    }
    let path = st
        .config
        .staging_dir
        .join(format!("upload-{:016x}.zip", rand::random::<u64>()));
    let mut got = false;
    let mut over = false;
    let res: anyhow::Result<()> = async {
        while let Some(mut field) = mp.next_field().await? {
            if field.name() != Some("file") {
                continue;
            }
            let mut f = tokio::fs::File::create(&path).await?;
            let mut written: u64 = 0;
            while let Some(chunk) = field.chunk().await? {
                written += chunk.len() as u64;
                if written > limit {
                    over = true;
                    return Ok(());
                }
                f.write_all(&chunk).await?;
            }
            f.flush().await?;
            got = true;
            break;
        }
        Ok(())
    }
    .await;
    if over {
        let _ = tokio::fs::remove_file(&path).await;
        tracing::warn!(limit, "upload over HEALTHVIBE_MAX_IMPORT_BYTES rejected");
        return Err(too_large);
    }
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

async fn sleep_vitals(
    State(st): State<AppState>,
    User(u): User,
    Query(r): Query<Range>,
) -> ApiResult {
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
        return Ok(Json(
            json!({ "date": null, "heart_rate": null, "respiratory_rate": null, "wrist_temperature": null, "heart_rate_series": [] }),
        ));
    };
    Ok(Json(json!(stats::sleep_vitals(&st.db, u.id, &end).await?)))
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

#[derive(Deserialize)]
struct WorkoutsQuery {
    limit: Option<i64>,
    before: Option<String>,
}

async fn workouts_list(
    State(st): State<AppState>,
    User(u): User,
    Query(q): Query<WorkoutsQuery>,
) -> ApiResult {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let rows = healthvibe_core::workouts::list(&st.db, u.id, limit, q.before.as_deref()).await?;
    Ok(Json(json!({ "workouts": rows })))
}

async fn workout_one(
    State(st): State<AppState>,
    User(u): User,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> ApiResult {
    match healthvibe_core::workouts::get(&st.db, u.id, id).await? {
        Some(w) => Ok(Json(json!(w))),
        None => Err(ApiError(StatusCode::NOT_FOUND, "not_found")),
    }
}

// ---- custom dashboards ----------------------------------------------------

async fn metrics(State(st): State<AppState>, User(u): User) -> ApiResult {
    Ok(Json(
        json!({ "metrics": dashboards::metrics(&st.db, u.id).await? }),
    ))
}

#[derive(Deserialize)]
struct SeriesQuery {
    metric: String,
    agg: Option<String>,
    bucket: Option<String>,
    end: Option<String>,
    days: Option<i64>,
}

async fn series(
    State(st): State<AppState>,
    User(u): User,
    Query(q): Query<SeriesQuery>,
) -> ApiResult {
    let agg = q
        .agg
        .as_deref()
        .unwrap_or_else(|| dashboards::default_agg(&q.metric));
    let bucket = q.bucket.as_deref().unwrap_or("day");
    if !["sum", "avg", "count"].contains(&agg) || !["day", "week", "month"].contains(&bucket) {
        return Err(bad("invalid_query"));
    }
    if let Some(e) = &q.end {
        if !chrono_ok(e) {
            return Err(bad("invalid_date"));
        }
    }
    let days = q.days.unwrap_or(30).clamp(1, 3650);
    Ok(Json(
        dashboards::series(&st.db, u.id, &q.metric, agg, bucket, q.end, days).await?,
    ))
}

#[derive(Deserialize)]
struct DashboardBody {
    name: Option<String>,
    widgets: Option<Vec<dashboards::Widget>>,
}

fn clean_name(n: &str) -> Result<String, ApiError> {
    let n = n.trim();
    if n.is_empty() || n.chars().count() > 60 {
        return Err(bad("invalid_name"));
    }
    Ok(n.to_string())
}

fn not_found() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "not_found")
}

async fn list_dashboards(State(st): State<AppState>, User(u): User) -> ApiResult {
    Ok(Json(
        json!({ "dashboards": dashboards::list(&st.db, u.id).await? }),
    ))
}

async fn create_dashboard(
    State(st): State<AppState>,
    User(u): User,
    Json(b): Json<DashboardBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let name = clean_name(b.name.as_deref().unwrap_or(""))?;
    let ws = b.widgets.unwrap_or_default();
    dashboards::validate(&ws).map_err(bad)?;
    if dashboards::count(&st.db, u.id).await? >= dashboards::MAX_DASHBOARDS {
        return Err(bad("too_many_dashboards"));
    }
    let id = dashboards::create(&st.db, u.id, &name, &ws).await?;
    dashboards::insert_dashboard_nav(&st.db, u.id, id).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "dashboard": dashboards::get(&st.db, u.id, id).await? })),
    ))
}

async fn get_dashboard(
    State(st): State<AppState>,
    User(u): User,
    Path(id): Path<i64>,
) -> ApiResult {
    let d = dashboards::get(&st.db, u.id, id)
        .await?
        .ok_or_else(not_found)?;
    Ok(Json(json!({ "dashboard": d })))
}

async fn update_dashboard(
    State(st): State<AppState>,
    User(u): User,
    Path(id): Path<i64>,
    Json(b): Json<DashboardBody>,
) -> ApiResult {
    let name = b.name.as_deref().map(clean_name).transpose()?;
    if let Some(ws) = &b.widgets {
        dashboards::validate(ws).map_err(bad)?;
    }
    match dashboards::update(&st.db, u.id, id, name.as_deref(), b.widgets.as_deref()).await? {
        Ok(true) => Ok(Json(
            json!({ "dashboard": dashboards::get(&st.db, u.id, id).await? }),
        )),
        Ok(false) => Err(not_found()),
        Err(code) => Err(bad(code)),
    }
}

async fn delete_dashboard(
    State(st): State<AppState>,
    User(u): User,
    Path(id): Path<i64>,
) -> ApiResult {
    match dashboards::delete(&st.db, u.id, id).await? {
        Ok(true) => Ok(Json(json!({ "ok": true }))),
        Ok(false) => Err(not_found()),
        Err(code) => Err(bad(code)),
    }
}

#[derive(Deserialize)]
struct NavBody {
    items: Vec<dashboards::NavItem>,
}

async fn get_nav(State(st): State<AppState>, User(u): User) -> ApiResult {
    Ok(Json(
        json!({ "items": dashboards::nav(&st.db, u.id).await? }),
    ))
}

async fn put_nav(State(st): State<AppState>, User(u): User, Json(b): Json<NavBody>) -> ApiResult {
    match dashboards::set_nav(&st.db, u.id, &b.items).await? {
        Ok(()) => Ok(Json(
            json!({ "items": dashboards::nav(&st.db, u.id).await? }),
        )),
        Err(code) => Err(bad(code)),
    }
}

/// Every metric the user has, with its Browse category and latest value.
async fn browse_list(State(st): State<AppState>, User(u): User) -> ApiResult {
    let latest = browse::latest(&st.db, u.id).await?;
    let out: Vec<Value> = dashboards::metrics(&st.db, u.id)
        .await?
        .into_iter()
        .map(|m| {
            let l = latest.iter().find(|l| l.0 == m.metric);
            json!({ "metric": m.metric, "name": m.name, "unit": m.unit, "count": m.count,
                    "first": m.first, "last": m.last, "agg": m.agg,
                    "category": browse::category(&m.metric),
                    "latest_value": l.and_then(|l| l.1.clone()), "latest_at": l.and_then(|l| l.2.clone()) })
        })
        .collect();
    Ok(Json(json!({ "metrics": out })))
}

#[derive(Deserialize)]
struct RecordsQuery {
    metric: String,
    before: Option<i64>,
    limit: Option<i64>,
}

async fn records(
    State(st): State<AppState>,
    User(u): User,
    Query(q): Query<RecordsQuery>,
) -> ApiResult {
    let limit = q.limit.unwrap_or(50).clamp(1, browse::MAX_PAGE);
    let recs = browse::records(&st.db, u.id, &q.metric, q.before, limit + 1).await?;
    let more = recs.len() as i64 > limit;
    let recs: Vec<_> = recs.into_iter().take(limit as usize).collect();
    let next = if more {
        recs.last().map(|r| r.id)
    } else {
        None
    };
    Ok(Json(json!({ "records": recs, "next": next })))
}

async fn get_pinned(State(st): State<AppState>, User(u): User) -> ApiResult {
    Ok(Json(
        json!({ "pinned": browse::pinned(&st.db, u.id).await? }),
    ))
}

#[derive(Deserialize)]
struct PinnedBody {
    pinned: Vec<String>,
}

async fn put_pinned(
    State(st): State<AppState>,
    User(u): User,
    Json(b): Json<PinnedBody>,
) -> ApiResult {
    if b.pinned.len() > browse::MAX_PINNED || b.pinned.iter().any(|m| m.is_empty() || m.len() > 200)
    {
        return Err(bad("invalid_pinned"));
    }
    browse::set_pinned(&st.db, u.id, &b.pinned).await?;
    Ok(Json(
        json!({ "pinned": browse::pinned(&st.db, u.id).await? }),
    ))
}

#[derive(Deserialize)]
struct HourlyQuery {
    metric: String,
    date: Option<String>,
}

async fn hourly(
    State(st): State<AppState>,
    User(u): User,
    Query(q): Query<HourlyQuery>,
) -> ApiResult {
    let date = match q.date {
        Some(d) if chrono_ok(&d) => Some(d),
        Some(_) => return Err(bad("invalid_date")),
        None => sqlx::query_scalar::<_, Option<String>>(
            "SELECT substr(MAX(local_start), 1, 10) FROM readings WHERE user_id = ? AND type = ?",
        )
        .bind(u.id)
        .bind(&q.metric)
        .fetch_one(&st.db)
        .await
        .map_err(anyhow::Error::from)?,
    };
    let values = match &date {
        Some(d) => browse::hourly(&st.db, u.id, &q.metric, d).await?,
        None => vec![0.0; 24],
    };
    let unit: Option<String> = sqlx::query_scalar(
        "SELECT unit FROM readings WHERE user_id = ? AND type = ? AND unit IS NOT NULL GROUP BY unit ORDER BY COUNT(*) DESC LIMIT 1",
    ).bind(u.id).bind(&q.metric).fetch_optional(&st.db).await.map_err(anyhow::Error::from)?.flatten();
    Ok(Json(
        json!({ "metric": q.metric, "date": date, "unit": unit, "agg": dashboards::default_agg(&q.metric), "values": values }),
    ))
}

async fn get_profile(State(st): State<AppState>, User(u): User) -> ApiResult {
    Ok(Json(
        json!({ "profile": profile::get(&st.db, u.id).await? }),
    ))
}

async fn put_profile(
    State(st): State<AppState>,
    User(u): User,
    Json(p): Json<profile::Profile>,
) -> ApiResult {
    let p = profile::validate(p).map_err(bad)?;
    profile::put(&st.db, u.id, &p).await?;
    Ok(Json(json!({ "profile": p })))
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

async fn export_csv(State(st): State<AppState>, User(u): User) -> ApiResult<Response> {
    type R = (
        String,
        Option<String>,
        Option<String>,
        String,
        String,
        Option<String>,
    );
    let rows: Vec<R> = sqlx::query_as(
        "SELECT type, value, unit, start_utc, end_utc, source FROM readings WHERE user_id = ? ORDER BY start_utc",
    ).bind(u.id).fetch_all(&st.db).await?;
    let mut out = String::from("type,value,unit,start_utc,end_utc,source\n");
    for r in rows {
        let f = [
            r.0,
            r.1.unwrap_or_default(),
            r.2.unwrap_or_default(),
            r.3,
            r.4,
            r.5.unwrap_or_default(),
        ];
        out.push_str(&f.iter().map(|x| csv_field(x)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    // Workouts, in the same columns: a `Workout:<activity type>` row, then one
    // `Workout:<activity type>:<field>` row per stored value. Only values as
    // imported; nothing is computed here.
    type W = (
        String,
        String,
        String,
        String,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
    );
    let workouts: Vec<W> = sqlx::query_as(
        "SELECT activity_type, start_utc, end_utc, source, duration_min, active_kcal, total_kcal,
                distance_m, elevation_m, avg_hr, max_hr
         FROM workouts WHERE user_id = ? ORDER BY start_utc",
    )
    .bind(u.id)
    .fetch_all(&st.db)
    .await?;
    for w in workouts {
        let kind = format!("Workout:{}", w.0);
        let fields = [
            ("duration", w.4, "min"),
            ("active_energy", w.5, "kcal"),
            ("total_energy", w.6, "kcal"),
            ("distance", w.7, "m"),
            ("elevation", w.8, "m"),
            ("avg_heart_rate", w.9, "count/min"),
            ("max_heart_rate", w.10, "count/min"),
        ];
        let mut row = |ty: &str, value: String, unit: &str| {
            let f = [ty, &value, unit, &w.1, &w.2, &w.3];
            out.push_str(&f.iter().map(|x| csv_field(x)).collect::<Vec<_>>().join(","));
            out.push('\n');
        };
        row(&kind, String::new(), "");
        for (name, v, unit) in fields {
            if let Some(v) = v {
                row(&format!("{kind}:{name}"), v.to_string(), unit);
            }
        }
    }
    // Manual ring goals (history): one ManualGoal row, then one field row per set value.
    type Mg = (i64, String, Option<f64>, Option<f64>, Option<f64>, String);
    let manuals: Vec<Mg> = sqlx::query_as(
        "SELECT id, effective_from, move_kcal, exercise_min, stand_hours, created_at
         FROM manual_goals WHERE user_id = ? ORDER BY effective_from, id",
    )
    .bind(u.id)
    .fetch_all(&st.db)
    .await?;
    for m in manuals {
        let kind = format!("ManualGoal:{}", m.0);
        let stamp = &m.1; // effective_from as start/end local day marker
        let mut row = |ty: &str, value: String, unit: &str| {
            let f = [ty, &value, unit, stamp, stamp, "manual"];
            out.push_str(&f.iter().map(|x| csv_field(x)).collect::<Vec<_>>().join(","));
            out.push('\n');
        };
        row(&kind, m.5.clone(), "");
        if let Some(v) = m.2 {
            row(&format!("{kind}:move"), v.to_string(), "kcal");
        }
        if let Some(v) = m.3 {
            row(&format!("{kind}:exercise"), v.to_string(), "min");
        }
        if let Some(v) = m.4 {
            row(&format!("{kind}:stand"), v.to_string(), "hr");
        }
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"healthvibe-export.csv\"".to_string(),
            ),
        ],
        out,
    )
        .into_response())
}

#[derive(Deserialize)]
struct Confirm {
    confirm: String,
}

/// Delete all of this user's health data (readings, workouts, imported and manual goals,
/// raw uploads awaiting parse, sync cursors, pinned metrics). Keeps the account, profile and devices.
async fn delete_data(
    State(st): State<AppState>,
    User(u): User,
    Json(b): Json<Confirm>,
) -> ApiResult {
    if b.confirm != u.username {
        return Err(bad("confirm_mismatch"));
    }
    let mut tx = st.db.begin().await?;
    for t in [
        "readings",
        "workouts",
        "activity_goals",
        "manual_goals",
        "ingest_raw",
        "sync_cursors",
        "pinned_metrics",
    ] {
        sqlx::query(&format!("DELETE FROM {t} WHERE user_id = ?"))
            .bind(u.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

async fn revoke_others(State(st): State<AppState>, h: HeaderMap, User(u): User) -> ApiResult {
    let cur = cookie_token(&h)
        .map(|t| auth::sha256_hex(&t))
        .unwrap_or_default();
    let n = sqlx::query("DELETE FROM sessions WHERE user_id = ? AND token_hash != ?")
        .bind(u.id)
        .bind(cur)
        .execute(&st.db)
        .await?
        .rows_affected();
    Ok(Json(json!({ "revoked": n })))
}

async fn list_manual_goals(State(st): State<AppState>, User(u): User) -> ApiResult {
    let goals = manual_goals::list(&st.db, u.id).await?;
    Ok(Json(json!({ "goals": goals })))
}

async fn create_manual_goal(
    State(st): State<AppState>,
    User(u): User,
    Json(body): Json<manual_goals::ManualGoalInput>,
) -> ApiResult {
    match manual_goals::validate(&body) {
        Err("invalid_date") => return Err(bad("invalid_date")),
        Err("invalid_goal") | Err("empty_goal") => return Err(bad("invalid_goal")),
        Err(_) => return Err(bad("invalid_goal")),
        Ok(()) => {}
    }
    let g = manual_goals::create(&st.db, u.id, &body).await?;
    Ok(Json(json!({ "goal": g })))
}

async fn update_manual_goal(
    State(st): State<AppState>,
    User(u): User,
    Path(id): Path<i64>,
    Json(body): Json<manual_goals::ManualGoalInput>,
) -> ApiResult {
    match manual_goals::validate(&body) {
        Err("invalid_date") => return Err(bad("invalid_date")),
        Err("invalid_goal") | Err("empty_goal") => return Err(bad("invalid_goal")),
        Err(_) => return Err(bad("invalid_goal")),
        Ok(()) => {}
    }
    match manual_goals::update(&st.db, u.id, id, &body).await? {
        Some(g) => Ok(Json(json!({ "goal": g }))),
        None => Err(ApiError(StatusCode::NOT_FOUND, "not_found")),
    }
}

async fn delete_manual_goal(
    State(st): State<AppState>,
    User(u): User,
    Path(id): Path<i64>,
) -> ApiResult {
    if !manual_goals::delete(&st.db, u.id, id).await? {
        return Err(ApiError(StatusCode::NOT_FOUND, "not_found"));
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct RingsQuery {
    start: String,
    end: String,
}

async fn rings(
    State(st): State<AppState>,
    User(u): User,
    Query(q): Query<RingsQuery>,
) -> ApiResult {
    if !chrono_ok(&q.start) || !chrono_ok(&q.end) {
        return Err(bad("invalid_date"));
    }
    Ok(Json(
        json!({ "days": browse::rings(&st.db, u.id, &q.start, &q.end).await? }),
    ))
}
