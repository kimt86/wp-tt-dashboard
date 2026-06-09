-- Candidate job pool: UNASSIGNED jobs (JOBSTATUS='Q', no truck yet) that still need a
-- truck, aggregated by their dispatch pickup unit. Refreshed each ~90s workpool tick
-- from the SAME JOB_ORDER_LIST scan as live_workpool (no extra Oracle scan).
--   discharge (DS): pickup = the QC → grouped per (qc, queue).  src_block NULL.
--   load (LD):      pickup = the container's SOURCE yard block → grouped per source
--                   block (YT_TOPOS prefix), since one load queue spans many blocks and
--                   each block is a different pickup location/distance.
-- Urgency (how soon the QC reaches this work) is derived in the API from the queue's
-- seq + progress in live_workqueue. Full-replace snapshot each tick.
CREATE TABLE IF NOT EXISTS live_candidate (
  id          BIGSERIAL PRIMARY KEY,
  qc          TEXT,                  -- attached from live_workqueue (queue → crane)
  queuename   TEXT NOT NULL,
  vessel      TEXT NOT NULL,
  jobtype     TEXT,                  -- 'DS' discharge / 'LD' load
  src_block   TEXT,                  -- LD: source block prefix (e.g. '10X'); DS: NULL
  rtg         TEXT,                  -- representative yard crane (load)
  n           INTEGER NOT NULL,      -- unassigned job count in this group
  as_of_ts    TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS live_candidate_qc_idx ON live_candidate (qc);
