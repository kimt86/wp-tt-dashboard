//! Live work-pool snapshot extract. Pulls the current per-QC work-queue plan
//! (JOB_QUEUE_SCHEDULE) and the live container moves still to do (JOB_ORDER_LIST)
//! from TOS Oracle and full-replaces two Postgres snapshot tables every ~90s. This is
//! the ONLY path that brings the work pool into Postgres; the API crate can't reach
//! Oracle. Unlike the KPI extracts this is "live now" (no date window) — bounded
//! instead by status (live only) + a recent CRE_DT to keep the scan small.

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use wp_core::parse::parse_rows;

use crate::kpis::common::run_logged;
use crate::runner::Toolbox;

const SQL_WORKQUEUE: &str = include_str!("../sql/workqueue.sql");
const SQL_WORKPOOL: &str = include_str!("../sql/workpool.sql");

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
    tracing::info!(%as_of, "workpool tick done");
    Ok(())
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

async fn src_workpool(pool: &PgPool, target: &str, date: chrono::NaiveDate, as_of: DateTime<Utc>) -> Result<()> {
    run_logged(pool, "WORKPOOL", date, |_| async move {
        let raw = Toolbox::from_env(target)?.run_sql(SQL_WORKPOOL).await?;
        let rows: Vec<MoveRow> = parse_rows(&raw).context("parsing workpool rows")?;
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM live_workpool").execute(&mut *tx).await?;
        for r in &rows {
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
        }
        // Attach the QC from the clean current queue snapshot (unique per vessel+queue),
        // avoiding the Oracle-side fan-out against reused historic queuenames.
        sqlx::query(
            "UPDATE live_workpool wp SET qc = wq.qc
               FROM live_workqueue wq
              WHERE wq.vessel = wp.vessel AND wq.queuename = wp.queuename",
        )
        .execute(&mut *tx).await.context("attach qc to live_workpool")?;
        tx.commit().await?;
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
