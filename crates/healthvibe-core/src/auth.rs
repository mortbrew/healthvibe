//! Password hashing (argon2id) and session tokens.

use argon2::password_hash::{rand_core::OsRng, PasswordHash, SaltString};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

pub const SESSION_DAYS: i64 = 30;

pub fn hash_password(pw: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("hash: {e}"))
}

pub fn verify_password(pw: &str, phc: &str) -> bool {
    PasswordHash::new(phc)
        .map(|h| Argon2::default().verify_password(pw.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

pub fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

/// True until the seeded admin has a password.
pub async fn needs_setup(db: &SqlitePool) -> anyhow::Result<bool> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE password_hash IS NOT NULL")
        .fetch_one(db)
        .await?;
    Ok(n == 0)
}

/// Set the admin password, only if no password exists yet.
pub async fn setup_admin(db: &SqlitePool, pw: &str) -> anyhow::Result<bool> {
    let h = hash_password(pw)?;
    let r = sqlx::query(
        "UPDATE users SET password_hash = ? WHERE id = 1 AND NOT EXISTS
         (SELECT 1 FROM users WHERE password_hash IS NOT NULL)",
    )
    .bind(h)
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Returns the user id when username/password match.
pub async fn login(db: &SqlitePool, user: &str, pw: &str) -> anyhow::Result<Option<i64>> {
    let row: Option<(i64, Option<String>)> =
        sqlx::query_as("SELECT id, password_hash FROM users WHERE username = ?")
            .bind(user)
            .fetch_optional(db)
            .await?;
    Ok(match row {
        Some((id, Some(h))) if verify_password(pw, &h) => Some(id),
        _ => None,
    })
}

/// Create a session; returns the raw cookie token.
pub async fn create_session(db: &SqlitePool, user_id: i64) -> anyhow::Result<String> {
    let mut b = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut b);
    let token = hex::encode(b);
    let exp = (chrono::Utc::now() + chrono::Duration::days(SESSION_DAYS)).to_rfc3339();
    sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at) VALUES (?, ?, ?)")
        .bind(sha256_hex(&token))
        .bind(user_id)
        .bind(exp)
        .execute(db)
        .await?;
    Ok(token)
}

pub async fn session_user(db: &SqlitePool, token: &str) -> anyhow::Result<Option<i64>> {
    let now = chrono::Utc::now().to_rfc3339();
    Ok(
        sqlx::query_scalar("SELECT user_id FROM sessions WHERE token_hash = ? AND expires_at > ?")
            .bind(sha256_hex(token))
            .bind(now)
            .fetch_optional(db)
            .await?,
    )
}

pub async fn delete_session(db: &SqlitePool, token: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(sha256_hex(token))
        .execute(db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn setup_login_session() {
        let db = crate::db::open_memory().await.unwrap();
        assert!(needs_setup(&db).await.unwrap());
        assert!(setup_admin(&db, "correct horse").await.unwrap());
        assert!(!setup_admin(&db, "again").await.unwrap(), "setup only once");
        assert_eq!(login(&db, "admin", "wrong").await.unwrap(), None);
        let uid = login(&db, "admin", "correct horse").await.unwrap().unwrap();
        let tok = create_session(&db, uid).await.unwrap();
        assert_eq!(session_user(&db, &tok).await.unwrap(), Some(1));
        delete_session(&db, &tok).await.unwrap();
        assert_eq!(session_user(&db, &tok).await.unwrap(), None);
    }
}
