//! Shared run-logging wrapper for KPI extracts.

use anyhow::Result;
use chrono::NaiveDate;
use sqlx::PgPool;

use crate::db;

/// Run `work` under an etl_run_log entry: insert RUNNING, then mark OK/FAILED and
/// update freshness based on the outcome. `work` receives the run_id and returns
/// the number of rows written.
pub async fn run_logged<F, Fut>(
    pool: &PgPool,
    kpi_key: &str,
    date: NaiveDate,
    work: F,
) -> Result<u64>
where
    F: FnOnce(i64) -> Fut,
    Fut: std::future::Future<Output = Result<u64>>,
{
    let run_id = db::start_run(pool, kpi_key, date).await?;
    match work(run_id).await {
        Ok(n) => {
            db::finish_run(pool, run_id, kpi_key, date, "OK", Some(n as i64), None).await?;
            tracing::info!(kpi = kpi_key, %date, rows = n, "extract OK");
            Ok(n)
        }
        Err(e) => {
            db::finish_run(pool, run_id, kpi_key, date, "FAILED", None, Some(&e.to_string()))
                .await?;
            Err(e)
        }
    }
}
