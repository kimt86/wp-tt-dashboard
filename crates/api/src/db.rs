//! Read-only PostgreSQL pool. This crate has NO Oracle access by construction.

use anyhow::{Context, Result};
use sqlx::postgres::{PgPool, PgPoolOptions};

pub async fn pool() -> Result<PgPool> {
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL not set")?;
    PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .context("connecting to PostgreSQL")
}

/// Latest business date present in kpi_daily (used when ?as_of is omitted).
pub async fn latest_as_of(pool: &PgPool) -> Result<Option<chrono::NaiveDate>> {
    let row: Option<(chrono::NaiveDate,)> =
        sqlx::query_as("SELECT max(snapshot_date) FROM kpi_daily")
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}
