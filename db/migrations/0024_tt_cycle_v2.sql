-- tt_cycle_v2: SHADOW leg-based cycle phases (design: docs/cycle-detection-v2-design.md).
-- One row per completed cycle, 1:1 with tt_cycle_log on (ytno, dropped_at). Written by the
-- same flusher; the v1 columns/logic are untouched. Phases come from the leg state machine:
--   cycle open  = first assignment signal (topos1 transition / container1 set) after the
--                 previous validated drop
--   leg         = one topos1 target (assignment -> arrival -> handover -> departure)
--   arrival     = arr_dtime > ARRIVED > cur_loc match > crane GPS proximity  (src recorded)
--   handover    = soon_idle engagement generalized: block = nearest RTG <= 30m;
--                 crane = flip-anchored PLC pairing (serving land's preceding lift) or
--                 first PLC activity fallback  (src recorded)
-- Validated against v1 via paired comparison; promotion gates in the design doc.
CREATE TABLE IF NOT EXISTS tt_cycle_v2 (
  ytno                 TEXT NOT NULL,
  dropped_at           TIMESTAMPTZ NOT NULL,   -- join key to tt_cycle_log
  opened_at            TIMESTAMPTZ,            -- v2 open (first assignment signal)
  jobtype              TEXT,
  -- pickup side (DS: crane / LD: block(s) / MI·MO: first block)
  empty_arrived_at     TIMESTAMPTZ,            -- pickup-leg arrival
  pickup_hand_start_at TIMESTAMPTZ,            -- handover start (est or paired)
  pickup_plc_at        TIMESTAMPTZ,            -- DS: serving land = physical completion (paired)
  pickup_left_at       TIMESTAMPTZ,            -- pickup-side departure (last pickup leg)
  -- drop side
  laden_arrived_at     TIMESTAMPTZ,            -- drop-leg arrival
  drop_hand_start_at   TIMESTAMPTZ,
  -- signal provenance (ML can weight by precision)
  arr_src_pickup       TEXT,                   -- arr_dtime|arrived|cur_loc|gps
  arr_src_drop         TEXT,
  handover_src_pickup  TEXT,                   -- plc_paired|plc_active|rtg_bay
  handover_src_drop    TEXT,
  legs                 JSONB,                  -- raw legs for debugging/validation
  created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (ytno, dropped_at)
);
CREATE INDEX IF NOT EXISTS tt_cycle_v2_drop_idx ON tt_cycle_v2 (dropped_at);
