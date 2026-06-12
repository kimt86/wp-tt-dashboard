---
title: 아키텍처
description: 시스템 구조 — 데이터 소스·KPI·배차·사이클 감지.
sidebar:
  order: 0
---

시스템이 **어떻게** 도는지를 담습니다. 데이터 소스부터 지표·배차·사이클 감지까지.

- [TOS DB 레퍼런스](/kc/architecture/tos-db-reference/) — 작업지시·배정 원천 데이터
- [실시간 websocket 데이터](/kc/architecture/websocket-data/) — GPS·PLC 피드 구조와 함정
- [websocket 원본 필드](/kc/architecture/websocket-fields/) — 필드 전수 레퍼런스
- [KPI 산출](/kc/architecture/kpi-computation/) — 운영 지표 7종
- [정확도 보강](/kc/architecture/kpi-accuracy/) — websocket로 KPI 교차검증
- [배차 풀 (라이브)](/kc/architecture/dispatch-pools/) — 실시간 TT 분류
- [사이클 감지 v1](/kc/architecture/cycle-detection-v1/) — 현행 이동필터 로직
- [클라우드 용량 산정](/kc/architecture/capacity-planning/)

차세대 사이클 감지(v2 그림자)의 설계·검증은 [실험](/kc/experiments/)을 보세요.
