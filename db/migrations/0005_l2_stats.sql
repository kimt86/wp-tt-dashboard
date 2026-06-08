-- 0005_l2_stats.sql
-- L2 "comparison shelf": static targets, accumulated baselines, paired-test inputs.

-- Static product constants (targets/excellent thresholds, direction, tier).
-- Seeded separately in db/seed/kpi_target.sql.
CREATE TABLE kpi_target (
  kpi_key         TEXT PRIMARY KEY,
  display_name    TEXT,
  unit            TEXT,
  direction       TEXT NOT NULL CHECK (direction IN ('LOWER_BETTER','HIGHER_BETTER')),
  target_value    NUMERIC(14,4),
  excellent_value NUMERIC(14,4),
  tier            TEXT
);

-- Baseline (4-week mean) + paired t-test, recomputed daily after kpi_daily updates.
CREATE TABLE kpi_baseline (
  kpi_key         TEXT NOT NULL,
  as_of_date      DATE NOT NULL,
  baseline_value  NUMERIC(14,4),
  baseline_n_days INTEGER,
  delta_abs       NUMERIC(14,4),
  delta_pct       NUMERIC(8,3),
  p_value         NUMERIC(8,5),
  cohens_d        NUMERIC(8,4),
  is_significant  BOOLEAN,
  meets_target    BOOLEAN,
  meets_excellent BOOLEAN,
  computed_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (kpi_key, as_of_date)
);

-- Finer-grain values (vessel/shift) feeding the paired t-test. The headline stays
-- in kpi_daily; this table holds the matched pairs the test consumes.
CREATE TABLE kpi_daily_pairs (
  kpi_key       TEXT NOT NULL,
  snapshot_date DATE NOT NULL,
  pair_key      TEXT NOT NULL,                -- vessel or shift identifier
  value         NUMERIC(14,4) NOT NULL,
  PRIMARY KEY (kpi_key, snapshot_date, pair_key)
);
