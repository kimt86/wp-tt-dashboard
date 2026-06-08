-- 0002_raw_tables.sql
-- L0 "raw shelf": one table per validated Oracle SQL. Columns mirror each SQL's
-- SELECT output exactly. PK = snapshot_date (the SYSDATE-1 business day) + the
-- SQL's natural grain, so re-running a date overwrites idempotently.

-- 1. e3a_k_util_tt_merged.sql — per YardTractor, one business day
CREATE TABLE raw_k_util_tt (
  snapshot_date   DATE NOT NULL,
  machno          TEXT NOT NULL,
  sessions_total  INTEGER,
  interval_groups INTEGER,
  logout_anomaly  INTEGER,
  active_min      NUMERIC(10,2),
  stop_min        NUMERIC(10,2),
  productive_min  NUMERIC(10,2),
  k_util_capped   NUMERIC(6,3),
  k_util_raw      NUMERIC(6,3),
  run_id          BIGINT NOT NULL REFERENCES etl_run_log(run_id),
  extracted_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (snapshot_date, machno)
);

-- 2. e1c_k_util_crane_merged_intervals.sql — per crane (QC/YC)
CREATE TABLE raw_k_util_crane (
  snapshot_date     DATE NOT NULL,
  machno            TEXT NOT NULL,
  machine_type      TEXT NOT NULL CHECK (machine_type IN ('QC','YC')),
  interval_groups   INTEGER,
  total_moves       INTEGER,
  active_sec_merged NUMERIC(12,1),
  k_util_merged_24h NUMERIC(6,3),
  avg_grp_sec       NUMERIC(12,1),
  longest_grp_sec   NUMERIC(12,1),
  run_id            BIGINT NOT NULL REFERENCES etl_run_log(run_id),
  extracted_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (snapshot_date, machno)
);

-- 3. e4_k_empty_decomposition.sql — per jobtype x shift
CREATE TABLE raw_k_empty (
  snapshot_date   DATE NOT NULL,
  jobtype         TEXT NOT NULL,
  shift           TEXT NOT NULL,
  jobs            INTEGER,
  k_empty_ratio   NUMERIC(6,4),
  avg_empty_m     NUMERIC(10,2),
  avg_laden_m     NUMERIC(10,2),
  total_empty_m   NUMERIC(14,2),
  total_laden_m   NUMERIC(14,2),
  distinct_blocks INTEGER,
  run_id          BIGINT NOT NULL REFERENCES etl_run_log(run_id),
  extracted_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (snapshot_date, jobtype, shift)
);

-- 4. e3b_k_cycle_refined_v2.sql — per jobtype
CREATE TABLE raw_k_cycle (
  snapshot_date         DATE NOT NULL,
  jobtype               TEXT NOT NULL,
  jobs                  INTEGER,
  avg_sec               NUMERIC(10,2),
  med_sec               NUMERIC(10,2),
  std_sec               NUMERIC(10,2),
  p25_sec               NUMERIC(10,2),
  p75_sec               NUMERIC(10,2),
  p95_sec               NUMERIC(10,2),
  outlier_threshold_sec NUMERIC(10,2),
  outlier_n             INTEGER,
  avg_transitions       NUMERIC(8,2),
  run_id                BIGINT NOT NULL REFERENCES etl_run_log(run_id),
  extracted_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (snapshot_date, jobtype)
);

-- 5. e5_k_crane_q_by_hour.sql — per hour of day (0-23)
CREATE TABLE raw_k_crane_q_hour (
  snapshot_date       DATE NOT NULL,
  hour                SMALLINT NOT NULL CHECK (hour BETWEEN 0 AND 23),
  events              INTEGER,
  avg_sec             NUMERIC(10,2),
  med_sec             NUMERIC(10,2),
  std_sec             NUMERIC(10,2),
  p25                 NUMERIC(10,2),
  p75                 NUMERIC(10,2),
  p95                 NUMERIC(10,2),
  alert_threshold_sec NUMERIC(10,2),
  distinct_cranes     INTEGER,
  run_id              BIGINT NOT NULL REFERENCES etl_run_log(run_id),
  extracted_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (snapshot_date, hour)
);

-- 6. phase_c/08_k_crane_q.sql — per work_date x jobtype (work_date is intrinsic to the SQL)
CREATE TABLE raw_k_crane_q_daily (
  work_date          DATE NOT NULL,
  jobtype            TEXT NOT NULL,
  events_nn          INTEGER,
  in_range           INTEGER,
  k_crane_q_avg_sec  NUMERIC(10,2),
  k_crane_q_med_sec  NUMERIC(10,2),
  k_crane_q_std_sec  NUMERIC(10,2),
  min_sec            NUMERIC(10,2),
  max_sec            NUMERIC(10,2),
  anomaly_negative   INTEGER,
  anomaly_over_30m   INTEGER,
  run_id             BIGINT NOT NULL REFERENCES etl_run_log(run_id),
  extracted_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (work_date, jobtype)
);

-- 7. phase_c/06_k_mph_voyage.sql — per vessel x voyage (30-day window)
CREATE TABLE raw_k_mph_voyage (
  vessel        TEXT NOT NULL,
  voyage        TEXT NOT NULL,
  confirmed_at  TEXT,                 -- VSS_STT_UP_DT wall-clock string YYYYMMDDHH24MISS
  confirmed     TEXT,                 -- VSS_STT_CONFIRM 'Y'/'N'
  stt_check     TEXT,
  containers    INTEGER,
  teu           NUMERIC(10,1),
  moves         INTEGER,
  single_moves  INTEGER,
  twin_moves    INTEGER,
  tandem_moves  INTEGER,
  gross_min     NUMERIC(10,1),
  net_min       NUMERIC(10,1),
  berth_min     NUMERIC(10,1),
  work_qc       INTEGER,
  k_mph_gross   NUMERIC(8,2),
  k_mph_net     NUMERIC(8,2),
  k_bp_gross    NUMERIC(8,2),
  k_bp_net      NUMERIC(8,2),
  snapshot_date DATE NOT NULL,        -- as-of date of the run; window is 30d ending here
  run_id        BIGINT NOT NULL REFERENCES etl_run_log(run_id),
  extracted_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (vessel, voyage)         -- voyage = stable identity; latest run wins
);

-- 8. phase_c/07_k_mph_realtime.sql — per vessel x voyage x QC, one business day
CREATE TABLE raw_k_mph_realtime (
  snapshot_date         DATE NOT NULL,
  vessel                TEXT NOT NULL,
  voyage                TEXT NOT NULL,
  qc_machno             TEXT NOT NULL,
  moves                 INTEGER,
  load_moves            INTEGER,
  discharge_moves       INTEGER,
  active_hours          NUMERIC(8,2),
  k_mph_per_active_hour NUMERIC(8,2),
  distinct_trucks       INTEGER,
  distinct_containers   INTEGER,
  first_move            TEXT,           -- YYYYMMDDHH24MISS concat string from source
  last_move             TEXT,
  run_id                BIGINT NOT NULL REFERENCES etl_run_log(run_id),
  extracted_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (snapshot_date, vessel, voyage, qc_machno)
);
