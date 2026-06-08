-- 0003_staging.sql
-- Real-time incremental layer. Intra-day, the extractor pulls only newly-completed
-- detail rows from Oracle (index range scan over a small time slice) into staging,
-- and Postgres computes the aggregates (incl. percentiles) cheaply. A per-stream
-- watermark records how far we've consumed each day.

CREATE TABLE etl_watermark (
  stream            TEXT NOT NULL,            -- 'jobs' | 'mch_oper' | 'mch_work'
  snapshot_date     DATE NOT NULL,
  last_completed_at TEXT,                     -- last consumed 'YYYYMMDDHH24MISS'
  updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (stream, snapshot_date)
);

-- Per-job detail for the JOB_ORDER_HISTORY KPIs (K_EMPTY/_R, K_CYCLE, K_CRANE_Q).
-- One row per job (deduped to 1 per CONTNO|POINT|SEQNO). Postgres aggregates these.
CREATE TABLE stg_jobs_today (
  snapshot_date DATE NOT NULL,
  job_key       TEXT NOT NULL,                -- CONTNO|POINT|SEQNO
  jobtype       TEXT,
  completed_at  TEXT,                         -- watermark-advancing timestamp
  empty_m       NUMERIC,                      -- UN_LNDN_TRV_RNG (filter-passing)
  laden_m       NUMERIC,                      -- LNDN_TRV_RNG
  cycle_sec     NUMERIC,                      -- for K_CYCLE
  crane_q_sec   NUMERIC,                      -- for K_CRANE_Q
  qc_machno     TEXT,                         -- for per-QC breakdown when available
  PRIMARY KEY (snapshot_date, job_key)
);

-- Work/stop/move intervals for K_UTIL. Interval merging is done in Postgres.
CREATE TABLE stg_intervals_today (
  snapshot_date DATE NOT NULL,
  machno        TEXT NOT NULL,
  machine_kind  TEXT,                         -- 'TT' | 'QC' | 'YC'
  start_dt      TEXT NOT NULL,                -- YYYYMMDDHH24MISS
  end_dt        TEXT NOT NULL,
  kind          TEXT NOT NULL,                -- 'work' | 'stop' | 'move'
  PRIMARY KEY (snapshot_date, machno, start_dt, kind)
);
