//! SQLite storage: open the database, apply pragmas, run migrations.

use crate::config::Config;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::time::Duration;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Open (creating if needed) the database and run pending migrations.
pub async fn open(config: &Config) -> anyhow::Result<SqlitePool> {
    std::fs::create_dir_all(&config.db_dir)?;
    std::fs::create_dir_all(&config.staging_dir)?;
    let url = format!("sqlite://{}", config.db_path().display());
    open_url(&url).await
}

pub async fn open_url(url: &str) -> anyhow::Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

/// In-memory database for tests.
pub async fn open_memory() -> anyhow::Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn migrations_seed_admin() {
        let pool = super::open_memory().await.unwrap();
        let (name, admin): (String, i64) =
            sqlx::query_as("SELECT username, is_admin FROM users WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!((name.as_str(), admin), ("admin", 1));
    }
}
