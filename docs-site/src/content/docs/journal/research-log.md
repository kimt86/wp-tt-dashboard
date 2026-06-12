---
title: 연구 일지
description: 무엇을 시도했고, 무엇이 틀렸고, 어떻게 정정했는지를 시간순으로 기록합니다.
sidebar:
  order: 2
---

무엇을 시도했고, 무엇이 틀렸고, 어떻게 정정했는지를 시간순으로 기록합니다. 성공만 남긴 일지는 다음 사람을 돕지 못하기에, **오판·번복·재검증**을 그대로 적습니다. 각 항목 끝에 근거가 되는 상세 문서를 연결했습니다.

## 2026-06-08 · 문제 정의

플랫폼이 존재하기 전, 질문을 먼저 정의한 날입니다.

### 세션 — 고객사 워킹 세션: "스마트 TT 배차"란 무엇인가

Westports(말레이시아) 컨테이너 터미널 운영진과 워킹 세션을 진행하고, 야드 트럭(TT) 배차를 데이터 기반으로 측정·개선한다는 목표를 문제 수준에서 정의했습니다. 왜 배차가 어려운 문제인지, 현행 운영 방식과 개선 기회가 어디에 있는지를 기록으로 남겼고, 이후의 모든 구축과 분석은 이 날의 질문으로 거슬러 올라갑니다.

관련 문서 → [챕터 01 · 스마트 TT 배차 세션 기록](/kc/research/tt-dispatch-problem/)

## 2026-06-09 · 측정 기반 구축

하루 동안 데이터 파이프라인·대시보드·문서를 한꺼번에 세웠습니다.

### 구축 — 대시보드·KPI 파이프라인 가동: "운영 DB 무접촉" 원칙

Rust(axum) API + PostgreSQL + React/Vite 구성으로 대시보드를 구축했습니다. 추출기가 운영 Oracle TOS를 타이머 주기로 Postgres에 복제하며, API는 운영 Oracle을 절대 직접 조회하지 않습니다(API 크레이트에는 Oracle 클라이언트 자체가 없습니다). KPI(운영 지표, 일/주/월 이력) · TT(배차 현황: 작업 풀/차량 풀) · MAP(라이브 맵) · HEALTH · FEED 페이지가 실데이터로 운영을 시작했습니다. 지표는 사이클타임, 공차거리·공차비율, TT 가동률, QC 대기, 시간당 처리량(move/hr) 등입니다.

관련 문서 → [KPI 산출 방법](/kc/architecture/kpi-computation/) · [TOS DB 레퍼런스](/kc/architecture/tos-db-reference/)

### 구축 — 실시간 websocket 수신 + 작업 풀 미러링

SSH 터널로 차량 GPS(`wpt_gps` 존)와 크레인 PLC(`ctab` 존) websocket을 수신하기 시작했습니다. TOS 스냅샷과 별개로 "차량이 실제로 움직인 것"을 보는 두 번째 눈이 생겨, 이후 모든 지표 교차 검증의 토대가 되었습니다. 시간당 처리량은 PLC 훅로드와 교차 검증합니다. 작업 풀은 TOS `JOB_QUEUE_SCHEDULE` + `JOB_ORDER_LIST`를 약 90초 주기로 `live_workpool` + `live_assigned_tt`에 미러링하며(JOBSTATUS A/B/Q, ytno=트럭, contno=컨테이너, twintandem 포함) TT 페이지의 수요·공급 화면을 구동합니다.

관련 문서 → [실시간 websocket 데이터](/kc/architecture/websocket-data/) · [배차 풀 (라이브)](/kc/architecture/dispatch-pools/)

### 문서 — 지식센터(/kc/) 개설: 기술 문서 8편 작성

TOS DB 레퍼런스 · websocket 데이터 · KPI 산출 · 배차 풀 · 정확도 보강 · 클라우드 용량 산정 · 예측 모형 연구 · 세션 기록을 작성해 지식센터를 열었습니다. "측정 근거를 코드 밖에 남긴다"는 원칙으로, 지표마다 왜 그 정의를 선택했는지를 함께 기록했습니다.

관련 문서 → [안내(여정 5개 챕터)](/kc/) · [클라우드 용량 산정](/kc/architecture/capacity-planning/)

## 2026-06-10 · 정밀화와 정정

전날 세운 지표를 실측으로 다시 검증한 날입니다. 이 날의 절반은 "우리가 틀렸던 것"의 기록입니다.

### 정정 — DB의 ETW는 수 시간 어긋나 있었습니다: ETW 게이트웨이 연동

TOS DB의 ETW(`JOB_ODR_ETW_DT`)를 그대로 쓰면 실제와 수 시간 차이가 난다는 사실을 확인했습니다. 대신 Azure의 `tos_etw_gateway`(FastAPI, TOS RPC `/RPC/yard/etw` 폴링)에서 정확값을 받도록 전환했습니다. SSH 터널(`wp-etw-bridge`, `127.0.0.1:18080`)로 수신해 `tos_etw_cntr` 테이블에 upsert하며, TTL은 30분입니다.

관련 문서 → [TOS DB 레퍼런스](/kc/architecture/tos-db-reference/) · [배차 풀 (라이브)](/kc/architecture/dispatch-pools/)

### 정정 — TT 가동률 재정의: 배정 기반 + 60초 샘플의 시간 기반 평균

가동률을 작업 풀 배정 기반으로 재정의했습니다: status A=작업 중, A·B·Q=투입. 순간 스냅샷이 아니라 60초 주기 샘플 테이블 `util_tt_sample`로 시간 기반 평균을 산출하도록 바꿔, 특정 순간의 우연한 값이 하루 지표를 흔들지 않게 했습니다.

관련 문서 → [KPI 산출 방법](/kc/architecture/kpi-computation/)

### 정정 — idle 102대 중 51대는 사실 배정 상태였습니다: staging 신설

dispatch 분류가 작업 풀 배정을 무시한 채 "공차+정지=idle"로 집계해 idle을 과다 산정하고 있었습니다. 검증 시점 idle 102대 가운데 51대가 실제로는 배정을 받은 상태였습니다. 공차+정지+배정 = **staging(배차·대기)** 상태를 신설하고, idle은 진짜 미배정만 남겼습니다(약 21대). "트럭이 놀고 있다"는 화면 표현 하나가 절반은 틀려 있었던 셈입니다.

관련 문서 → [배차 풀 (라이브)](/kc/architecture/dispatch-pools/)

### 발견 — container1은 적재가 아니라 "배정"입니다: LD 34대로 검증

GPS websocket의 container1 필드를 물리 적재로 읽으면 안 된다는 점을 규명했습니다. 이 필드는 TOS의 배정 필드로, 직전 drop 시점에 다음 컨테이너가 사전 배정됩니다(c2c). 검증: LD 트럭 34대가 container1이 세팅된 채 픽업 블록에 ARRIVED했습니다 — 적재 전인데 값이 차 있는 것입니다. 따라서 픽업 시점은 ARRIVED 신호의 측면 분류로 복원합니다: LD는 블록 도착=픽업/크레인 도착=드롭, DS는 반대, MI/MO는 첫 도착=픽업.

관련 문서 → [실시간 websocket 데이터](/kc/architecture/websocket-data/) · [정확도 보강](/kc/architecture/kpi-accuracy/)

### 구축 — 사이클 로그(tt_cycle_log) 가동: 이동 필터로 가짜 인도 기각

container1이 비-빈 값에서 바뀌는 엣지에 더해 **보유 ≥30초 + 운반 ≥150m**를 모두 만족해야 진짜 인도로 인정하고, 미달 건은 TOS 재배정 artifact로 기각하는 이동 필터를 넣었습니다. `tt_cycle_log`에는 ytno, 작업 메타(jobtype/vessel/voyage/container/qc/twintandem), 단계 타임스탬프(assigned_at=공차 시작 프록시, pickup_arrived_at, pickup_left_at, arrived_at, dropped_at), leg별 거리/시간을 기록하며 `UNIQUE(ytno, dropped_at)` 멱등 flush(30초)로 적재합니다. ML 학습 데이터 축적이 시작되었고, 대시보드에 CYCLES(사이클 이력) 페이지를 신설했습니다.

관련 문서 → [예측 모형 연구](/kc/research/tt-prediction/) · [정확도 보강](/kc/architecture/kpi-accuracy/)

### 구축 — 사이클 4단계 분해 + KPI 사이클타임 DS/LD 분리

사이클을 **공차이동**(배정→픽업 도착) / **받기**(픽업 도착→출발) / **부하이동**(출발→드롭측 도착) / **주기**(도착→drop)의 4단계로 분해했습니다. 단계 경계는 ARRIVED 신호에 의존하므로 일부 사이클은 부분 관측이며, 관측하지 못한 단계는 추정하지 않고 NULL로 둡니다. KPI의 사이클타임도 DS/LD를 분리한 중위수로 바꿨습니다 — 양하와 적하는 동선이 달라 하나로 합치면 양쪽 모두를 왜곡하기 때문입니다.

관련 문서 → [KPI 산출 방법](/kc/architecture/kpi-computation/)

### 정정 — "픽업 도착 < 배정" 역전 23건: 오독, 전수 조사, 그리고 번복

4단계 타임스탬프에서 픽업 도착이 배정보다 빠른 역전 23건이 나왔을 때, 처음에는 "트럭이 미리 가서 대기한 것"이라는 그럴듯한 운영 해석을 붙였습니다. **이 해석은 틀렸습니다.** 전수 조사 결과 역전 건은 전부 assigned_at == pickup_arrived_at이었고, 83%가 수집기 재시작 후 첫 사이클이었습니다 — 운영 현상이 아니라 관측 경계(재시작) 아티팩트였던 것입니다. 재시작 직후 첫 사이클에 시딩 가드를 추가하고, 오염된 56행의 해당 단계를 NULL로 정리했습니다. 이 페이지가 존재하는 이유가 바로 이런 기록입니다: 해석을 붙이기 전에 데이터를 전수로 확인할 것.

관련 문서 → [정확도 보강](/kc/architecture/kpi-accuracy/) · [예측 모형 연구](/kc/research/tt-prediction/)

## 다음 계획

- **4단계 캡처율 관찰** — ARRIVED 의존으로 인한 부분 관측이 어느 비율인지 추적하고, NULL 단계의 분포를 정기적으로 점검합니다.
- **사이클 데이터로 예측 모형 베이스라인** — 누적 중인 tt_cycle_log를 학습 데이터로 작업 소요시간 예측의 베이스라인을 측정합니다 — [예측 모형 연구](/kc/research/tt-prediction/)의 1단계.
