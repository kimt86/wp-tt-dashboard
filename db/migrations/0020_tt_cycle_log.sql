-- tt_cycle_log: one completed per-truck work cycle, accumulated for ops analytics AND as
-- ML training data. A "cycle" = one container job (pickup → validated drop), where the drop
-- is confirmed by the SAME GPS movement filter the live cycle KPIs use (container1 changed
-- away from a non-empty value, held ≥30s AND physically carried ≥150m). A field-only
-- rewrite while the truck sits still is NOT a cycle (rejected as an artifact upstream).
--
-- Written by the API's cycle flusher (`spawn_cycle_flusher`): completed cycles are buffered
-- in memory in LiveMap and drained here every ~30s, mirroring util_tt_sample's sampler.
-- Job metadata (jobtype/vessel/voyage/qc/twintandem) is the truck's latched GPS state at
-- pickup, enriched from the live work pool (live_assigned_tt ⨝ live_workpool) when available.
-- Append-only. The UNIQUE(ytno, dropped_at) index makes the flush idempotent (ON CONFLICT
-- DO NOTHING) so an API restart can't double-write a cycle.
CREATE TABLE IF NOT EXISTS tt_cycle_log (
  id            BIGSERIAL PRIMARY KEY,
  ytno          TEXT NOT NULL,
  business_date DATE NOT NULL,
  shift         TEXT NOT NULL,
  -- job metadata snapshot at pickup
  jobtype       TEXT,                  -- DS/LD vessel work · MI/MO/LC yard moves
  vessel        TEXT,
  voyage        TEXT,
  container     TEXT,
  qc            TEXT,
  twintandem    TEXT,                  -- kept so twin/tandem lifts can be de-weighted in ML
  -- phase timestamps (UTC); nullable when the phase wasn't observed (e.g. truck already
  -- carrying at API start, so no pickup edge was seen)
  assigned_at   TIMESTAMPTZ,
  pickup_at     TIMESTAMPTZ,           -- container1 became non-empty (laden leg start)
  arrived_at    TIMESTAMPTZ,           -- arrival == 'ARRIVED' at the unload side
  dropped_at    TIMESTAMPTZ NOT NULL,  -- validated drop (cycle complete)
  -- leg durations (s) and path distances (m)
  idle_before_s INTEGER,               -- unassigned/idle time before this cycle began
  empty_leg_s   INTEGER,               -- assignment/idle end → pickup
  empty_leg_m   DOUBLE PRECISION,
  laden_leg_s   INTEGER,               -- pickup → drop
  laden_leg_m   DOUBLE PRECISION,      -- carried path length (the ≥150m the filter validated)
  cycle_s       INTEGER,               -- assigned_at (or pickup_at) → dropped_at
  -- quality flags
  movement_ok            BOOLEAN NOT NULL DEFAULT true,  -- passed the ≥150m carry filter
  incomplete             BOOLEAN NOT NULL DEFAULT false, -- assignment vanished mid-haul (reserved)
  container_to_container BOOLEAN NOT NULL DEFAULT false, -- A→B drop with no empty gap
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX  IF NOT EXISTS tt_cycle_log_bd_idx ON tt_cycle_log (business_date, shift, dropped_at);
CREATE INDEX  IF NOT EXISTS tt_cycle_log_yt_idx ON tt_cycle_log (ytno, dropped_at);
CREATE UNIQUE INDEX IF NOT EXISTS tt_cycle_log_uniq ON tt_cycle_log (ytno, dropped_at);
