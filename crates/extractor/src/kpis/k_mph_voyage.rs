//! K_MPH official voyage-level extract: phase_c/06 -> raw_k_mph_voyage (30-day window).

use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::Deserialize;
use sqlx::PgPool;
use wp_core::parse::parse_rows;

use crate::kpis::common::run_logged;
use crate::{params, runner::Toolbox};

pub const KPI_KEY: &str = "K_MPH_VOYAGE";
const SQL: &str = include_str!("../../sql/c06_k_mph_voyage.sql");
const WINDOW_DAYS: i64 = 30;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct Row {
    pub vessel: String,
    pub voyage: String,
    pub confirmed_at: Option<String>,
    pub confirmed: Option<String>,
    pub stt_check: Option<String>,
    pub containers: Option<f64>,
    pub teu: Option<f64>,
    pub moves: Option<f64>,
    pub single_moves: Option<f64>,
    pub twin_moves: Option<f64>,
    pub tandem_moves: Option<f64>,
    pub gross_min: Option<f64>,
    pub net_min: Option<f64>,
    pub berth_min: Option<f64>,
    pub work_qc: Option<f64>,
    pub k_mph_gross: Option<f64>,
    pub k_mph_net: Option<f64>,
    pub k_bp_gross: Option<f64>,
    pub k_bp_net: Option<f64>,
}

pub fn parse(raw: &str) -> Result<Vec<Row>> {
    parse_rows(raw).context("parsing k_mph_voyage rows")
}

pub async fn upsert(pool: &PgPool, date: NaiveDate, run_id: i64, rows: &[Row]) -> Result<u64> {
    let mut tx = pool.begin().await?;
    for r in rows {
        sqlx::query(
            "INSERT INTO raw_k_mph_voyage
               (vessel, voyage, confirmed_at, confirmed, stt_check, containers, teu, moves,
                single_moves, twin_moves, tandem_moves, gross_min, net_min, berth_min, work_qc,
                k_mph_gross, k_mph_net, k_bp_gross, k_bp_net, snapshot_date, run_id)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21)
             ON CONFLICT (vessel, voyage) DO UPDATE SET
               confirmed_at=EXCLUDED.confirmed_at, confirmed=EXCLUDED.confirmed,
               stt_check=EXCLUDED.stt_check, containers=EXCLUDED.containers, teu=EXCLUDED.teu,
               moves=EXCLUDED.moves, single_moves=EXCLUDED.single_moves, twin_moves=EXCLUDED.twin_moves,
               tandem_moves=EXCLUDED.tandem_moves, gross_min=EXCLUDED.gross_min, net_min=EXCLUDED.net_min,
               berth_min=EXCLUDED.berth_min, work_qc=EXCLUDED.work_qc, k_mph_gross=EXCLUDED.k_mph_gross,
               k_mph_net=EXCLUDED.k_mph_net, k_bp_gross=EXCLUDED.k_bp_gross, k_bp_net=EXCLUDED.k_bp_net,
               snapshot_date=EXCLUDED.snapshot_date, run_id=EXCLUDED.run_id, extracted_at=now()",
        )
        .bind(&r.vessel)
        .bind(&r.voyage)
        .bind(&r.confirmed_at)
        .bind(&r.confirmed)
        .bind(&r.stt_check)
        .bind(r.containers)
        .bind(r.teu)
        .bind(r.moves)
        .bind(r.single_moves)
        .bind(r.twin_moves)
        .bind(r.tandem_moves)
        .bind(r.gross_min)
        .bind(r.net_min)
        .bind(r.berth_min)
        .bind(r.work_qc)
        .bind(r.k_mph_gross)
        .bind(r.k_mph_net)
        .bind(r.k_bp_gross)
        .bind(r.k_bp_net)
        .bind(date)
        .bind(run_id)
        .execute(&mut *tx)
        .await
        .context("upserting raw_k_mph_voyage")?;
    }
    tx.commit().await?;
    Ok(rows.len() as u64)
}

pub async fn extract(pool: &PgPool, date: NaiveDate, target: &str) -> Result<u64> {
    let sql = params::render_window(SQL, date, WINDOW_DAYS)?;
    run_logged(pool, KPI_KEY, date, |run_id| async move {
        let raw = Toolbox::from_env(target)?.run_sql(&sql).await?;
        upsert(pool, date, run_id, &parse(&raw)?).await
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rows() {
        let raw = r#"{"result":"[{\"VESSEL\":\"SLSL\",\"VOYAGE\":\"079/2017\",\"CONFIRMED_AT\":\"20260601120000\",\"CONFIRMED\":\"Y\",\"STT_CHECK\":\"Y\",\"CONTAINERS\":786,\"TEU\":1320.0,\"MOVES\":1061,\"SINGLE_MOVES\":786,\"TWIN_MOVES\":275,\"TANDEM_MOVES\":0,\"GROSS_MIN\":2735.0,\"NET_MIN\":2200.0,\"BERTH_MIN\":3000.0,\"WORK_QC\":3,\"K_MPH_GROSS\":29.31,\"K_MPH_NET\":35.0,\"K_BP_GROSS\":80.0,\"K_BP_NET\":90.0}]"}"#;
        let rows = parse(raw).unwrap();
        assert_eq!(rows[0].vessel, "SLSL");
        assert_eq!(rows[0].confirmed.as_deref(), Some("Y"));
        assert!((rows[0].k_mph_gross.unwrap() - 29.31).abs() < 1e-9);
    }
}
