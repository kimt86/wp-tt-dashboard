//! Live work-pool snapshot extract. Two bounded Oracle scans per ~90s tick:
//!   1. JOB_QUEUE_SCHEDULE → live_workqueue (per-QC queue plan + progress).
//!   2. JOB_ORDER_LIST (A + Q) → split in Rust into:
//!        - live_workpool  (A = dispatched in-flight moves, the QC task cards)
//!        - live_candidate (Q = UNASSIGNED demand, aggregated: discharge by QC,
//!                          load by source block — the dispatch candidate pool)
//! This is the ONLY path that brings the work pool into Postgres; the API crate can't
//! reach Oracle. "Live now" (no date window) — bounded by status + recent CRE_DT to
//! keep the scan small and Oracle-friendly.

use std::collections::HashMap;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use wp_core::parse::parse_rows;

use crate::kpis::common::run_logged;
use crate::runner::Toolbox;

const SQL_WORKQUEUE: &str = include_str!("../sql/workqueue.sql");
const SQL_WORKPOOL: &str = include_str!("../sql/workpool.sql");
const SQL_ASSIGNED: &str = include_str!("../sql/assigned_tt.sql");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct QueueRow {
    pub qc: String,
    pub vessel: String,
    pub voyage: Option<String>,
    pub queuename: String,
    pub disload: Option<String>,
    pub seq: Option<i64>,
    pub total_qty: Option<i64>,
    pub comp_qty: Option<i64>,
    pub plan_qty: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct MoveRow {
    pub queuename: String,
    pub vessel: String,
    pub voyage: Option<String>,
    pub jobtype: Option<String>,
    pub jobstatus: Option<String>,
    pub yt_status: Option<String>,
    pub ytno: Option<String>,
    pub armgc: Option<String>,
    pub etw_dt: Option<String>,
    pub contno: Option<String>,
    pub msnseq: Option<String>,
    pub yt_topos: Option<String>,
    pub from_pos: Option<String>,
    pub to_pos: Option<String>,
    pub twintandem: Option<String>,
}

/// Parse an ETW field ("YYYYMMDDHH24MISS[mmm]", terminal MYT) to a UTC instant.
/// Returns None for empty/short/malformed values.
pub fn parse_etw(raw: &str) -> Option<DateTime<Utc>> {
    let s = raw.trim();
    if s.len() < 14 || !s.as_bytes()[..14].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let naive = NaiveDateTime::parse_from_str(&s[..14], "%Y%m%d%H%M%S").ok()?;
    Some(wp_core::shift::terminal_to_utc(naive))
}

/// Run one work-pool tick: refresh both snapshot tables. Each source is logged and a
/// failure in one does not abort the other.
pub async fn tick_workpool(pool: &PgPool, target: &str) -> Result<()> {
    let date = wp_core::shift::terminal_now().date_naive();
    let as_of = Utc::now();

    macro_rules! step {
        ($name:expr, $fut:expr) => {
            if let Err(e) = $fut.await {
                tracing::error!(source = $name, error = %e, "workpool source failed (continuing)");
            }
        };
    }
    step!("workqueue", src_workqueue(pool, target, date, as_of));
    step!("workpool", src_workpool(pool, target, date, as_of));
    step!("assigned", src_assigned(pool, target, date, as_of));
    tracing::info!(%as_of, "workpool tick done");
    Ok(())
}

/// All TTs with an active assignment of ANY job type (for utilization). Refills
/// live_assigned_tt each tick. Separate from live_workpool (DS/LD only, for dispatch).
async fn src_assigned(pool: &PgPool, target: &str, date: chrono::NaiveDate, as_of: DateTime<Utc>) -> Result<()> {
    run_logged(pool, "ASSIGNED_TT", date, |_| async move {
        let raw = Toolbox::from_env(target)?.run_sql(SQL_ASSIGNED).await?;
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "UPPERCASE")]
        struct YtRow { ytno: String, jobstatus: Option<String> }
        let rows: Vec<YtRow> = parse_rows(&raw).context("parsing assigned_tt rows")?;
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM live_assigned_tt").execute(&mut *tx).await?;
        for r in &rows {
            let yt = r.ytno.trim();
            if yt.is_empty() { continue; }
            sqlx::query("INSERT INTO live_assigned_tt (ytno, jobstatus, as_of_ts) VALUES ($1,$2,$3)")
                .bind(yt).bind(r.jobstatus.as_deref().map(str::trim))
                .bind(as_of).execute(&mut *tx).await.context("insert live_assigned_tt")?;
        }
        tx.commit().await?;
        Ok(rows.len() as u64)
    }).await.map(|_| ())
}

async fn src_workqueue(pool: &PgPool, target: &str, date: chrono::NaiveDate, as_of: DateTime<Utc>) -> Result<()> {
    run_logged(pool, "WORKQUEUE", date, |_| async move {
        let raw = Toolbox::from_env(target)?.run_sql(SQL_WORKQUEUE).await?;
        let rows: Vec<QueueRow> = parse_rows(&raw).context("parsing workqueue rows")?;
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM live_workqueue").execute(&mut *tx).await?;
        for r in &rows {
            sqlx::query(
                "INSERT INTO live_workqueue
                   (qc, vessel, voyage, queuename, disload, seq, total_qty, comp_qty, plan_qty, as_of_ts)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
                 ON CONFLICT (qc, vessel, queuename) DO UPDATE SET
                   voyage=EXCLUDED.voyage, disload=EXCLUDED.disload, seq=EXCLUDED.seq,
                   total_qty=EXCLUDED.total_qty, comp_qty=EXCLUDED.comp_qty,
                   plan_qty=EXCLUDED.plan_qty, as_of_ts=EXCLUDED.as_of_ts",
            )
            .bind(&r.qc).bind(&r.vessel).bind(&r.voyage).bind(&r.queuename)
            .bind(&r.disload).bind(r.seq.map(|v| v as i32))
            .bind(r.total_qty.map(|v| v as i32)).bind(r.comp_qty.map(|v| v as i32))
            .bind(r.plan_qty.map(|v| v as i32)).bind(as_of)
            .execute(&mut *tx).await.context("insert live_workqueue")?;
        }
        tx.commit().await?;
        Ok(rows.len() as u64)
    })
    .await
    .map(|_| ())
}

/// Block prefix of a yard code: "10X-16" → "10X" (matches livemap's centroid keys).
fn block_prefix(s: &str) -> &str {
    s.split('-').next().unwrap_or(s).trim()
}

async fn src_workpool(pool: &PgPool, target: &str, date: chrono::NaiveDate, as_of: DateTime<Utc>) -> Result<()> {
    run_logged(pool, "WORKPOOL", date, |_| async move {
        let raw = Toolbox::from_env(target)?.run_sql(SQL_WORKPOOL).await?;
        let rows: Vec<MoveRow> = parse_rows(&raw).context("parsing workpool rows")?;

        // candidate (unassigned) aggregation: key = (queue, vessel, jobtype, src_block);
        // value = (count, representative rtg). Discharge groups by QC (src_block = None,
        // pickup = the crane); load groups by source block (pickup varies per container).
        let mut cand: HashMap<(String, String, String, Option<String>), (i64, Option<String>)> =
            HashMap::new();

        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM live_workpool").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM live_candidate").execute(&mut *tx).await?;

        let mut active = 0u64;
        for r in &rows {
            match r.jobstatus.as_deref() {
                Some("A") => {
                    let etw_ts = r.etw_dt.as_deref().and_then(parse_etw);
                    sqlx::query(
                        "INSERT INTO live_workpool
                           (queuename, vessel, voyage, jobtype, jobstatus, yt_status, ytno, armgc,
                            etw_ts, etw_raw, contno, msnseq, yt_topos, from_pos, to_pos, twintandem, as_of_ts)
                         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)",
                    )
                    .bind(&r.queuename).bind(&r.vessel).bind(&r.voyage)
                    .bind(&r.jobtype).bind(&r.jobstatus).bind(&r.yt_status).bind(&r.ytno).bind(&r.armgc)
                    .bind(etw_ts).bind(&r.etw_dt).bind(&r.contno).bind(&r.msnseq).bind(&r.yt_topos)
                    .bind(&r.from_pos).bind(&r.to_pos).bind(&r.twintandem).bind(as_of)
                    .execute(&mut *tx).await.context("insert live_workpool")?;
                    active += 1;
                }
                // unassigned demand → candidate pool (only truly unassigned: no truck yet)
                Some("Q") if r.ytno.as_deref().unwrap_or("").is_empty() => {
                    let jt = r.jobtype.clone().unwrap_or_default();
                    let src_block = if jt == "LD" {
                        r.yt_topos.as_deref().map(|t| block_prefix(t).to_string()).filter(|s| !s.is_empty())
                    } else {
                        None // discharge: pickup is the QC, not a yard block
                    };
                    let e = cand
                        .entry((r.queuename.clone(), r.vessel.clone(), jt, src_block))
                        .or_insert((0, None));
                    e.0 += 1;
                    if e.1.is_none() {
                        e.1 = r.armgc.clone().filter(|s| !s.is_empty());
                    }
                }
                _ => {}
            }
        }

        for ((queuename, vessel, jobtype, src_block), (n, rtg)) in &cand {
            sqlx::query(
                "INSERT INTO live_candidate (queuename, vessel, jobtype, src_block, rtg, n, as_of_ts)
                 VALUES ($1,$2,$3,$4,$5,$6,$7)",
            )
            .bind(queuename).bind(vessel).bind(jobtype).bind(src_block).bind(rtg)
            .bind(*n as i32).bind(as_of)
            .execute(&mut *tx).await.context("insert live_candidate")?;
        }

        // Attach the QC from the clean current queue snapshot (unique per vessel+queue),
        // avoiding the Oracle-side fan-out against reused historic queuenames.
        for t in ["live_workpool", "live_candidate"] {
            sqlx::query(&format!(
                "UPDATE {t} x SET qc = wq.qc
                   FROM live_workqueue wq
                  WHERE wq.vessel = x.vessel AND wq.queuename = x.queuename"
            ))
            .execute(&mut *tx).await.with_context(|| format!("attach qc to {t}"))?;
        }
        tx.commit().await?;
        tracing::info!(active, candidates = cand.len(), "workpool: active moves + candidate groups");
        Ok(rows.len() as u64)
    })
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_etw_14_and_17() {
        assert!(parse_etw("20260609094228").is_some());
        assert!(parse_etw("20260609094833726").is_some()); // trailing millis tolerated
        assert!(parse_etw("").is_none());
        assert!(parse_etw("2026").is_none());
        assert!(parse_etw("notadate012345").is_none());
    }

    #[test]
    fn parses_move_rows() {
        let raw = r#"{"result":"[{\"QUEUENAME\":\"34H-D\",\"VESSEL\":\"CLOA\",\"VOYAGE\":\"12E\",\"JOBTYPE\":\"DS\",\"JOBSTATUS\":\"A\",\"YT_STATUS\":\"F\",\"YTNO\":\"TT945\",\"ARMGC\":\"RTG122\",\"ETW_DT\":\"20260609101604681\",\"CONTNO\":\"EITU0580638\",\"MSNSEQ\":null,\"YT_TOPOS\":\"08T-1011\",\"FROM_POS\":\"208\",\"TO_POS\":\"208\",\"TWINTANDEM\":null}]"}"#;
        let rows: Vec<MoveRow> = parse_rows(raw).unwrap();
        assert_eq!(rows[0].queuename, "34H-D");
        assert_eq!(rows[0].ytno.as_deref(), Some("TT945"));
        assert!(parse_etw(rows[0].etw_dt.as_deref().unwrap()).is_some());
    }
}
