-- 0004_l1_rollups.sql
-- L1 "tidy shelf": the shapes the dashboard reads directly.

-- One row per KPI per day (long format) so all 6 headline KPIs share one code path.
CREATE TABLE kpi_daily (
  kpi_key        TEXT NOT NULL,               -- K_EMPTY|K_EMPTY_R|K_CRANE_Q|K_MPH|K_CYCLE|K_UTIL
  snapshot_date  DATE NOT NULL,
  value          NUMERIC(14,4) NOT NULL,      -- headline number in display units
  sample_n       INTEGER,                     -- N as the card shows it
  unit           TEXT,                        -- 'km/job'|'%'|'s'|'move/hr'
  source_grain   TEXT,                        -- how value was aggregated (audit)
  is_provisional BOOLEAN NOT NULL DEFAULT false, -- today's incremental value = true
  as_of_ts       TIMESTAMPTZ,                 -- watermark time of a provisional value
  computed_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (kpi_key, snapshot_date)
);

-- Per-QC breakdown for the B-view table. Phase 1: MPH is real per-QC; empty/crane
-- wait are NULL until per-QC source SQL exists.
CREATE TABLE kpi_breakdown_qc (
  snapshot_date  DATE NOT NULL,
  qc_machno      TEXT NOT NULL,
  jobtype        TEXT,
  mph            NUMERIC(8,2),
  empty_km       NUMERIC(8,3),
  crane_wait_sec NUMERIC(8,2),
  status         TEXT,                         -- EXCELLENT|MET|NEAR|MISS|DOWN
  computed_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (snapshot_date, qc_machno)
);

-- Day x hour empty-travel grid for the heatmap. Phase 1 may be sparse/approximated
-- (no hour-grained empty SQL yet); cells without data stay NULL.
CREATE TABLE kpi_heatmap_empty (
  snapshot_date DATE NOT NULL,
  hour          SMALLINT NOT NULL CHECK (hour BETWEEN 0 AND 23),
  empty_km_job  NUMERIC(8,3),
  jobs          INTEGER,
  computed_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (snapshot_date, hour)
);
