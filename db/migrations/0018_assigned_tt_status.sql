-- carry the dispatch status so utilization can split active (A) vs deployed (A/B/Q).
ALTER TABLE live_assigned_tt ADD COLUMN IF NOT EXISTS jobstatus TEXT;
