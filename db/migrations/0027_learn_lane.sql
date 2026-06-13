-- 0027: 학습 센터 ③ 차량 주행 차선 — GPS 이동 트레이스를 격자(~22m)에 집계.
-- 셀별 통과수·진행방향(원형평균)·방향성(0~1, 1=일방통행)·평균속도를 누적해
-- 고통과 셀 = 도로/차선, 방향성으로 일방/양방 판별. (차선 개수는 GPS 해상도 한계.)

CREATE TABLE IF NOT EXISTS learn_lane_cell (
  lat_idx        integer NOT NULL,        -- round(lat / CELL_DEG)
  lon_idx        integer NOT NULL,
  lat            double precision NOT NULL, -- 셀 중심
  lon            double precision NOT NULL,
  passes         bigint  NOT NULL,         -- 누적 통과수
  heading_deg    double precision,         -- 원형평균 진행방향(0~360)
  directionality double precision,         -- 0~1 (1=일방통행, 0=양방/혼합)
  mean_speed     double precision,         -- 평균 속도(km/h)
  updated_at     timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (lat_idx, lon_idx)
);

CREATE TABLE IF NOT EXISTS learn_lane_metric (
  captured_at  timestamptz PRIMARY KEY DEFAULT now(),
  cells        integer NOT NULL,           -- 관측된 셀 수
  road_cells   integer NOT NULL,           -- 도로(통과≥20) 셀 수 = 커버리지
  total_passes bigint  NOT NULL,
  oneway_frac  double precision            -- 도로 셀 중 일방통행(방향성≥0.8) 비율
);
