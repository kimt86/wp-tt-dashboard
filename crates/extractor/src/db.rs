//! PostgreSQL access for the extractor: connection pool and etl_run_log /
//! data_freshness bookkeeping. (Per-KPI upserts live in the `kpis` modules.)

use anyhow::{Context, Result};
use chrono::NaiveDate;
use sqlx::postgres::{PgPool, PgPoolOptions};

pub async fn pool() -> Result<PgPool> {
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL not set")?;
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .context("connecting to PostgreSQL")
}

/// Insert a RUNNING row in etl_run_log and return its run_id.
pub async fn start_run(pool: &PgPool, kpi_key: &str, business_date: NaiveDate) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO etl_run_log (kpi_key, business_date, status)
         VALUES ($1, $2, 'RUNNING') RETURNING run_id",
    )
    .bind(kpi_key)
    .bind(business_date)
    .fetch_one(pool)
    .await
    .context("inserting etl_run_log RUNNING row")?;
    Ok(row.0)
}

/// Mark a run finished and update the KPI's freshness in one transaction.
pub async fn finish_run(
    pool: &PgPool,
    run_id: i64,
    kpi_key: &str,
    business_date: NaiveDate,
    status: &str,
    rows_written: Option<i64>,
    error_text: Option<&str>,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE etl_run_log
            SET status = $2, rows_written = $3, error_text = $4, finished_at = now()
          WHERE run_id = $1",
    )
    .bind(run_id)
    .bind(status)
    .bind(rows_written.map(|n| n as i32))
    .bind(error_text)
    .execute(&mut *tx)
    .await?;

    let success = status == "OK" || status == "PARTIAL";
    sqlx::query(
        "INSERT INTO data_freshness
           (kpi_key, last_success_date, last_success_at, last_attempt_at, last_status, is_stale)
         VALUES ($1,
                 CASE WHEN $3 THEN $2 ELSE NULL END,
                 CASE WHEN $3 THEN now() ELSE NULL END,
                 now(), $4, false)
         ON CONFLICT (kpi_key) DO UPDATE SET
           last_attempt_at  = now(),
           last_status      = EXCLUDED.last_status,
           last_success_date= CASE WHEN $3 THEN $2 ELSE data_freshness.last_success_date END,
           last_success_at  = CASE WHEN $3 THEN now() ELSE data_freshness.last_success_at END,
           is_stale         = false",
    )
    .bind(kpi_key)
    .bind(business_date)
    .bind(success)
    .bind(status)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}
