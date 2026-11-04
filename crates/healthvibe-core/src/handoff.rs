//! Phone handoff for Health Auto Export setup.
//!
//! The desktop asks for a handoff and shows `https://<public>/h/<code>` as a
//! QR code. Opening it on the phone claims the code (single use, 10 minutes)
//! and only *then* mints a fresh upload token, which is returned once in that
//! response. So:
//! - the database only ever holds `sha256(code)` and `sha256(token secret)`;
//! - no token exists anywhere while the link is waiting to be opened;
//! - a used or expired link reveals nothing.
//!
//! The desktop polls with a separate random `id` (scoped to its own user) to
//! show "Opened on your phone".

use crate::auth::sha256_hex;
use crate::shortcut;
use chrono::{Duration, SecondsFormat, Utc};
use rand::RngCore;
use serde::Serialize;
use sqlx::SqlitePool;

pub const MINUTES: i64 = 10;

fn random_hex(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[derive(Debug, Serialize)]
pub struct NewHandoff {
    pub id: String,
    /// Shown once (in the QR code); only its hash is stored.
    #[serde(skip)]
    pub code: String,
    pub expires_at: String,
}

/// New handoff for `user_id`. Older unopened handoffs of this user stop working.
pub async fn create(db: &SqlitePool, user_id: i64) -> anyhow::Result<NewHandoff> {
    let id = random_hex(12);
    let code = random_hex(16);
    let exp = (Utc::now() + Duration::minutes(MINUTES)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let mut tx = db.begin().await?;
    sqlx::query(
        "DELETE FROM handoffs WHERE (user_id = ? AND claimed_at IS NULL) OR expires_at < ?",
    )
    .bind(user_id)
    .bind((Utc::now() - Duration::days(1)).to_rfc3339_opts(SecondsFormat::Secs, true))
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO handoffs (id, code_hash, user_id, created_at, expires_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(sha256_hex(&code))
    .bind(user_id)
    .bind(now())
    .bind(&exp)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(NewHandoff {
        id,
        code,
        expires_at: exp,
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum Claim {
    /// (user id, fresh upload token)
    Ok(i64, String),
    /// Unknown, already used or expired. Deliberately not distinguished.
    Invalid,
}

/// Open a handoff link: single use. Mints a new Health Auto Export token
/// (replacing the user's previous one) and returns it once.
pub async fn claim(db: &SqlitePool, code: &str) -> anyhow::Result<Claim> {
    if code.len() != 32 || !code.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(Claim::Invalid);
    }
    let t = now();
    let user: Option<i64> = sqlx::query_scalar(
        "UPDATE handoffs SET claimed_at = ?
         WHERE code_hash = ? AND claimed_at IS NULL AND expires_at > ?
         RETURNING user_id",
    )
    .bind(&t)
    .bind(sha256_hex(&code.to_ascii_lowercase()))
    .bind(&t)
    .fetch_optional(db)
    .await?;
    let Some(uid) = user else {
        return Ok(Claim::Invalid);
    };
    let token = shortcut::new_token(db, uid, crate::hae::DEVICE).await?;
    Ok(Claim::Ok(uid, token))
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Status {
    pub claimed: bool,
    pub expired: bool,
    pub claimed_at: Option<String>,
    pub expires_at: String,
}

/// Status of one of `user_id`'s handoffs (None if it isn't theirs).
pub async fn status(db: &SqlitePool, user_id: i64, id: &str) -> anyhow::Result<Option<Status>> {
    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT expires_at, claimed_at FROM handoffs WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user_id)
            .fetch_optional(db)
            .await?;
    Ok(row.map(|(expires_at, claimed_at)| Status {
        claimed: claimed_at.is_some(),
        expired: claimed_at.is_none() && expires_at <= now(),
        claimed_at,
        expires_at,
    }))
}
