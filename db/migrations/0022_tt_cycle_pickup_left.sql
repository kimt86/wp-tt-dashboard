-- container1 in the GPS feed is ASSIGNMENT-driven (the next box is pre-assigned at the
-- previous drop), so the physical pickup is invisible as a container1 edge. The pickup is
-- recovered from ARRIVED side-classification instead: ARRIVED at the job's LOAD side
-- (LD: block, DS: crane, MI/MO: first arrival) = pickup_arrived_at; the truck speeding up
-- again afterwards = pickup_left_at (laden travel start). Phase decomposition:
--   공차이동: assigned_at        → pickup_arrived_at
--   받기   : pickup_arrived_at  → pickup_left_at
--   부하이동: pickup_left_at     → arrived_at  (ARRIVED at the UNLOAD side)
--   주기   : arrived_at         → dropped_at
ALTER TABLE tt_cycle_log ADD COLUMN IF NOT EXISTS pickup_left_at TIMESTAMPTZ;
