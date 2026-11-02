pub mod models;

use anyhow::Result;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use std::time::Duration;
use tauri::{AppHandle, Manager};

static POOL: tokio::sync::OnceCell<SqlitePool> = tokio::sync::OnceCell::const_new();

pub async fn init(app: &AppHandle) -> Result<()> {
    let dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&dir)?;
    let db_path = dir.join("ioun.db");
    let url = format!("sqlite://{}", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(
            SqliteConnectOptions::from_str(&url)?
                .create_if_missing(true)
                .foreign_keys(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
                .busy_timeout(Duration::from_secs(5)),
        )
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    // `set` falla si ya había un pool: el primero manda y este se descarta. Es el
    // comportamiento que se quiere (la inicialización es idempotente), así que el
    // error se ignora a propósito.
    let _ = POOL.set(pool);
    Ok(())
}

pub async fn pool() -> Result<&'static SqlitePool> {
    POOL.get()
        .ok_or_else(|| anyhow::anyhow!("db no inicializada"))
}

#[cfg(test)]
pub async fn test_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::from_str("sqlite::memory:")
                .expect("valid in-memory url")
                .foreign_keys(true),
        )
        .await
        .expect("in-memory pool");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations apply");
    pool
}
