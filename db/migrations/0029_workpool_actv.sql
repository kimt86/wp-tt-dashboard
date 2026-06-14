-- Add the RTG/order activation timestamp (JOB_ORDER_LIST.JOB_ODR_ACTV_DT) to the live
-- work pool. For DS (discharge) active orders this marks when the yard RTG became active
-- on the move — the soon-idle handover-start signal the websocket lacks (RTG sends no PLC),
-- paired with the already-present ytno (target truck). Additive (full-replace snapshot;
-- API selects explicit columns, so this is backward-compatible).
-- NOTE: ACTV_DT = order/RTG activation, NOT the ±1s physical lift (activation can lead
-- the lift by minutes). See research/soon-idle-tos (연구 2차).
ALTER TABLE live_workpool ADD COLUMN IF NOT EXISTS actv_ts  TIMESTAMPTZ; -- parsed UTC (NULL if unset/malformed)
ALTER TABLE live_workpool ADD COLUMN IF NOT EXISTS actv_raw TEXT;        -- raw YYYYMMDDHH24MISS (terminal MYT)
