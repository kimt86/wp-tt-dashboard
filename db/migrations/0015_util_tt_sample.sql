-- util_tt_sample: periodic snapshots of TT assignment, written by the API every ~60s.
-- Each row = at sample time, how many TTs had an active job (assigned, from the live work
-- pool) out of the on-duty fleet (assigned ∪ manned-by-GPS). Averaging these over a shift
-- gives a TIME-BASED utilization that matches the operator definition (utilized from job
-- allocation to completion, queuing included; idle = unassigned time) and accrues history
-- forward — the per-truck historical allocation timestamp is not available in TOS.
CREATE TABLE IF NOT EXISTS util_tt_sample (
  ts            TIMESTAMPTZ NOT NULL DEFAULT now(),
  business_date DATE NOT NULL,
  shift         TEXT NOT NULL,
  assigned      INTEGER NOT NULL,
  on_duty       INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS util_tt_sample_bd_idx ON util_tt_sample (business_date, shift, ts);
