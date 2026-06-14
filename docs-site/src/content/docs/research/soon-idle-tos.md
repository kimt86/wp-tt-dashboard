---
title: 곧 유휴될 차량 감지 — TOS DB 최소부하 + QC/RTG 핸드오버
description: 웹소켓만으론 양하(DS) 곧유휴 정확도가 약하다. TOS DB에 최소 부하를 주면서 QC·RTG 핸드오버 시작 시점과 대상 차량을 얻는 방안 연구 — 과정과 결과.
sidebar:
  order: 3
---

**상태:** `검증됨`(핵심 골자) · `초안`(권장 설계) · **최종 검토:** 2026-06-14

배차 로직에서 가장 중요한 것 중 하나는 **곧 유휴될(soon-idle) 차량을 미리 감지**하는 것입니다. 웹소켓만으로 알아내면 좋지만 **양하(DS)는 정확도가 구조적으로 약하고**(RTG가 PLC를 안 보냄), 정답(라벨) 확보도 어렵습니다. 그래서 **TOS DB를 보조로** 쓰되, 운영 DB에 **부하를 최소로** 주면서 핵심 두 가지 — **(1) QC·RTG 핸드오버 시작 시점, (2) 핸드오버 대상 차량** — 을 얻는 방안을 연구했습니다. 본 문서는 그 **연구 과정과 결과**를 함께 남깁니다.

> **출처 등급:** **[코드]** = `crates/`에서 직접 확인 · **[DB]** = `psql 127.0.0.1:5433/wp_tt` 직접 쿼리 · **[문서]** = 지식센터/설계문서 · **(미검증)** = 문서·코드·DB로 확인 못 함. 운영원칙: **prod Oracle 직접조회 금지(추출기·Postgres만)**, 관측 못 한 값은 **NULL**(추정 금지).

## 한 장 요약 — 두 핵심에 대한 직답

| 핵심 질문 | 선적(LD) — 안벽 QC | 양하(DS) — 야드 RTG |
|---|---|---|
| **(1) 핸드오버 "시작 시점"** | 권위(사후): `MCH_OPERATION.ST_DT` **[코드/문서]** · 라이브 임박: QC PLC `ctab` load 전이(~1초) **[코드/문서]** · 라이브 예측: `JOB_ORDER_LIST.JOB_ODR_ETW_DT`(크레인 준비시각) **[코드/DB]** | **직접 신호 없음** — RTG는 PLC 미송신 **[코드/문서]** · 사후 프록시: `JOB_ORDER_HISTORY.JOB_HIST_ACTV_DT`(±수 초) · 라이브: RTG GPS가 그 차 레인 **≤30m** 진입(`RTG_BAY_M`) **[코드]** |
| **(2) 핸드오버 "대상 차량"** | 사후: `MCH_OPERATION.TRK_ID` **[코드/문서]** · 라이브: `JOB_ORDER_LIST.JOB_ODR_YTNO`(=`TT####`, GPS device id와 동일) **[코드/DB]** | 라이브: `JOB_ODR_YTNO` **[DB]** · 담당 RTG는 `JOB_HIST_ARMGC` **[코드/문서]** · 사후 이력의 트럭ID 컬럼은 **(미검증)** |

**핵심 비대칭(실측):** 차를 비우는 핸드오버가 LD는 PLC 있는 **QC**에서, DS는 PLC 없는 **RTG**에서 일어납니다. 라이브 DB `tt_cycle_log`(61,279 사이클, 2026-06-10~14)에서 **드롭측 크레인 도착 포착 = LD 50.1% vs DS 0.0%** **[DB]** — DS 23,219건 중 단 4건만 PLC로 잡힘. 그래서 "선적 곧유휴는 풀리고, 양하 곧유휴는 구조적으로 어렵다".

**최소부하 결론:** 곧유휴 입력의 **대부분(대상차량·작업종류·ETW·배정RTG·POW)은 이미 `live_workpool`에 90초 주기로 들어와 있습니다** — **LD 곧유휴엔 Oracle 추가추출이 불필요** **[DB: 234행 전부 ytno·armgc 보유, 95% ETW]**. 추가가 필요한 건 **단 하나**: 양하 "방금 유휴" 권위 확정(`JOB_ORDER_HISTORY.ACTV_DT` 저지연 폴링) — 이건 **증분 워터마크 + 진행중-only + 30~60초 주기**로 수~수십 행짜리 쿼리가 되어 Oracle 부하가 무시할 수준입니다.

:::tip[권장 — 하이브리드]
웹소켓을 **1차 라이브 감지**로, TOS를 **권위·라벨·지연보정 2차**로. LD는 PLC가 긴 호라이즌(~90~120초)을 주고, DS는 RTG 근접이 짧은 호라이즌(관여 후 ~30초)만 주되 **RTG 미관여 차는 풀에 넣지 않습니다**(오포함 방지).
:::

---

## 연구 과정 — 어떻게 알아냈나

추측을 배제하기 위해 **멀티에이전트 워크플로**로 조사했습니다. 모든 결론을 **세 출처로 삼각측량**하고, 핵심 주장은 **반증(refute) 시도**를 거쳤습니다.

### 방법

```
① 조사(Understand, 병렬 4)   현 곧유휴 한계 · QC 신호 · RTG 신호 · 추출현황/부하
        ↓
② 검증(Verify, 병렬 4)        핵심 TOS 스키마 주장 4건을 각각 반증 시도(기본값 '미확인')
        ↓
③ 종합(Synthesize)            검증 통과분만으로 최소부하 곧유휴 설계
```

- **삼각측량 원칙:** 어느 컬럼·테이블도 지어내지 않음. 모든 사실을 **[코드]**(추출 SQL·`livemap.rs`) + **[DB]**(서빙 Postgres 실측) + **[문서]**(지식센터) 중 둘 이상으로 교차확인하고, 못 한 것은 `(미검증)`.
- **운영원칙 준수:** prod Oracle은 한 번도 직접 조회하지 않음. TOS 스키마는 **추출 SQL이 실제로 SELECT하는 컬럼** + **그 추출 결과(Postgres 집계)** 로만 검증.
- 규모: 9개 에이전트, 166회 도구 호출, 라이브 DB 직접 쿼리(`tt_cycle_log` 61,279 · `tt_cycle_v2` 41,420 · `raw_k_crane_q_daily` · `live_workpool` 234행 · `etl_run_log` · `etl_watermark`).

### 검증 결과 — 핵심 주장 4건의 반증 판정

| # | 주장 | 판정 | 핵심 근거 |
|---|---|---|---|
| 1 | `MCH_OPERATION`이 QC move의 **시작·완료·트럭ID·크레인ID**를 모두 제공 | **확인** | 추출 SQL 4종(c07·c10·f2·e1c)이 한 행에서 `ST_DT`·`COMPDATE/TIME`·`TRK_ID`·`MACHNO`를 SELECT **[코드]**; 추출 결과 `raw_k_mph_realtime`로 실재 입증 **[DB]** |
| 2 | `JOB_ORDER_HISTORY`가 **대상 트럭·담당 RTG·시각**을 제공 → PLC 없는 RTG도 TOS로 관측 | **일부확인** | RTG(`ARMGC`)+시각(`YT_DIS_DT→ACTV_DT`)은 확인, K_CRANE_Q의 **97.1%가 DS**로 야드측임을 실증 **[DB]**. 단 **HISTORY 한 행에서 트럭ID가 나오는지는 (미검증)** — 트럭ID는 `MCH_OPERATION.TRK_ID`/라이브 `JOB_ODR_YTNO`에만 확인 |
| 3 | **진행중(시작O·완료X) move 식별 = 곧 유휴**를 완료 전에 알 수 있다 | **일부확인** | 완료 전 리드타임 실재(LD 도착→유휴 중앙값 ~60초) **[문서/DB]**. 단 "진행중"은 **필요조건일 뿐** — 적재이동 중도 진행중이라 `delivering`. 추가로 도착+크레인 관여 필요. DS는 도착≠임박(스냅샷 RTG 25m 이내 0/6)으로 반증 |
| 4 | 필요한 신호를 **증분·진행중-only 폴링**으로 Oracle 부하 최소화하며 추출 가능 | **일부확인** | 진행중-only·저부하는 확인(현 워크풀 `COMPDATE IS NULL`, ~3.7s/90s) **[코드/DB]**. 단 **"증분(워터마크)"은 미구현** — `etl_watermark` 테이블은 정의돼 있으나 **코드 참조 0건·행 0건** **[코드/DB]**. 현재는 전량 스냅샷 교체 |

> 이 "일부확인" 3건이 곧 **설계가 정직해야 할 지점**입니다: ① RTG 트럭ID는 라이브 `JOB_ODR_YTNO`로 우회, ② 진행중 식별엔 크레인 관여 확인을 반드시 결합, ③ 증분은 빈 `etl_watermark`를 실제로 가동.

---

## 연구 결과

### 1. 문제 — 왜 어려운가

#### 1.1 웹소켓 단독 분류의 약점

현행 곧유휴는 **순수 웹소켓 standalone 분류**입니다(`classify_tt()` **[코드: livemap.rs:694-770]**, TOS 무관). 각 TT를 매 스냅샷 5개 상태로 분류: `idle / empty_travel / delivering / soon_idle / wait_rtg`(+`staging`). 곧유휴 진입 = 적재중 AND `arrival=ARRIVED` AND 드롭사이드 도착 AND 크레인 관여. 드롭사이드는 jobtype으로 결정:

- **LD:** `topos`가 크레인(`^[CMZ][0-9]+$`)이고 그 PLC `last_seen` ≤120초(`STALE_AFTER_S`)면 `soon_idle`+"PLC 확인" **[코드:752-754]**.
- **DS:** 최근접 RTG ≤30m(`RTG_BAY_M`)면 `soon_idle`+"블록 RTG 근접", 멀면 `wait_rtg` **[코드:760-768]**.

확인된 상수 **[코드]**: `STALE_AFTER_S=120` · `RTG_BAY_M=30.0` · `IDLE_SPEED_KMH=3.0` · `MIN_CARRY_TRIP_M=150.0` · `SWAP_MIN_M=500.0`.

세 가지 약점:

1. **DS엔 물리 신호가 없다.** plc_data(`ctab`)는 동적 안벽크레인 C/M/Z만 송신, **RTG는 PLC 미수신** **[문서]**. 차를 비우는 순간 직접 신호가 없음.
2. **도착 ≠ 곧유휴.** RTG는 여러 블록·레인을 공유하며 도착한 차를 바로 안 받음. "도착했으나 RTG가 멀어 대기"가 일반적이라 `wait_rtg`로 분리.
3. **GPS는 성기다.** 장비당 ~3초, 정지 시 30~55초 **[문서]** — 도착/출발 시각이 ±수 초~수십 초 흔들림(단 RTG 위치 정확도는 중앙값 ~2m로 bay 판별엔 충분).

#### 1.2 라벨(정답) 난제

"차가 정확히 언제 비었나" 라벨이 어렵습니다:

- **`container1` 비는 시점 ≠ 물리 핸드오버.** `container1`은 **TOS 배정 필드이지 물리 적재가 아님** **[문서: [피드 의미론](/kc/research/feed-semantics/)]**. 직전 drop에 사전배정(c2c)되므로 물리 픽업이 container1 엣지가 아님 — 그래서 코드는 ARRIVED 상승엣지를 jobtype·드롭사이드로 분류해 픽업/드롭을 복원.
- **이동필터로 가짜 배정 제거 필요:** `carry_trip_m < 150m`면 TOS 재배정 아티팩트로 거부 **[코드: MIN_CARRY_TRIP_M]**.
- **DS 드롭측엔 정밀화할 신호가 없음:** QC측은 PLC가 ±1초로 스냅(LD `crane_arr_method=plc` 12,198건 **[DB]**), RTG측은 GPS dwell에만 의존.
- **타이밍 보정:** DS `topos1` 플립은 핸드오버 **완료 후 중앙값 +19초**(n=36) **[문서: [사이클 v2 실험](/kc/experiments/cycle-v2-shadow/)]** → 라벨 시각을 물리 순간에 맞추려면 이 오프셋 보정 필요.

### 2. TOS 신호 매핑

#### 2.1 QC(안벽) — `MCH_OPERATION`(권위·사후) + `JOB_ORDER_LIST`(라이브)

권위 테이블 = `TOSADM.MCH_OPERATION`(move 1건 = 1행). 4요소 모두 실재 컬럼으로 **확인**:

| 항목 | 컬럼 | 근거 |
|---|---|---|
| **핸드오버 시작** | `ST_DT`(YYYYMMDDHH24MISS, 초 단위) | **[코드: f2·e1c]** |
| **완료** | `MCH_OPER_COMPDATE`+`MCH_OPER_COMPTIME` | **[코드: c07/c10/f2/e1c]** — 추출 인덱스 술어 |
| **대상 트럭** | `TRK_ID` | **[코드: c07/c10]** |
| **크레인ID** | `MCH_OPER_MACHNO`(QC=`^C[0-9]+$`) | **[코드: c10 REGEXP_LIKE]** |
| 컨테이너 | `MCH_OPER_CONTNO` | **[코드: c07]** |

:::caution[시작 시각의 의미 주의]
`ST_DT`는 "크레인이 그 move를 시작한 시각"이지 "그 트럭과의 물리적 접촉 ±1초 순간"이 아닙니다. 초 단위 물리 순간은 QC PLC `ctab` load 전이로 ±10초 상관시켜 정밀화 **[문서: tt-prediction §6.2]**. `ST_DT`가 'hoist 시작'인지 '작업지시 개시'인지는 **(미검증)**.
:::

**라이브 임박 신호는 `MCH_OPERATION`이 아니다.** 모든 추출 SQL이 `WHERE MCH_OPER_COMPDATE='{{DAY_STR}}'`(완료일 인덱스)로 시작 → **완료 move만** 들어옴. 라이브 "진행중/곧유휴/ETW"의 실제 원천은:

- `JOB_ORDER_LIST`(90초 추출, `WHERE JOB_ODR_COMPDATE IS NULL` **[코드: workpool.sql:31]**): `JOB_ODR_ETW_DT`(크레인 준비시각)·`JOB_ODR_YTNO`(배정 TT)·`JOB_ODR_ARMGC`(배정 RTG).
- QC PLC `ctab`(웹소켓, ~1초): load 전이로 임박 PICKUP 감지.

> **요약:** `MCH_OPERATION` = 끝난 move의 권위 기록(소급·검증·라벨). 진행중·미래 = `JOB_ORDER_LIST`(ETW)+PLC(라이브). **둘을 혼동 금지.**

#### 2.2 RTG(블록) — `JOB_ORDER_HISTORY`(대기 기반·사후)

QC가 **move 기반**이면 RTG는 **대기 기반**입니다. 깨끗한 시작/완료 쌍 대신 대기 구간만 잡힙니다:

| 항목 | 컬럼 | 근거 |
|---|---|---|
| TT 하차 시각 | `YT_DIS_DT` | **[코드: c08]** |
| 크레인 활성(시작 프록시) | `JOB_HIST_ACTV_DT` | **[코드: c08]** |
| **담당 RTG** | `JOB_HIST_ARMGC`(RTG/ES만, C## 없음) | **[코드: c08·e5]** |
| **대상 트럭ID** | 직접 컬럼 **확인 안 됨** | **(미검증)** — 라이브 `JOB_ODR_YTNO`로 대체 |

핵심 파생값: `K_CRANE_Q = (ACTV_DT − YT_DIS_DT) × 86400`초 = "TT 하차 → 야드 크레인 활성까지 대기"(0..1800s) **[코드: c08]**. **이것이 RTG/블록측 신호임을 DB가 증명:** `raw_k_crane_q_daily` in-range 이벤트의 **97.1%가 DS** **[DB]**.

:::caution[존재하되 약하다]
`ACTV_DT`는 "활성화 시각"이지 RTG가 물리적으로 집은 순간이 아닙니다(±수 초, PLC보다 약함). `ACTV_DT`의 정확한 의미('물리 집기' vs '스케줄 활성')는 **(미검증)**.
:::

#### 2.3 분리·결합 키

| | 테이블 | 구분 키 | 시작/완료 |
|---|---|---|---|
| QC | `MCH_OPERATION` | `MACHNO ^C[0-9]+$`(RTG/M/Z도 동거) | `ST_DT`+`COMP` 쌍 ○ |
| RTG | `JOB_ORDER_HISTORY` | `ARMGC`(RTG/ES) | `YT_DIS_DT→ACTV_DT` 대기만 |

`MCH_OPERATION`은 RTG도 포함하므로 QC만 보려면 MACHNO 정규식 필터 필수 **[코드: c10]**.

### 3. 최소부하 추출 설계

#### 3.1 이미 있는 것 vs 추가 필요한 것

**90초 주기로 이미 Postgres에 있는 곧유휴 입력** **[DB: live_workpool 234행]**:

| 신호 | 컬럼 | 가용성(실측) |
|---|---|---|
| 대상차량 | `ytno`(`JOB_ODR_YTNO`) | **234/234 (100%)** — GPS device id와 동일 |
| 작업종류 | `jobtype` | DS 129 / LD 105 |
| ETW(시작 1차 예측) | `etw_ts`(`JOB_ODR_ETW_DT`) | **222/234 (95%)** |
| 배정 RTG | `armgc`(`JOB_ODR_ARMGC`) | **234/234 (100%)** |
| POW | `from_pos`/`to_pos`/`yt_topos` | — |

→ **LD 곧유휴엔 Oracle 추가추출 불필요.** 양하 RTG 식별도 `armgc`가 이미 있음(관여여부 판정만 GPS).

**추가가 필요한 것 단 하나:** 양하 "방금 유휴" 권위 확정 = `JOB_ORDER_HISTORY.ACTV_DT` 저지연 감지. 현재는 일자등식 일배치로만 들어와 저지연 폴링이 없음 **[코드: c08]**.

#### 3.2 현 추출 부하 — 실측

| 스트림 | 주기 | Oracle 부하 |
|---|---|---|
| **workpool**(곧유휴 핵심) | **90s** **[코드: wp-workpool.timer]** | WORKPOOL ~1.2s·1048행 + WORKQUEUE ~1.6s·647행 + ASSIGNED ~0.85s·372행 = **~3.7s/tick** **[DB: etl_run_log]** |

**이미 적용된 최소부하 기법** **[코드/DB]**: 진행중-only(`COMPDATE IS NULL`) · 상태 화이트리스트(`JOBSTATUS IN ('A','Q')`) · 시간경계(`CRE_DT >= TRUNC(SYSDATE)-2`) · 인덱스 안전 술어(함수래핑 없는 문자열 등식) · 동시성 직렬화(`ORACLE_LOCK` Mutex) · API는 Oracle 무접근.

**미적용:** **증분 워터마크 미구현** — `etl_watermark`는 정의돼 있으나 **코드 참조 0건·행 0건** **[코드+DB]**. 현재는 매 tick `DELETE` 후 전량 재삽입.

#### 3.3 양하 곧유휴 확정 폴링 — 권장 설계 `초안`

목표: `JOB_ORDER_HISTORY`에서 **"직전 폴링 이후 새로 완료된 DS 핸드오버"** 만 끌어와 호라이즌 0의 사후확인(권위 라벨/풀 보정)에 사용.

```sql
WHERE (JOB_HIST_DATE||SUBSTR(JOB_HIST_TIME,1,6)) > {{last_watermark}}
  AND JOB_HIST_JOBTYPE = 'DS'
  AND JOB_HIST_ACTV_DT IS NOT NULL
```

1. **증분 워터마크(빈 테이블 가동):** `params.rs`에 이미 `TimeCol::JobHist = "JOB_HIST_DATE||SUBSTR(JOB_HIST_TIME,1,6)"`(14자 비교키)가 있어 재사용 가능 — `etl_watermark.last_completed_at`을 실제로 갱신·소비.
2. **진행중-only 윈도우:** 직전 tick~현재로 좁혀 결과셋을 수~수십 행으로.
3. **폴링 주기 ↔ 지연:** 안 C는 호라이즌 0이라 **지연 = 폴링주기**. 양하 호라이즌이 "RTG 관여 후 ~30초"이므로 **30~60초 주기**면 충분(더 빠르면 불필요한 Oracle 히트만 증가). **운영시간 가드** 권장.
4. **부하 추정:** (30~60초 윈도우) × (DS 완료율) ≈ **수~수십 행/폴링** → 현 워크풀(1048행/1.2s)보다 가벼움.

:::caution[열린 질문 — 인덱스]
prod에 `JOB_HIST_DATE‖TIME` 또는 ROWID/시퀀스 위 인덱스가 있는지 DB로 확인 불가 → 인덱스 부재 시 워터마크 술어도 풀스캔 위험. **TOS팀 인덱스 확인 후 적용.** prod 실행계획 코스트도 본 환경에서 검증 불가 **(미검증)**.
:::

### 4. 하이브리드 감지 로직

#### 4.1 역할 분담

| 레이어 | 신호 | 역할 | 지연 | 신뢰 |
|---|---|---|---|---|
| **L1 라이브(웹소켓)** | GPS + QC PLC `ctab` | 1차 실시간 감지 | ~1~3초 | LD 높음 / DS 중간 |
| **L2 라이브(TOS)** | `live_workpool`(ETW·ytno·armgc) | 대상차량·임박시각 보강 | ~90초 | 중간 |
| **L3 권위(TOS)** | `JOB_ORDER_HISTORY.ACTV_DT`(증분 폴링) | 사후 확정·지연보정·라벨 | 30~60초 | 높음(DS) |

#### 4.2 LD(선적) — PLC 권위, 긴 호라이즌

`arrival=ARRIVED` + drop_at_crane + QC PLC 신선(≤120초) → `soon_idle`. 호라이즌: 적재차 도착→유휴 리드타임 중앙값 **~60초**(QC PLC PICKUP과 ±수 초 일치). 합의안 **90~120초**. 신뢰: 높음(drop_crane PLC 포착 50.1%).

#### 4.3 DS(양하) — RTG 근접 필요조건, 짧은 호라이즌, 풀 오포함 방지

`arrival=ARRIVED` + drop_at_block + 최근접 RTG ≤30m → `soon_idle`; RTG 멀면 `wait_rtg`(풀 제외). 호라이즌: RTG 관여 후 **~30초**. 신뢰: 낮음~중간(드롭측 크레인 직접포착 0.0%). **반드시 도착+RTG 관여 둘 다 충족 시에만 풀 포함.** L3 `ACTV_DT` 증분 폴링으로 "방금 비었음"을 30~60초 내 권위 확정 → topos1 +19초 오프셋 캘리브레이션.

#### 4.4 폴백 사다리

```
1순위: QC PLC load 전이 (LD, ±1초)       → soon_idle 확정 + 긴 호라이즌
2순위: live_workpool ETW (LD/DS, 90초)   → 임박시각 1차 예측
3순위: RTG GPS ≤30m (DS, bay 수준)       → 관여 확인 시에만 soon_idle, 짧은 호라이즌
4순위: ACTV_DT 증분 폴링 (DS, 30~60초)   → 사후 권위 확정·라벨 (호라이즌 0)
관측 못 함                                → NULL (추정 금지)
```

신뢰 점수: LD-PLC > DS-RTG관여 > ETW단독 > 도착단독(부적합, 0/6 관여로 반증).

### 5. 검증·라벨 전략

- 라이브 판정엔 GPS/PLC 근접, **학습 라벨엔 `ACTV_DT`+RTG 근접 구간** 병행. 관측 못 한 단계는 **NULL**.
- 이동필터(`carry_trip_m ≥ 150m`)로 가짜 배정 제거.
- DS 라벨 시각 −19초(중앙값)로 물리 완료 정렬.
- **그림자 승격 게이트(v2):** `tt_cycle_v2`(41,420행)에 병렬 기록 후 게이트 통과 시에만 메인 교체. G1: DS 픽업도착 ≥60%(v1 29%), LD 드롭도착 ≥70%(v1 42%) → v2 레그모델이 DS 32→66%, LD 43→77%로 충족 **[문서/DB]**.

## 리스크·열린 질문

| 항목 | 내용 | 상태 |
|---|---|---|
| DS 물리 순간 부재 | RTG PLC 없음 → "정확히 언제 비었나"는 GPS dwell/ACTV_DT 근사만 | 구조적, 해소 불가 |
| 워터마크 인덱스 | `JOB_HIST_DATE‖TIME`/ROWID 위 인덱스 존재 여부 | **(미검증)** — TOS팀 확인 필수 |
| `ACTV_DT` 의미 | '물리 집기' vs '스케줄 활성' | **(미검증)** |
| HISTORY 트럭ID | `JOB_ORDER_HISTORY`가 대상 트럭ID를 직접 주는지 | **(미검증)** — 라이브 `JOB_ODR_YTNO`로 대체 |
| prod Oracle 부하 | 신규 폴링의 실제 실행계획·서버 CPU/IO | **(미검증)** — 추출기 로그로만 간접 추정 |
| 안 A(RTG 예측) | 관여 *전* 예측(예측기 ⑤)은 미구현 | 미구현 |
| 보존 한계 | `JOB_ORDER_HISTORY` ~15일, `MCH_OPERATION` ~35일 → 깊은 백필 불가 | 운영 제약 |

## 다음 단계(제안)

1. `etl_watermark` 가동 + `JOB_ORDER_HISTORY.ACTV_DT` 증분 폴링 추출기 추가(30~60초, 운영시간 가드) — **TOS팀 인덱스 확인 선행**.
2. `classify_tt()`에 L3 보정 훅: DS `soon_idle`/`wait_rtg`를 `ACTV_DT` 확정으로 사후 검증(그림자).
3. 양하 라벨 정밀화(`ACTV_DT`+RTG 근접, −19초 보정)로 곧유휴 정확도 측정 → 게이트 통과 후 승격.

---

**관련 문서:** [예측 모형 연구 §7b](/kc/research/tt-prediction/) · [피드 의미론 실측](/kc/research/feed-semantics/) · [TOS DB 레퍼런스](/kc/architecture/tos-db-reference/) · [배차 풀(라이브)](/kc/architecture/dispatch-pools/) · [사이클 v2 그림자 실험](/kc/experiments/cycle-v2-shadow/)

**근거(절대경로):** 분류 `crates/api/src/livemap.rs`(`classify_tt` 694-770) · QC 추출 `crates/extractor/sql/{c07,c10,f2,e1c}*.sql` · RTG 추출 `crates/extractor/sql/{c08,e5}*.sql` · 라이브 `crates/extractor/sql/workpool.sql`(`COMPDATE IS NULL`)·`crates/extractor/src/{runner,params}.rs` · 스케줄 `deploy/systemd/wp-workpool.timer` · DB `tt_cycle_log`·`tt_cycle_v2`·`raw_k_crane_q_daily`·`live_workpool`·`etl_run_log`·`etl_watermark`.
