-- 0025: 6-event 사이클 모델로 정리.
-- 사이클시작(opened_at) → 공차이동시작(empty_travel_start_at) → 공차이동완료(empty_arrived_at)
--   → 부하이동시작(pickup_left_at) → 부하이동완료(laden_arrived_at) → 사이클종료(dropped_at)
-- 받기/주기(픽업·드롭에서의 체류=핸드오버 포함)는 도착~출발 간격으로 대체.
-- 크레인 핸드오버 *시작 시각*은 웹소켓으로 신뢰도 있게 관측 불가 → 수집 포기, 컬럼 제거.
ALTER TABLE tt_cycle_v2 ADD COLUMN IF NOT EXISTS empty_travel_start_at timestamptz;
ALTER TABLE tt_cycle_v2 DROP COLUMN IF EXISTS pickup_hand_start_at;
ALTER TABLE tt_cycle_v2 DROP COLUMN IF EXISTS pickup_plc_at;
ALTER TABLE tt_cycle_v2 DROP COLUMN IF EXISTS drop_hand_start_at;
ALTER TABLE tt_cycle_v2 DROP COLUMN IF EXISTS handover_src_pickup;
ALTER TABLE tt_cycle_v2 DROP COLUMN IF EXISTS handover_src_drop;
