-- Per-TT utilisation components per shift, so the "today" K_UTIL can be recombined
-- EXACTLY across shifts (mean-of-per-TT-capped-ratio is not a simple weighted mean):
--   day_util(tt) = min(1, Σ productive_min / Σ shift_elapsed_min)
--   K_UTIL = avg(day_util) · 100
-- elapsed_min is the shift window length used as the K_UTIL denominator (same for all
-- TTs in a shift). Written by the shift tick (≤50 rows/shift), DELETE+INSERT per shift.
CREATE TABLE IF NOT EXISTS util_tt_shift (
  business_date  DATE NOT NULL,
  shift          TEXT NOT NULL CHECK (shift IN ('N','D','E')),
  machno         TEXT NOT NULL,
  productive_min NUMERIC(10,2),
  elapsed_min    NUMERIC(10,2) NOT NULL,
  computed_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (business_date, shift, machno)
);
