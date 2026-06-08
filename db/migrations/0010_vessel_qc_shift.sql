-- Per-vessel-per-QC throughput for the current shift, feeding the LIVE "QC별 처리량"
-- vessel-grouped cards. Folded from the same K_MPH shift rows the vessel panel already
-- uses (zero extra Oracle query); DELETE+INSERT per (business_date, shift) each tick.
CREATE TABLE IF NOT EXISTS vessel_qc_shift (
  business_date DATE NOT NULL,
  shift         TEXT NOT NULL CHECK (shift IN ('N','D','E')),
  vessel        TEXT NOT NULL,
  voyage        TEXT NOT NULL,
  qc            TEXT NOT NULL,
  moves           INTEGER,
  load_moves      INTEGER,
  discharge_moves INTEGER,
  active_hours    NUMERIC(8,2),
  mph             NUMERIC(8,2),
  first_move      TEXT,
  last_move       TEXT,
  as_of_ts        TIMESTAMPTZ NOT NULL,
  computed_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (business_date, shift, vessel, voyage, qc)
);
