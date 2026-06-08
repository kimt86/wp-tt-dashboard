-- 0007_kpi_shift.sql
-- Current-shift cumulative KPIs for the LIVE tab. One row per (date, shift, kpi)
-- holds the latest cumulative value (shift-start -> as_of_ts). kpi_shift_history
-- appends each tick so the API can compare against the previous shift at the SAME
-- elapsed minutes (apples-to-apples) and draw intra-shift sparklines. Postgres-only
-- (no extra Oracle load — it persists what each tick already computes).

CREATE TABLE kpi_shift (
  business_date DATE        NOT NULL,
  shift         TEXT        NOT NULL CHECK (shift IN ('N','D','E')),
  kpi_key       TEXT        NOT NULL,
  value         NUMERIC(14,4),
  sample_n      INTEGER,
  unit          TEXT        NOT NULL,
  as_of_ts      TIMESTAMPTZ NOT NULL,   -- the END_TS instant this cumulative reflects
  window_start  TIMESTAMPTZ NOT NULL,
  computed_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (business_date, shift, kpi_key)
);
CREATE INDEX kpi_shift_lookup ON kpi_shift (business_date, shift);

CREATE TABLE kpi_shift_history (
  business_date DATE        NOT NULL,
  shift         TEXT        NOT NULL,
  kpi_key       TEXT        NOT NULL,
  as_of_ts      TIMESTAMPTZ NOT NULL,
  elapsed_min   INTEGER     NOT NULL,
  value         NUMERIC(14,4),
  sample_n      INTEGER,
  PRIMARY KEY (business_date, shift, kpi_key, as_of_ts)
);
CREATE INDEX kpi_shift_history_lookup ON kpi_shift_history (business_date, shift, kpi_key, elapsed_min);
