-- 0028: 학습 센터 ① TT 이동시간 예측 — 검증된 사이클(tt_cycle_v2)의 연속 레그에서
-- (출발지점 → 도착지점) 실제 이동시간(출발 left → 도착 arrived)을 라벨로 수확.
-- 모델 v0 = per-(origin→dest) 중앙값(거리·시간대는 향후 v1 GBM 피처).

CREATE TABLE IF NOT EXISTS learn_travel_sample (
  ytno        text NOT NULL,
  dropped_at  timestamptz NOT NULL,
  leg_ord     integer NOT NULL,          -- 출발 레그의 순번 (멱등 키)
  origin      text NOT NULL,             -- 출발 topos
  dest        text NOT NULL,             -- 도착 topos
  travel_s    integer NOT NULL,          -- 라벨: 이동 소요(초) = 도착 - 출발
  dist_m      double precision,          -- 학습 좌표(②) 간 거리
  hour        integer,                   -- 도착 시각의 시(시간대 피처)
  captured_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (ytno, dropped_at, leg_ord)
);
CREATE INDEX IF NOT EXISTS learn_travel_sample_od ON learn_travel_sample (origin, dest);
CREATE INDEX IF NOT EXISTS learn_travel_sample_cap ON learn_travel_sample (captured_at);

-- 모델 품질 스냅샷 (시간순): 개선 곡선
CREATE TABLE IF NOT EXISTS learn_travel_metric (
  captured_at      timestamptz PRIMARY KEY DEFAULT now(),
  samples          bigint NOT NULL,      -- 누적 학습 표본
  od_pairs         integer NOT NULL,     -- 관측된 (출발→도착) 쌍
  confident_pairs  integer NOT NULL,     -- 확신(n≥10) 쌍
  median_speed_kmh double precision      -- 표본 중앙 속도
);
