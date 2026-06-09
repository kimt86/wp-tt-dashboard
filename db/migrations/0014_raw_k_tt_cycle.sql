-- raw_k_tt_cycle: per-day per-truck cycle approximation from MCH_OPERATION.
-- One row per snapshot_date (overall, not per jobtype). The displayed K_CYCLE value is
-- the jobs-... here samples-weighted med_sec. The container handling span stays in
-- raw_k_cycle (kept internally, no longer the displayed cycle).
CREATE TABLE IF NOT EXISTS raw_k_tt_cycle (
  snapshot_date  DATE NOT NULL,
  trucks         INTEGER,
  samples        INTEGER,
  avg_sec        NUMERIC(10,2),
  med_sec        NUMERIC(10,2),
  p25_sec        NUMERIC(10,2),
  p75_sec        NUMERIC(10,2),
  run_id         BIGINT NOT NULL REFERENCES etl_run_log(run_id),
  extracted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (snapshot_date)
);
