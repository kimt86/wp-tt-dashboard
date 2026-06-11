//! wp-tt-dashboard read-only API (axum). Reads ONLY PostgreSQL (L1/L2) and, for the
//! live map, subscribes to the WP-TT GPS websocket via the local SSH tunnel. This crate
//! has NO Oracle/SSH access — it cannot reach production Oracle.

mod agg;
mod cycles;
mod db;
mod live;
mod livemap;
mod models;
mod periods;
mod routes;
mod workpool;

use std::sync::Arc;

use axum::extract::FromRef;
use axum::{routing::get, Router};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

/// Combined app state. Existing handlers take `State<PgPool>`; the `FromRef` impls let
/// both that and `State<Arc<LiveMap>>` be extracted from this one state.
#[derive(Clone)]
struct AppState {
    pool: sqlx::PgPool,
    livemap: Arc<livemap::LiveMap>,
}

impl FromRef<AppState> for sqlx::PgPool {
    fn from_ref(s: &AppState) -> sqlx::PgPool {
        s.pool.clone()
    }
}
impl FromRef<AppState> for Arc<livemap::LiveMap> {
    fn from_ref(s: &AppState) -> Arc<livemap::LiveMap> {
        s.livemap.clone()
    }
}

fn app(state: AppState) -> Router {
    let api = Router::new()
        .route("/api/kpis", get(routes::kpis))
        .route("/api/kpis/history", get(routes::kpi_history))
        .route("/api/kpis/:key/trend", get(routes::trend))
        .route("/api/breakdown/qc", get(routes::breakdown_qc))
        .route("/api/stats/:key", get(routes::stats))
        .route("/api/live", get(live::live))
        .route("/api/live/vessels", get(live::vessels))
        .route("/api/livemap/positions", get(livemap::positions))
        .route("/api/livemap/health", get(livemap::health))
        .route("/api/workpool", get(workpool::workpool))
        .route("/api/tt-cycles/summary", get(cycles::summary))
        .route("/api/tt-cycles/detail", get(cycles::detail))
        .route("/api/health", get(routes::health))
        .layer(CorsLayer::permissive()) // dev; tighten to the dashboard origin in prod
        .with_state(state);

    // Knowledge center — self-contained static HTML docs at /kc/ (NOT linked from the
    // dashboard UI; shared internally by direct link, reachable over Tailscale).
    // no-cache = always revalidate (cheap 304s): without it browsers heuristically cache
    // the doc HTML for hours, so doc updates (e.g. the shared sidebar shell) don't show.
    let kc_dir = std::env::var("KC_DIR").unwrap_or_else(|_| "kc".to_string());
    let kc = tower::ServiceBuilder::new()
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-cache"),
        ))
        .service(ServeDir::new(&kc_dir));
    let api = api.nest_service("/kc", kc);

    // Serve the built SPA (if present) and fall back to index.html for client routing.
    let web_dist = std::env::var("WEB_DIST").unwrap_or_else(|_| "web/dist".to_string());
    let index = format!("{web_dist}/index.html");
    let spa = ServeDir::new(&web_dist).not_found_service(ServeFile::new(index));

    api.fallback_service(spa)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let pool = db::pool().await?;
    let livemap = livemap::LiveMap::new();
    livemap::spawn(livemap.clone()); // background GPS ingest (via local SSH tunnel)
    livemap::spawn_util_sampler(livemap.clone(), pool.clone()); // 60s TT-utilization samples
    livemap::spawn_assignment_refresh(livemap.clone(), pool.clone()); // 30s work-pool assignment cache
    livemap::spawn_cycle_flusher(livemap.clone(), pool.clone()); // 30s persist completed TT cycles
    let state = AppState { pool, livemap };

    let addr = std::env::var("API_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "wp-api listening");
    axum::serve(listener, app(state)).await?;
    Ok(())
}
