-- 0006_qc_wait.sql
-- QC (quay crane) wait/idle time — the gap between a quay crane's merged active
-- work groups = time it waited (mostly for the next truck). Source: MCH_OPERATION
-- C## moves (tos-db-research phase_f/f2). Per quay crane, per business day.

CREATE TABLE raw_k_qc_q (
  snapshot_date      DATE NOT NULL,
  qc                 TEXT NOT NULL,
  idle_periods       INTEGER,
  quick_under_1m     INTEGER,
  normal_1_5m        INTEGER,
  delayed_5_10m      INTEGER,
  extended_10_30m    INTEGER,
  over_30m           INTEGER,
  avg_idle_sec       NUMERIC(10,1),   -- mean idle gap within 0..1800s
  med_idle_sec       NUMERIC(10,1),
  total_tt_wait_sec  NUMERIC(14,1),   -- cumulative 0..600s gaps (TT-wait estimate)
  total_idle_30m_sec NUMERIC(14,1),
  run_id             BIGINT NOT NULL REFERENCES etl_run_log(run_id),
  extracted_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (snapshot_date, qc)
);

-- per-QC wait column for the breakdown table (dimensionally correct: per quay crane)
ALTER TABLE kpi_breakdown_qc ADD COLUMN qc_wait_sec NUMERIC(8,1);
