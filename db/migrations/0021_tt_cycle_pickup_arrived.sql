-- Add the pickup-side arrival timestamp so a cycle can be split into the four canonical
-- TT phases for the history view:
--   공차이동 empty-travel  : assigned_at      → pickup_arrived_at  (drive empty to pickup)
--   받기   pickup-handover : pickup_arrived_at → pickup_at         (wait/receive at pickup)
--   부하이동 laden-travel   : pickup_at        → arrived_at         (drive loaded to drop)
--   주기   drop-handover   : arrived_at       → dropped_at         (handover at drop)
-- Captured from the GPS arrival=='ARRIVED' signal while the assigned truck is still empty.
-- NULL for container-to-container cycles (no observable empty leg) and pre-existing rows.
ALTER TABLE tt_cycle_log ADD COLUMN IF NOT EXISTS pickup_arrived_at TIMESTAMPTZ;
