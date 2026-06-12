---
title: 로드맵 초안
description: 단계별 계획 — 한 것·다음 (함께 다듬을 초안).
sidebar:
  order: 2
---

**상태:** `초안` · **최종 검토:** 2026-06-12

:::caution[함께 다듬을 초안]
우선순위는 함께 검토해 조정합니다.
:::

## 한 것 (요약)

- 실시간 대시보드 + KPI 7종 + websocket 정확도 보강.
- `idle→staging` 분류 교정, 라이브 배차 풀.
- **작업 사이클 적재** — `tt_cycle_log`(v1) + `tt_cycle_v2` 그림자.
- **사이클 v2** — 6-이벤트 모델(핸드오버 수집 포기), 픽업 레그 보장, CYCLES 페이지에 6이벤트 노출. 상세 [실험](/kc/experiments/cycle-v2-shadow/).

## 다음 (우선순위순)

### 1) 사이클 v2 승격 검토
v2 그림자가 경계(열림/닫힘) 정확도에서 v1을 추월하고 trip 포착도 개선됨. 메인 지표를 v2로 교체할지 게이트로 판단 — [실험](/kc/experiments/cycle-v2-shadow/) 참고.

### 2) AI 배차 — 예측 모형
누적된 `tt_cycle_log`/`tt_cycle_v2`를 연료로 예측 라벨 정의·베이스라인 측정. [리서치](/kc/research/tt-prediction/).

### 3) 백로그
- `tt_cycle_log.incomplete` 플래그(운반 중 배정 소멸).
- `topos2` latch 활용(트윈 레그 예고), `lapse`(체류 분) 교차검증.
- 재시작 내구성(진행 중 사이클 스냅샷).
- P2 trip 잔존 격차 — 크레인 드롭 도착 등 **관측 불가분은 NULL 유지**(허위 생성 금지).

## 원칙

- 라이브 핫패스 변경은 **그림자 검증 후 승격**(페어드 비교·게이트).
- 관측 못 한 값은 NULL. 추측 금지.
