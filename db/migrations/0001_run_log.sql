-- 0001_run_log.sql
-- ETL bookkeeping: every extractor run is logged; per-KPI freshness is tracked
-- so the API/UI can show whether each KPI's data is current or stale.

CREATE TABLE etl_run_log (
  run_id        BIGSERIAL PRIMARY KEY,
  kpi_key       TEXT,                       -- KPI key, extract name, or 'ALL'
  business_date DATE,                        -- the business day the run covered
  started_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  finished_at   TIMESTAMPTZ,
  status        TEXT NOT NULL CHECK (status IN ('RUNNING','OK','PARTIAL','FAILED')),
  rows_written  INTEGER,
  error_text    TEXT
);

CREATE INDEX idx_etl_run_log_kpi_date ON etl_run_log (kpi_key, business_date);

-- One row per KPI: the "is this KPI trustworthy right now" signal.
CREATE TABLE data_freshness (
  kpi_key           TEXT PRIMARY KEY,
  last_success_date DATE,
  last_success_at   TIMESTAMPTZ,
  last_attempt_at   TIMESTAMPTZ,
  last_status       TEXT,
  is_stale          BOOLEAN NOT NULL DEFAULT false
);
