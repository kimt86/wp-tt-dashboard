-- SHADOW columns for crane-side arrival recovery — OBSERVATIONAL, additive, non-breaking.
-- The feed's arrival=='ARRIVED' flag is weak at quay cranes (~23% vs ~78% at blocks), so the
-- crane-side phase boundaries (DS pickup at crane, LD drop at crane) are under-captured in the
-- live columns (pickup_arrived_at / arrived_at). These shadow columns record a PARALLEL
-- estimate from crane GPS proximity + crane PLC activity, WITHOUT touching the live columns.
-- Once validated against the live columns over a clean window, the better signal can be promoted.
--   pickup_arrived_crane_at : DS — first detection at the assigned crane (pickup side)
--   arrived_crane_at        : LD — first detection at the assigned crane (drop side)
--   crane_arr_method        : which signal fired first — 'arrived' | 'gps' | 'plc'
ALTER TABLE tt_cycle_log ADD COLUMN IF NOT EXISTS pickup_arrived_crane_at TIMESTAMPTZ;
ALTER TABLE tt_cycle_log ADD COLUMN IF NOT EXISTS arrived_crane_at        TIMESTAMPTZ;
ALTER TABLE tt_cycle_log ADD COLUMN IF NOT EXISTS crane_arr_method        TEXT;
