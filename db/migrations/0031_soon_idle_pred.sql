-- Soon-idle prediction shadow log + accuracy time-series. Measures how well classify_tt's
-- soon_idle prediction matches the authoritative idle moment (tos_handover_label.comp_ts).
-- spawn_soon_idle_logger (30s) records each carry-trip's FIRST soon_idle entry with the firing
-- signal (source) and a counterfactual gps_would_fire flag (would GPS/PLC alone have fired?) so
-- we can isolate the TOS-correction hook's contribution. Hot path untouched; Postgres-only.
-- See research/soon-idle-tos (다음단계 ④).
CREATE TABLE IF NOT EXISTS tt_soon_idle_pred (
  id             BIGSERIAL PRIMARY KEY,
  ytno           TEXT NOT NULL,
  container      TEXT,                  -- trip id (= container1/latched/aj.contno = label.contno)
  jobtype        TEXT,                  -- DS / LD (matching key + stratification)
  qc             TEXT,
  topos          TEXT,                  -- handover work-point (block/crane code)
  predicted_at   TIMESTAMPTZ NOT NULL,  -- first soon_idle entry observed = lead-time reference
  source         TEXT NOT NULL,         -- gps_rtg | tos_actv | qc_plc | both | other
  gps_would_fire BOOLEAN NOT NULL,      -- would GPS/PLC alone have fired? (counterfactual for the TOS hook)
  nearest_rtg_m  DOUBLE PRECISION,      -- block: nearest RTG distance (NULL if none)
  reason         TEXT,                  -- classify_tt reason string (audit)
  business_date  DATE NOT NULL,
  shift          TEXT NOT NULL,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- one row per (ytno, container) trip; predicted_at differs per tick so the in-memory open-set
-- in the sampler is the real trip-once guard, this index is the exact-dup backstop.
CREATE UNIQUE INDEX IF NOT EXISTS tt_soon_idle_pred_uniq  ON tt_soon_idle_pred (ytno, container, predicted_at);
CREATE INDEX IF NOT EXISTS tt_soon_idle_pred_match_idx ON tt_soon_idle_pred (ytno, container);
CREATE INDEX IF NOT EXISTS tt_soon_idle_pred_bd_idx    ON tt_soon_idle_pred (business_date, shift, predicted_at);

-- Accuracy snapshot (improvement curve), learn_*_metric-shaped. Written by spawn_learn_persist.
-- Per (jobtype, source) rows carry precision+lead; per-jobtype source='ALL' rows carry recall.
CREATE TABLE IF NOT EXISTS tt_soon_idle_metric (
  captured_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  jobtype       TEXT NOT NULL,
  source        TEXT NOT NULL,         -- gps_rtg|tos_actv|qc_plc|both|ALL
  window_h      INT  NOT NULL,
  predictions   INT  NOT NULL,
  matched       INT  NOT NULL,
  precision_pct DOUBLE PRECISION,
  recall_pct    DOUBLE PRECISION,
  lead_p10_s    DOUBLE PRECISION,
  lead_p50_s    DOUBLE PRECISION,
  lead_p90_s    DOUBLE PRECISION,
  PRIMARY KEY (captured_at, jobtype, source, window_h)
);
