-- 0026: 학습 센터 — 블록 작업지점 좌표 학습(②) 영속화 + 모델 품질 시계열.
-- livemap의 인메모리 centroids(topos→평균좌표)를 주기 스냅샷으로 영속화하고,
-- 모델 품질(커버리지·정밀도)을 시간순으로 적재해 "개선되는 모습"을 본다.

-- 학습된 작업지점 좌표 (모델): topos 코드별 대표 (lat,lon) + 표본수 + 정밀도
CREATE TABLE IF NOT EXISTS learn_topos_point (
  topos       text PRIMARY KEY,          -- 예 '03U-21'(작업지점) / '03U'(블록) / 'C35'(크레인)
  is_crane    boolean NOT NULL DEFAULT false,
  lat         double precision NOT NULL,
  lon         double precision NOT NULL,
  n           integer NOT NULL,          -- 평균 가중 표본수(≤500 캡, 적응성)
  obs         bigint  NOT NULL,          -- 누적 관측수(무제한) — 학습데이터 축적량
  spread_m    double precision,          -- 좌표 산포(m) = 모델 정밀도(작을수록 확신)
  updated_at  timestamptz NOT NULL DEFAULT now()
);

-- 모델 품질 스냅샷 (시간순): 개선 곡선용
CREATE TABLE IF NOT EXISTS learn_topos_metric (
  captured_at      timestamptz PRIMARY KEY DEFAULT now(),
  distinct_topos   integer NOT NULL,     -- 학습된 작업지점 수
  confident_topos  integer NOT NULL,     -- 확신(n≥30) 작업지점 수
  total_obs        bigint  NOT NULL,     -- 누적 관측수 합
  median_spread_m  double precision      -- 확신 지점들의 중앙 산포(m)
);
