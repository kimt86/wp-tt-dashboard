-- Add the true aggregation denominator to kpi_shift so the HISTORY "today" card can
-- be rebuilt by combining today's shift rows in Postgres (zero extra Oracle scan),
-- instead of a separate today-provisional Oracle tick.
--
-- For sum-weighted-mean KPIs the day value = Σ(value·weight)/Σ(weight) across the
-- day's shifts. For 4 KPIs the weight already equals sample_n (jobs / in_range /
-- idle_periods), but K_MPH (weight = active_hours, sample_n = voyages) and
-- K_EMPTY_R (weight = empty+laden metres, sample_n = jobs) need the real weight.
-- The rollup falls back to COALESCE(agg_weight, sample_n) for rows written before
-- this column existed, so completed shifts degrade gracefully (exact for the 4,
-- approximate for MPH/EMPTY_R) until the nightly authoritative run corrects them.
ALTER TABLE kpi_shift ADD COLUMN IF NOT EXISTS agg_weight NUMERIC(18,4);

COMMENT ON COLUMN kpi_shift.agg_weight IS
  'Aggregation denominator for combining shifts into a day value: Σ(value·agg_weight)/Σ(agg_weight). NULL for K_UTIL (avg-of-ratios, not linearly combinable; filled by nightly).';
