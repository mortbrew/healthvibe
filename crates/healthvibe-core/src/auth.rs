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

pub const MIN_PASSWORD: usize = 12;

/// Longest username accepted: the maximum length of an email address.
pub const MAX_USERNAME: usize = 254;

/// Usernames are matched case-insensitively: trim and lowercase before
/// validating, storing or looking one up.
pub fn normalize_username(u: &str) -> String {
    u.trim().to_lowercase()
}

/// Validate a normalized username: 1-254 chars of [a-z0-9@.+_-], so plain
/// names ("toby") and email addresses ("toby.moon+hv@example.com") both work.
/// If there is an '@' there must be exactly one, with text on both sides.
pub fn valid_username(u: &str) -> bool {
    let ok_chars = u
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "@.+_-".contains(c));
    let at_ok = match u.split_once('@') {
        None => true,
        Some((local, domain)) => !local.is_empty() && !domain.is_empty() && !domain.contains('@'),
    };
    (1..=MAX_USERNAME).contains(&u.len()) && ok_chars && at_ok
}

/// First run: the first account (the seeded row, id 1) becomes the admin with
/// the chosen username and password. Only works while no password exists.
pub async fn setup_admin(db: &SqlitePool, username: &str, pw: &str) -> anyhow::Result<bool> {
    let h = hash_password(pw)?;
    let r = sqlx::query(
        "UPDATE users SET username = ?, password_hash = ?, is_admin = 1, must_change_password = 0
         WHERE id = 1 AND NOT EXISTS (SELECT 1 FROM users WHERE password_hash IS NOT NULL)",
    )
    .bind(username)
    .bind(h)
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, PartialEq)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub must_change_password: bool,
    pub theme_mode: String,
    pub theme_light: String,
    pub theme_dark: String,
    pub onboarding_done: bool,
    pub awards_enabled: bool,
}

pub async fn get_user(db: &SqlitePool, id: i64) -> anyhow::Result<Option<User>> {
    Ok(sqlx::query_as(
        "SELECT id, username, is_admin, must_change_password, theme_mode, theme_light, theme_dark, onboarding_done, awards_enabled
         FROM users WHERE id = ? AND password_hash IS NOT NULL",
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

pub async fn list_users(db: &SqlitePool) -> anyhow::Result<Vec<User>> {
    Ok(sqlx::query_as(
        "SELECT id, username, is_admin, must_change_password, theme_mode, theme_light, theme_dark, onboarding_done, awards_enabled
         FROM users WHERE password_hash IS NOT NULL ORDER BY id",
    )
    .fetch_all(db)
    .await?)
}

/// Admin creates a user with a temporary password; they must change it on
/// first login. Returns None if the username is taken.
pub async fn create_user(
    db: &SqlitePool,
    username: &str,
    temp_pw: &str,
    is_admin: bool,
) -> anyhow::Result<Option<i64>> {
    let h = hash_password(temp_pw)?;
    let r = sqlx::query(
        "INSERT OR IGNORE INTO users (username, password_hash, is_admin, must_change_password)
         VALUES (?, ?, ?, 1)",
    )
    .bind(username)
    .bind(h)
    .bind(is_admin)
    .execute(db)
    .await?;
    Ok((r.rows_affected() == 1).then(|| r.last_insert_rowid()))
}

/// Change a password after checking the current one. Clears the
/// must-change flag. The new password must differ from the current one.
pub async fn change_password(
    db: &SqlitePool,
    user_id: i64,
    current: &str,
    new: &str,
) -> anyhow::Result<bool> {
    let h: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(db)
        .await?
        .flatten();
    match h {
        Some(h) if verify_password(current, &h) && current != new => {}
        _ => return Ok(false),
    }
    sqlx::query("UPDATE users SET password_hash = ?, must_change_password = 0 WHERE id = ?")
        .bind(hash_password(new)?)
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(true)
}

/// Returns the user id when username/password match.
pub async fn login(db: &SqlitePool, user: &str, pw: &str) -> anyhow::Result<Option<i64>> {
    let row: Option<(i64, Option<String>)> =
        sqlx::query_as("SELECT id, password_hash FROM users WHERE lower(username) = ?")
            .bind(normalize_username(user))
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

    #[test]
    fn usernames_accept_emails() {
        for ok in [
            "toby",
            "t.moon_1-x",
            "toby@example.com",
            "toby.moon+hv@mail.example.co.uk",
        ] {
            assert!(valid_username(ok), "{ok}");
        }
        let long = format!("{}@example.com", "a".repeat(MAX_USERNAME - 12));
        assert_eq!(long.len(), MAX_USERNAME);
        assert!(valid_username(&long));
        for bad in [
            "",
            "Toby",
            "to by",
            "a@b@c",
            "@example.com",
            "toby@",
            "tö",
            "x;y",
            &format!("{long}x"),
        ] {
            assert!(!valid_username(bad), "{bad}");
        }
        assert_eq!(
            normalize_username("  Toby.Moon@Example.COM \n"),
            "toby.moon@example.com"
        );
    }

    #[tokio::test]
    async fn setup_login_session() {
        let db = crate::db::open_memory().await.unwrap();
        assert!(needs_setup(&db).await.unwrap());
        assert!(setup_admin(&db, "admin", "correct horse").await.unwrap());
        assert!(
            !setup_admin(&db, "x", "again again again").await.unwrap(),
            "setup only once"
        );
        assert_eq!(login(&db, "admin", "wrong").await.unwrap(), None);
        let uid = login(&db, "admin", "correct horse").await.unwrap().unwrap();
        let tok = create_session(&db, uid).await.unwrap();
        assert_eq!(session_user(&db, &tok).await.unwrap(), Some(1));
        delete_session(&db, &tok).await.unwrap();
        assert_eq!(session_user(&db, &tok).await.unwrap(), None);
    }

    #[tokio::test]
    async fn temp_password_must_change() {
        let db = crate::db::open_memory().await.unwrap();
        setup_admin(&db, "toby", "admin password!").await.unwrap();
        let id = create_user(&db, "sam", "temporary-pass", false)
            .await
            .unwrap()
            .unwrap();
        assert!(create_user(&db, "sam", "x", false).await.unwrap().is_none());
        assert!(
            get_user(&db, id)
                .await
                .unwrap()
                .unwrap()
                .must_change_password
        );
        assert!(!change_password(&db, id, "wrong", "new password 1")
            .await
            .unwrap());
        assert!(
            !change_password(&db, id, "temporary-pass", "temporary-pass")
                .await
                .unwrap()
        );
        assert!(change_password(&db, id, "temporary-pass", "new password 1")
            .await
            .unwrap());
        assert!(
            !get_user(&db, id)
                .await
                .unwrap()
                .unwrap()
                .must_change_password
        );
    }
}
