-- live_assigned_tt: every TT with an ACTIVE (in-flight) assignment of ANY job type
-- (DS/LD vessel work AND MI/MO/LC yard moves). Refilled each workpool tick. Used for
-- utilization — the DS/LD-only work pool undercounts trucks doing yard moves as idle.
CREATE TABLE IF NOT EXISTS live_assigned_tt (
  ytno     TEXT NOT NULL,
  as_of_ts TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS live_assigned_tt_asof_idx ON live_assigned_tt (as_of_ts);
