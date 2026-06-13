//! Learning center API. v1 model = block work-point coordinates (target ②): per topos code,
//! the learned (lat,lon) accumulated from TTs observed ARRIVED there (livemap centroids,
//! persisted by `spawn_learn_persist`). Exposes the model points, a summary, and a quality
//! time series so the dashboard can show accumulation + precision improving over time.
//! Pure Postgres reads.

use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;

use crate::routes::AppError;

#[derive(Serialize, sqlx::FromRow)]
struct ToposPoint {
    topos: String,
    is_crane: bool,
    lat: f64,
    lon: f64,
    n: i32,
    obs: i64,
    spread_m: Option<f64>,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
struct MetricPoint {
    captured_at: DateTime<Utc>,
    distinct_topos: i32,
    confident_topos: i32,
    total_obs: i64,
    median_spread_m: Option<f64>,
}

#[derive(Serialize)]
pub struct ToposResp {
    distinct_topos: i64,
    confident_topos: i64, // n ≥ 30
    block_points: i64,    // non-crane work-points (the focus)
    total_obs: i64,
    median_spread_m: Option<f64>,
    points: Vec<ToposPoint>,
    metric_series: Vec<MetricPoint>,
}

/// GET /api/learn/topos — the block work-point coordinate model + accumulation/quality series.
pub async fn topos(State(pool): State<PgPool>) -> Result<Json<ToposResp>, AppError> {
    let points: Vec<ToposPoint> = sqlx::query_as(
        "SELECT topos, is_crane, lat, lon, n, obs, spread_m, updated_at
           FROM learn_topos_point ORDER BY obs DESC LIMIT 1000",
    )
    .fetch_all(&pool)
    .await?;

    let metric_series: Vec<MetricPoint> = sqlx::query_as(
        "SELECT captured_at, distinct_topos, confident_topos, total_obs, median_spread_m
           FROM learn_topos_metric
          WHERE captured_at > now() - interval '30 days'
          ORDER BY captured_at",
    )
    .fetch_all(&pool)
    .await?;

    let (distinct_topos, confident_topos, block_points, total_obs, median_spread_m): (
        i64,
        i64,
        i64,
        i64,
        Option<f64>,
    ) = sqlx::query_as(
        "SELECT count(*),
                count(*) FILTER (WHERE n >= 30),
                count(*) FILTER (WHERE NOT is_crane),
                coalesce(sum(obs), 0)::bigint,
                percentile_cont(0.5) WITHIN GROUP (ORDER BY spread_m) FILTER (WHERE n >= 30)
           FROM learn_topos_point",
    )
    .fetch_one(&pool)
    .await?;

    Ok(Json(ToposResp {
        distinct_topos,
        confident_topos,
        block_points,
        total_obs,
        median_spread_m,
        points,
        metric_series,
    }))
}
