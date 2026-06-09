-- Live work pool (the queue of crane moves needing a TT), refreshed ~every 90s by the
-- extractor `workpool` tick from TOS JOB_QUEUE_SCHEDULE + JOB_ORDER_LIST. Both tables
-- are full-replace snapshots of the current live state (DELETE all + INSERT per tick);
-- the API reads them and the frontend fuses them with the live websocket PLC/GPS.
-- The API crate never touches Oracle; this is the only path that brings the work pool
-- into Postgres.

-- Per-QC work-queue plan: each row is one (crane, vessel, queue) chunk worked in `seq`
-- order, with progress (comp_qty / total_qty). Small (tens of rows).
CREATE TABLE IF NOT EXISTS live_workqueue (
  qc          TEXT NOT NULL,
  vessel      TEXT NOT NULL,
  voyage      TEXT,
  queuename   TEXT NOT NULL,
  disload     TEXT,                  -- 'D' discharge / 'L' load
  seq         INTEGER,               -- order the QC works its queues
  total_qty   INTEGER,
  comp_qty    INTEGER,
  plan_qty    INTEGER,
  as_of_ts    TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (qc, vessel, queuename)
);

-- Individual live moves (the task cards): assigned TT, crane-ready ETW, container,
-- yard positions. No natural key (twin moves share a container), so a serial id.
CREATE TABLE IF NOT EXISTS live_workpool (
  id          BIGSERIAL PRIMARY KEY,
  qc          TEXT,                  -- crane from the queue join
  queuename   TEXT NOT NULL,
  vessel      TEXT NOT NULL,
  voyage      TEXT,
  jobtype     TEXT,                  -- 'DS' discharge / 'LD' load
  jobstatus   TEXT,                  -- 'A' active / 'Q' queued / 'P' planned
  yt_status   TEXT,
  ytno        TEXT,                  -- assigned TT (e.g. 'TT1153'), NULL = unassigned
  armgc       TEXT,                  -- yard crane (RTG)
  etw_ts      TIMESTAMPTZ,           -- parsed crane-ready time (NULL if unset/malformed)
  etw_raw     TEXT,
  contno      TEXT,
  msnseq      TEXT,
  yt_topos    TEXT,                  -- yard block-bay (e.g. '10Q-0405')
  from_pos    TEXT,
  to_pos      TEXT,
  twintandem  TEXT,
  as_of_ts    TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS live_workpool_qc_idx ON live_workpool (qc, etw_ts);
CREATE INDEX IF NOT EXISTS live_workpool_unassigned_idx ON live_workpool (etw_ts) WHERE ytno IS NULL;
