-- 0008_vessel_shift.sql
-- Vessels worked in the current shift (LIVE tab panel). Derived from the same
-- current-shift MCH_OPERATION rows the K_MPH shift tick already fetches (no extra
-- Oracle query). planned_moves/berth are filled only if the optional voyage-plan /
-- crane-history probes find usable data; otherwise NULL -> UI shows "—".

CREATE TABLE vessel_shift (
  business_date   DATE        NOT NULL,
  shift           TEXT        NOT NULL,
  vessel          TEXT        NOT NULL,
  voyage          TEXT        NOT NULL,
  moves           INTEGER,
  load_moves      INTEGER,
  discharge_moves INTEGER,
  qc_count        INTEGER,
  qcs             TEXT,                 -- comma list "C12,C18,C24"
  mph             NUMERIC(8,2),
  first_move      TEXT,                 -- YYYYMMDDHH24MISS
  last_move       TEXT,
  planned_moves   INTEGER,              -- NULL until raw_voyage_plan exists
  berth           TEXT,                 -- NULL until a berth source is found
  as_of_ts        TIMESTAMPTZ NOT NULL,
  computed_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (business_date, shift, vessel, voyage)
);

-- Near-static per-voyage planned totals (for progress %). Populated only if the
-- VSS_STATISTICS probe finds an in-progress planned count. Refreshed slowly.
CREATE TABLE raw_voyage_plan (
  vessel        TEXT NOT NULL,
  voyage        TEXT NOT NULL,
  planned_moves INTEGER,
  source        TEXT,                   -- e.g. 'VSS_STT_MOVES' / 'VSS_STT_VAN'
  run_id        BIGINT REFERENCES etl_run_log(run_id),
  extracted_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (vessel, voyage)
);
