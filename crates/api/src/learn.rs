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

// ───────────────────────── lanes (③) ─────────────────────────

#[derive(Serialize, sqlx::FromRow)]
struct LaneCellOut {
    lat: f64,
    lon: f64,
    passes: i64,
    heading_deg: Option<f64>,
    directionality: Option<f64>,
    mean_speed: Option<f64>,
}

#[derive(Serialize, sqlx::FromRow)]
struct LaneMetricPoint {
    captured_at: DateTime<Utc>,
    cells: i32,
    road_cells: i32,
    total_passes: i64,
    oneway_frac: Option<f64>,
}

#[derive(Serialize)]
pub struct LanesResp {
    cells: i64,
    road_cells: i64, // passes ≥ 20
    total_passes: i64,
    oneway_frac: Option<f64>, // road cells with directionality ≥ 0.8
    grid: Vec<LaneCellOut>,
    metric_series: Vec<LaneMetricPoint>,
}

/// GET /api/learn/lanes — the learned driving-lane grid + accumulation/quality series.
pub async fn lanes(State(pool): State<PgPool>) -> Result<Json<LanesResp>, AppError> {
    let grid: Vec<LaneCellOut> = sqlx::query_as(
        "SELECT lat, lon, passes, heading_deg, directionality, mean_speed
           FROM learn_lane_cell WHERE passes >= 5 ORDER BY passes DESC LIMIT 4000",
    )
    .fetch_all(&pool)
    .await?;

    let metric_series: Vec<LaneMetricPoint> = sqlx::query_as(
        "SELECT captured_at, cells, road_cells, total_passes, oneway_frac
           FROM learn_lane_metric
          WHERE captured_at > now() - interval '30 days'
          ORDER BY captured_at",
    )
    .fetch_all(&pool)
    .await?;

    let (cells, road_cells, total_passes, oneway_frac): (i64, i64, i64, Option<f64>) =
        sqlx::query_as(
            "SELECT count(*),
                    count(*) FILTER (WHERE passes >= 20),
                    coalesce(sum(passes), 0)::bigint,
                    (count(*) FILTER (WHERE passes >= 20 AND directionality >= 0.8))::float8
                      / nullif(count(*) FILTER (WHERE passes >= 20), 0)
               FROM learn_lane_cell",
        )
        .fetch_one(&pool)
        .await?;

    Ok(Json(LanesResp { cells, road_cells, total_passes, oneway_frac, grid, metric_series }))
}
