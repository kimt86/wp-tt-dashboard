---
title: Live Map 재구현 명세
status: draft
created: 2026-06-04
updated: 2026-06-04
author: Copilot
tags: [architecture, livemap, specification, implementation]
---

# Live Map 재구현 명세

이 문서는 **기존 소스 코드를 참조하지 않고도 Live Map을 새로 구현할 수 있게**
기능 요구사항, 데이터 계약, 상태 규칙, 구현 순서만 정리한 명세다.

구현자는 이 문서를 기준으로 신규 설계를 해도 되고, 에이전트에게 그대로 작업
지시문으로 넘겨도 된다.

---

## 1. 제품 목표

Live Map의 목적은 운영자가 **지금 이 순간 어떤 차량이 어디에 있고 무엇을 하고
있는지**를 한 화면에서 파악하도록 돕는 것이다.

필수 가치:

1. 실시간성: 차량 위치와 상태가 초 단위로 갱신되어야 한다.
2. 가독성: 위성 지도 위에 차량, 상태, 작업 위치가 한눈에 보여야 한다.
3. 안정성: 연결이 흔들려도 자동 복구되어야 한다.
4. 확장성: 기본 지도만 먼저 만들고, 궤적/의도 경로/작업 패널은 나중에 붙일 수 있어야 한다.

---

## 2. 구현 범위

### 2.1 최소 구현(MVP)

아래가 되면 Live Map의 핵심 가치는 충족된다.

| 기능 | 설명 |
|------|------|
| 위성 지도 | 지도 배경은 위성 타일 기반 |
| 실시간 차량 표시 | 차량 latest snapshot을 지도 위 점으로 표시 |
| 장비 필터 | TT / RTG / Quay Crane / 기타 / 전체 |
| 검색 | `vehicle_id` 부분 문자열 검색 |
| 상태 필터 | TRAVELING / IDLE / LOADING / UNLOADING / STOPPED |
| 연결 상태 표시 | WebSocket 연결중/정상/끊김/stale 표시 |
| 선택/호버 | hover tooltip, click select, 선택 차량 강조 |

### 2.2 운영형 구현

운영자가 실제로 오래 쓰려면 아래 기능이 추가되는 편이 좋다.

| 기능 | 설명 |
|------|------|
| 레이아웃 오버레이 | 도로/블록 polygon을 위성 지도 위에 표시 |
| 상태 요약 칩 | 화면 하단에서 상태별 개수와 필터 제공 |
| 상세 패널 | 선택 차량의 작업/연료/GPS 정확도/매핑 정보 표시 |
| 레이어 패널 | 포인트/링크/영역 토글 |
| 최근 cycle 요약 | 선택 차량의 최근 업무 cycle 1건 표시 |

### 2.3 고급 구현

아래는 운영 분석 품질을 높이는 기능이다.

| 기능 | 설명 |
|------|------|
| 과거 trajectory | 최근 15분/1시간/4시간 궤적 조회 |
| raw vs corrected 비교 | 원본 GPS와 보정선 비교 |
| 작업 구간 강조 | LOADING / UNLOADING 구간을 궤적 위에 점으로 강조 |
| 의도 경로 | 현재 위치 -> 목적지(`topos1`) 점선 표시 |
| 동적 장비 좌표 | 크레인/PLC 계열 장비의 동적 world 좌표 표시 |

---

## 3. 사용자 경험 요구사항

## 3.1 메인 화면

- 전체 화면은 **지도 중심**이어야 한다.
- 지도 배경은 위성 타일을 사용한다.
- 차량은 상태별 색이 다른 원형 마커로 표시한다.
- 선택된 차량은 흰색 링 또는 유사한 강조 표현으로 표시한다.
- 장비 라벨(`vehicle_id`)은 확대 시에만 보이게 한다.

## 3.2 상단 바

상단 바에는 최소 아래 요소가 필요하다.

- 장비 탭: `TT | RTG | Quay Crane | 기타 | 전체`
- 검색창: `vehicle_id` substring 검색
- 표시 중인 차량 수 / 전체 차량 수
- 현재 시각
- 실시간 연결 상태 pill

## 3.3 상태 칩

하단 상태 칩은 아래 요구를 만족해야 한다.

- `ALL` + 5개 상태 칩
- 각 칩에 현재 개수 표시
- 칩 클릭 시 해당 상태만 필터
- 같은 칩 다시 클릭 시 필터 해제

## 3.4 상호작용

- **hover**: 차량 ID, 상태, block_id, 마지막 수신 상대시각 표시
- **single click**: 차량 선택
- **double click**: 상세 화면 이동 또는 host callback 실행
- **search/filter 변경**: 지도 마커에 즉시 반영

## 3.5 선택 차량 패널

선택 패널은 아래 항목을 우선순위대로 보여준다.

1. `vehicle_id`, 현재 상태, 마지막 수신 상대시각
2. 작업 정보
   - `jobtype`
   - `cur_loc -> topos1`
   - vessel / container
3. logical mapping
   - `block_id`
   - `point_id`
   - `point_type`
4. 차량 상태
   - 연료
   - GPS 정확도
   - 운전자
5. 최근 cycle 요약(있으면)
6. trajectory 컨트롤(고급 구현 시)

---

## 4. 시스템 구성

Live Map은 아래 4개 층으로 보면 된다.

```text
Realtime source
  -> transport layer (WebSocket + reconnect)
  -> latest-state store
  -> map projection + geo conversion
  -> UI shell (map, filters, panels)
```

각 층의 책임:

| 층 | 책임 |
|----|------|
| Transport | WebSocket 연결, backoff, 상태 노출 |
| Store | 차량별 latest snapshot 유지 |
| Geometry | projection 역변환, polygon/point/arc -> GeoJSON 변환 |
| UI | 지도, 필터, 상세 패널, 상호작용 |

## 4.1 권장 프런트 스택

구현 자유도는 있지만, 아래 조합을 권장한다.

| 영역 | 권장 선택 | 이유 |
|------|-----------|------|
| 지도 엔진 | **MapLibre GL JS** | raster/symbol/circle/line layer, GeoJSON source, fitBounds, zoom interpolation 지원 |
| 데이터 조회 | **TanStack Query** | 정적 geometry 캐시, polling, query key 관리에 적합 |
| 클라이언트 상태 | **Zustand** 또는 동등한 경량 store | latest snapshot map 관리가 단순함 |
| 실시간 연결 | 브라우저 기본 **WebSocket** | 서버 계약이 단순하고 별도 라이브러리 불필요 |
| 좌표 렌더 | **GeoJSON** | MapLibre source와 직접 연결 가능 |

## 4.2 지도 배경과 초기 영역

### 위성 타일

권장 기본값:

- provider: **ESRI World Imagery**
- tile URL:
  `https://services.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}`
- glyphs URL:
  `https://demotiles.maplibre.org/font/{fontstack}/{range}.pbf`

### 초기 viewport

레이아웃을 아직 받기 전 fallback 값:

```ts
center = [101.296, 2.927]
zoom = 14
```

실제 운영에서는 이 fallback을 오래 유지하지 말고, `layout/geometry`와 `projection`
을 모두 받은 뒤 **layout bbox를 lon/lat로 역변환해서 `fitBounds()`** 해야 한다.

현재 운영 자산 기준(2026-06-04 측정) fallback에 대응하는 실제 중심값은 아래와 같다.

```ts
computedCenter = [101.29548878251356, 2.9279147399459635]
```

권장값:

- fit padding: `80`
- fit duration: `0`
- `doubleClickZoom: false`  
  (double click을 상세 이동에 쓰기 때문)

### 실제 보여줄 영역

정답은 임의의 위경도 박스가 아니라 **layout 전체 bbox 영역**이다.

즉:

1. `bbox_mm`를 받는다.
2. 네 꼭짓점을 projection으로 lon/lat로 변환한다.
3. 그 bounds를 지도 초기 표시 영역으로 쓴다.

이 규칙을 쓰면 새 환경에서도 레이아웃이 바뀌어도 코드 수정 없이 대응 가능하다.

현재 운영 자산 기준(2026-06-04 측정) 실제 값:

### layout bbox (mm)

```ts
bboxMm = {
  min_x: 11448.0,
  min_y: 4331.0,
  max_x: 5898528.0,
  max_y: 940458.0,
}
```

### 투영 후 axis-aligned bounds (lon/lat)

```ts
mapBounds = {
  west: 101.27863561487992,
  south: 2.9027275588172343,
  east: 101.31234195014719,
  north: 2.9531019210746927,
}
```

### 네 꼭짓점 (lon/lat)

```ts
corners = {
  sw: [101.27863561487992, 2.9069646670870672],
  se: [101.30509233362167, 2.9531019210746927],
  ne: [101.31234195014719, 2.94886481280486],
  nw: [101.28588523140543, 2.9027275588172343],
}
```

---

## 5. 필수 데이터 계약

아래 계약은 코드가 아니라 **제품 인터페이스**다.  
백엔드가 이미 있으면 맞춰 쓰고, 없으면 이 shape대로 새로 만들면 된다.

## 5.1 Projection API

목적: 내부 좌표(mm/m)를 지도 좌표(lon/lat)로 역변환.

### Endpoint

`GET /api/v1/projection`

### Response

```ts
interface ProjectionParams {
  lat0_deg: number
  lon0_deg: number
  a11: number
  a12: number
  tx: number
  a21: number
  a22: number
  ty: number
  rms_residual_mm: number
  n_pairs: number
}
```

### 요구사항

- 응답은 immutable configuration 취급
- 한 번 받아 캐시해도 됨
- 행렬이 singular면 프런트는 오류 UI를 보여야 함

현재 운영 자산 기준(2026-06-04 측정):

```ts
projection = {
  lat0_deg: 2.9196835,
  lon0_deg: 101.28931016666668,
  a11: 502.9806996949668,
  a12: 859.4738699360356,
  tx: 1823219.333334009,
  a21: 870.9000901903794,
  a22: -498.7563719884642,
  ty: 331333.6666678039,
  rms_residual_mm: 794.7826900282276,
  n_pairs: 6,
}
```

## 5.2 Layout Geometry API

목적: 지도에 block / road polygon을 그리기 위한 정적 geometry 제공.

### Endpoint

`GET /api/v1/layout/geometry`

### Response

```ts
interface LayoutGeometry {
  layout_version: string
  x_dir: number
  y_dir: number
  bbox_mm: {
    min_x: number
    min_y: number
    max_x: number
    max_y: number
  }
  blocks: Array<{
    id: string
    vertices_mm: number[][]
  }>
  roads: Array<{
    id: string
    vertices_mm: number[][]
  }>
  points: Array<{
    id: string
    type: number
    x_mm: number
    y_mm: number
  }>
}
```

### 요구사항

- 응답은 초기 로딩 후 사실상 고정 데이터여야 함
- `bbox_mm`로 초기 fit bounds 가능해야 함
- polygon vertex는 시각화 가능한 순서여야 함

현재 운영 자산 기준(2026-06-04 측정):

- block 수: `310`
- road 수: `329`
- bbox(mm): `min_x=11448`, `min_y=4331`, `max_x=5898528`, `max_y=940458`

## 5.3 Realtime WebSocket

목적: 실시간 차량 latest snapshot 스트림 제공.

### Endpoint

`WS /ws/realtime`

### Envelope

```ts
interface RealtimeEnvelope {
  batch_id: string
  seq: number
  last: boolean
  rows: RealtimeRow[]
}
```

### Row

```ts
type VehicleState =
  | 'TRAVELING'
  | 'IDLE'
  | 'LOADING'
  | 'UNLOADING'
  | 'STOPPED'

interface RealtimeRow {
  vehicle_id: string
  time: string
  state: VehicleState
  x_m: number | null
  y_m: number | null
  trip_id: number | null
  block_id: string | null

  raw_x_mm?: number | null
  raw_y_mm?: number | null
  corr_x_mm?: number | null
  corr_y_mm?: number | null

  point_id?: string | null
  point_type?: number | null
  nearest_arc_id?: number | null

  jobtype?: string | null
  cur_loc?: string | null
  topos1?: string | null
  topos2?: string | null
  container1?: string | null
  container2?: string | null
  vslname?: string | null
  fuel_level_pct?: number | null
  arrival_status?: string | null
  gps_accuracy_m?: number | null
  driver_userid?: string | null
}
```

### 최소 구현에 필요한 필드

- `vehicle_id`
- `time`
- `state`
- `x_m`, `y_m`

### 운영형 구현에 유용한 필드

- `block_id`
- `jobtype`
- `cur_loc`, `topos1`
- `fuel_level_pct`
- `gps_accuracy_m`
- `point_id`, `point_type`

### 고급 구현에 필요한 필드

- `raw_x_mm`, `raw_y_mm`
- `corr_x_mm`, `corr_y_mm`

## 5.4 실시간 필드 사용 매트릭스

이 표가 실제 구현에서 가장 중요하다.  
**어떤 필드를 받아서 화면 어디에 쓰는지**를 명확히 정의한다.

| 필드 | 사용처 | 필수 여부 |
|------|--------|-----------|
| `vehicle_id` | store key, marker label, 검색, 상세 패널 | 필수 |
| `time` | out-of-order 방지, 상대시각, live tail merge | 필수 |
| `state` | 마커 색상, 상태 칩 집계, trajectory segment 색상 | 필수 |
| `x_m`, `y_m` | 메인 실시간 마커 위치 | 필수 |
| `trip_id` | 상세 패널/추적 맥락 | 선택 |
| `block_id` | hover tooltip, 상세 패널 | 운영형 |
| `raw_x_mm`, `raw_y_mm` | raw trajectory 선/점, live raw dot | 고급 |
| `corr_x_mm`, `corr_y_mm` | corrected trajectory, live corrected tail | 고급 |
| `point_id` | 선택 point 하이라이트, 상세 패널 | 운영형 |
| `point_type` | point 라벨, 상세 패널 | 운영형 |
| `nearest_arc_id` | 향후 링크 강조/디버깅 | 선택 |
| `jobtype` | 상세 패널 작업 정보 | 운영형 |
| `cur_loc` | 상세 패널 출발 위치 문자열 | 운영형 |
| `topos1` | 상세 패널 목적지 문자열, 의도 경로 계산 | 고급 |
| `container1`, `container2` | 상세 패널 적재 정보 | 선택 |
| `vslname` | 상세 패널 선박 정보 | 선택 |
| `fuel_level_pct` | 상세 패널 연료 바 | 선택 |
| `arrival_status` | 목적지 도착 태그 | 선택 |
| `gps_accuracy_m` | 상세 패널 GPS 정확도 | 선택 |
| `driver_userid` | 상세 패널 운전자 | 선택 |

---

## 6. 선택 기능용 추가 계약

아래는 고급 기능에 필요하다.

## 6.1 모든 포인트

`GET /api/v1/layout/all-points`

```ts
interface LayoutPoint {
  id: string
  type: number
  x_mm: number
  y_mm: number
}
```

용도:

- point 레이어 토글
- 선택 차량의 `point_id` 하이라이트

운영 메모:

- point 개수는 수만 건까지 갈 수 있다고 가정한다
- 정적 데이터이므로 aggressive cache 대상이다

현재 운영 자산 기준(2026-06-04 측정):

- 전체 point 수: `32,057`
- 렌더용 point 수(NEW/DELETE 제외): `32,053`
- quay(type 1): `3,845`
- block work(type 2,3): `18,760`
- gate in(type 5): `9`
- gate out(type 6): `5`
- other(type 0): `9,434`

## 6.2 모든 링크

`GET /api/v1/layout/all-arcs`

```ts
interface LayoutArc {
  arc_id: number
  type: number // 0=straight, 1=turn, 2=lane_switch
  trace_mm: [number, number][]
  road_id: string
  lane_id: string
}
```

용도:

- 링크 레이어 토글

운영 메모:

- arc 데이터는 raw JSON 기준 수십 MB가 될 수 있다
- **GZip 압축 + lazy load**가 사실상 필수다
- 초기 페이지 로딩에 포함하면 안 된다

현재 운영 자산 기준(2026-06-04 측정):

- 전체 arc 수: `60,438`
- straight(type 0): `30,798`
- turn(type 1): `4,510`
- lane switch(type 2): `25,130`

## 6.3 Trajectory API

`GET /api/v1/gps-quality/trajectory`

```ts
interface GpsTrajectoryPoint {
  time: string
  raw_x_mm: number | null
  raw_y_mm: number | null
  x_mm_corrected: number | null
  y_mm_corrected: number | null
  correction_method: string | null
  quality_flag: string | null
  speed_mps: number | null
  state: VehicleState | null
}

interface GpsTrajectoryResponse {
  vehicle_id: string
  from_time: string
  to_time: string
  points: GpsTrajectoryPoint[]
}
```

용도:

- 과거 궤적 표시
- raw vs corrected 비교
- LOADING/UNLOADING 구간 강조

## 6.4 목적지 좌표 조회

`GET /api/v1/topos/lookup`

```ts
interface ToposLookupEntry {
  x_mm: number
  y_mm: number
  source: 'arrived_centroid' | 'block_prefix_fallback'
  confidence_m: number | null
  n_samples: number
  last_seen_at: string | null
  fallback_from?: string | null
}

interface ToposDynamicEntry {
  dynamic: true
  hint: string
}

interface ToposLookupResponse {
  results: Record<string, ToposLookupEntry | ToposDynamicEntry | null>
}
```

용도:

- `topos1` 같은 정적 목적지 라벨을 world 좌표로 해석

## 6.5 동적 장비 좌표

`GET /api/v1/cranes/current`

```ts
interface CraneCurrentEntry {
  crane_id: string
  time: string
  freshness_s: number
  load_t: number | null
  is_loaded: boolean
  lock: boolean | null
  land: boolean | null
  hpos: number | null
  tpos: number | null
  world_x_mm: number | null
  world_y_mm: number | null
  world_source: 'tt_arrived_centroid' | 'plc_affine' | null
  world_n_samples: number
  world_freshness_s: number | null
}

interface CranesCurrentResponse {
  cranes: CraneCurrentEntry[]
}
```

용도:

- 동적 장비의 목적지 해석
- 상세 패널의 PLC 상태 표시

## 6.6 최근 cycle

`GET /api/v1/vehicles/{vehicle_id}/cycles`

```ts
type CycleType =
  | 'discharging'
  | 'loading'
  | 'internal_transfer'
  | 'other'
  | 'partial'

interface CycleSummary {
  cycle_id: number
  vehicle_id: string
  started_at: string
  ended_at: string | null
  closed: boolean
  cycle_type: CycleType
  origin_point_id: string | null
  origin_block_id: string | null
  dest_point_id: string | null
  dest_block_id: string | null
  total_distance_m: number | null
}
```

용도:

- 선택 차량 패널에 "최근 어떤 작업 cycle에 있는가" 표시

---

## 7. 상태 모델

새 구현은 아래 상태 모델을 기준으로 하면 된다.

## 7.1 최신 차량 스냅샷

```ts
interface VehicleSnapshot {
  vehicle_id: string
  time: string
  timeMs: number
  state: VehicleState
  x_m: number | null
  y_m: number | null
  raw_x_mm: number | null
  raw_y_mm: number | null
  corr_x_mm: number | null
  corr_y_mm: number | null
  trip_id: number | null
  block_id: string | null
  point_id: string | null
  point_type: number | null
  nearest_arc_id: number | null
  jobtype: string | null
  cur_loc: string | null
  topos1: string | null
  topos2: string | null
  container1: string | null
  container2: string | null
  vslname: string | null
  fuel_level_pct: number | null
  arrival_status: string | null
  gps_accuracy_m: number | null
  driver_userid: string | null
  received_at_ms: number
}
```

## 7.2 연결 상태

```ts
type RealtimeConnState = 'connecting' | 'open' | 'closed' | 'error'

interface RealtimeStatus {
  state: RealtimeConnState
  envelopes: number
  rows: number
  lastEnvelopeAt: number
  reconnects: number
}
```

---

## 8. 동작 규칙

이 부분은 구현 방식이 아니라 **반드시 지켜야 하는 동작 규약**이다.

## 8.1 WebSocket 처리

1. 수신 메시지는 즉시 렌더에 반영하지 말고 **배치 처리**한다.
2. 브라우저 1 frame 안에서 여러 rows를 합쳐 store에 적용한다.
3. 연결이 끊기면 **지수 backoff**로 재연결한다.
4. 백오프 상한은 10초 이내를 권장한다.
5. 브라우저 탭이 다시 visible이 되었을 때 stale이면 즉시 재연결한다.

권장 상수:

```ts
BACKOFF_INITIAL_MS = 200
BACKOFF_MAX_MS = 10_000
VISIBLE_STALE_RECONNECT_MS = 30_000
STALE_MS = 300_000
EVICT_INTERVAL_MS = 30_000
CLOCK_TICK_MS = 5_000
DBLCLICK_THRESHOLD_MS = 220
```

권장 처리 순서:

1. `onopen` -> 상태를 `open`으로 변경
2. `onmessage` -> `JSON.parse()`
3. `env.rows`가 배열인지 확인
4. pending queue에 push
5. `requestAnimationFrame`에서 한 번에 flush
6. `applyRows(rows)`로 latest-state store 갱신
7. `onclose` -> reconnect timer 등록
8. `visibilitychange`에서 stale/open 상태를 검사해 강제 재연결

추가 규칙:

- 클라이언트는 WebSocket으로 **데이터를 보내지 않아도 된다**
- envelope parse 실패는 조용히 버려도 되지만, debug counter는 남기는 편이 좋다
- `lastEnvelopeAt`은 stale 판정과 연결 상태 pill에 사용한다

## 8.2 latest-state store

1. store는 차량별 최신 1건만 유지한다.
2. 새 row의 `time`이 더 오래되면 무시한다.
3. `Date.parse(time)`가 실패하면 버린다.
4. `received_at_ms`는 클라이언트 수신 시각으로 기록한다.
5. 5분 이상 업데이트 없는 차량은 stale로 간주하고 제거한다.

권장 구현 형태:

```ts
Map<string, VehicleSnapshot>
```

이유:

- `vehicle_id` 기반 random access가 빠름
- 선택 차량 조회가 단순함
- 전체 리스트가 필요할 때만 `Array.from(map.values())` 하면 됨

## 8.3 지도 표시

1. 메인 차량 마커는 **raw GPS (`x_m`, `y_m`)** 기준으로 그린다.
2. corrected 좌표는 메인 마커가 아니라 trajectory 비교용으로만 사용한다.
3. 선택 차량 trajectory를 띄운다고 해서 지도 viewport를 자동으로 옮기지 않는다.
4. vehicle label은 확대 레벨이 충분할 때만 보인다.

## 8.4 쿼리/폴링 주기

권장 주기:

| 데이터 | 주기 / 캐시 정책 |
|--------|------------------|
| projection | 앱 시작 시 1회, 사실상 `staleTime: Infinity` |
| layout geometry | 앱 시작 시 1회, 사실상 `staleTime: Infinity` |
| all points | 필요할 때만 lazy load, `staleTime: Infinity` |
| all arcs | 필요할 때만 lazy load, `staleTime: Infinity` |
| trajectory | 선택 차량 + window 기준, 10초 단위 refetch |
| cranes current | 5초 polling |
| recent cycles | 5초 polling 또는 패널 open 시 refetch |

주의:

- `all-arcs`는 payload가 매우 클 수 있으므로 절대 초기 로딩에서 같이 가져오지 않는다
- `trajectory`는 현재 시각 매초 refetch가 아니라 **10초 bucket key**를 쓰는 편이 안정적이다

## 8.5 로컬 지속성

아래 UI 상태는 local storage 또는 동등한 사용자 설정 저장소에 보존하는 것을 권장한다.

- 레이어 토글 상태
- trajectory window
- raw trajectory 표시 여부
- 레이어 패널 open/close 상태

현재 기본값:

```ts
defaultTab = 'TT'
defaultSearch = ''
defaultStateFilter = null

defaultLayerToggles = {
  areas: true,
  pointsQuay: false,
  pointsBlock: false,
  pointsGateIn: false,
  pointsGateOut: false,
  pointsOther: false,
  linksStraight: false,
  linksTurn: false,
  linksLaneSwitch: false,
}

defaultTrajectoryWindowMin = 60
defaultTrajectoryShowRaw = false
defaultLayerPanelOpen = true
```

---

## 9. 지도 레이어 사양

레이어 순서는 중요하다. 아래 순서를 권장한다.

| 순서 | 레이어 | 설명 |
|------|--------|------|
| 1 | 위성 배경 | raster base |
| 2 | 도로 영역 | 반투명 fill |
| 3 | 블록 영역 | fill + line + label |
| 4 | 포인트 레이어 | quay / block / gate / other |
| 5 | 링크 레이어 | straight / turn / lane-switch |
| 6 | 선택 point | 선택 차량의 logical point 강조 |
| 7 | 의도 경로 | 현재 위치 -> 목적지 점선 |
| 8 | raw trajectory | 점선 + sample dots |
| 9 | corrected trajectory | 상태별 색상 선 |
| 10 | 작업 도트 | loading / unloading 점 |
| 11 | 차량 마커 | 실시간 차량 |
| 12 | 차량 라벨 | 확대 시 vehicle_id |
| 13 | 선택 링 | 선택 차량 강조 |

## 9.1 레이아웃 레이어 권장 스타일

| 레이어 | 권장 스타일 |
|--------|-------------|
| roads fill | `fill-color: #ffe5a8`, `fill-opacity: 0.18` |
| blocks fill | `fill-color: #7eb6ff`, `fill-opacity: 0.22` |
| blocks line | `line-color: #9ec6ff`, `line-width: 1.2`, `line-opacity: 0.8` |
| block labels | `minzoom: 16`, 흰 글자 + 검은 halo |

## 9.2 차량 레이어 권장 스타일

| 요소 | 권장값 |
|------|--------|
| vehicle circle radius | zoom 12=`4`, 16=`6`, 18=`8` |
| vehicle stroke | `1.5px`, 검정 계열 |
| vehicle label minzoom | `16` |
| selected ring radius | zoom 12=`9`, 16=`13`, 18=`17` |
| selected ring stroke | `2.5px`, 흰색 |

## 9.3 trajectory 레이어 권장 스타일

| 요소 | 권장값 |
|------|--------|
| raw trajectory | magenta dashed, width `2.5`, opacity `0.9` |
| raw dots | zoom 12=`1.8`, 16=`2.8`, 18=`4` |
| corrected trajectory | width `2.5`, 상태별 색상 |
| work dots | zoom 12=`2.5`, 16=`4`, 18=`5` |

## 9.4 point / arc 카테고리

point type 분류 권장:

- quay: `1`
- block work: `2`, `3`
- gate in: `5`
- gate out: `6`
- other: `0`, `4`, `7`, `10`, `11`

arc type 분류 권장:

- straight: `0`
- turn: `1`
- lane switch: `2`

권장 lazy-load 기준:

- point 레이어는 point 카테고리 중 하나라도 ON일 때 로드
- arc 레이어는 link 카테고리 중 하나라도 ON일 때 로드

### 권장 색상

| 상태 | 색상 |
|------|------|
| TRAVELING | `#4ea3ff` |
| IDLE | `#b1b1b1` |
| LOADING | `#ffd84e` |
| UNLOADING | `#c084fc` |
| STOPPED | `#ff6b6b` |

기타 추천:

- corrected travel trajectory: `#22d3ee`
- raw trajectory: `#ec4899`
- intended path: 보라색 dashed

---

## 10. 신규 구현 순서

에이전트는 아래 순서로 구현하는 것이 가장 안전하다.

### Phase 1 - 기본 지도

완료 기준: 차량 점이 위성 지도 위에서 실시간으로 움직인다.

작업:

1. projection API 연동
2. layout geometry API 연동
3. WebSocket 연결 모듈 구현
4. latest vehicle store 구현
5. 지도 렌더 + 차량 점 + 기본 필터 구현

### Phase 2 - 운영 UI

완료 기준: 운영자가 필터와 선택 기능으로 실제 사용 가능하다.

작업:

1. 상단 바
2. 상태 칩
3. hover tooltip
4. 선택 링
5. 상세 패널
6. 레이어 패널
7. 정적 포인트/링크 토글

### Phase 3 - 분석 보조 기능

완료 기준: trajectory와 의도 경로까지 운영 화면에서 확인 가능하다.

작업:

1. trajectory API 연동
2. raw / corrected 동시 표시
3. live tail merge
4. topos lookup
5. 동적 장비 좌표 연동
6. 최근 cycle 요약

---

## 11. 에이전트용 구현 지시문

아래 문단은 그대로 에이전트에게 넘겨도 된다.

```text
Live Map 기능을 신규 구현하라.
기존 코드 구조나 파일 경로를 전제로 하지 말고, 아래 제품 명세만 기준으로 설계하라.

목표:
- 실시간 차량 위치를 위성 지도 위에 표시
- 장비 탭, 검색, 상태 필터 제공
- hover/select/double-click 상호작용 제공
- WebSocket reconnect와 stale handling 포함

필수 계약:
- GET /api/v1/projection
- GET /api/v1/layout/geometry
- WS /ws/realtime

핵심 규칙:
- 차량별 latest snapshot만 유지
- out-of-order sample 무시
- stale vehicle eviction 필요
- 메인 마커는 raw GPS 좌표 사용
- corrected 좌표는 trajectory 비교용으로만 사용
- trajectory 조회 시 viewport 자동 fit 금지

권장 구현 순서:
1. transport + store
2. projection + geometry
3. map shell + markers
4. filters + hover + selection
5. detail panel + layer toggles
6. trajectory + intended path + cycle
```

---

## 12. 완료 판정 기준

### MVP 완료

- [ ] 페이지 로딩 후 실시간 연결이 열린다
- [ ] 차량 마커가 1초 단위 수준으로 갱신된다
- [ ] 장비 탭이 정상 동작한다
- [ ] 검색이 `vehicle_id`에 반영된다
- [ ] 상태 칩이 마커 필터에 반영된다
- [ ] hover tooltip이 뜬다
- [ ] single click으로 선택 강조가 된다
- [ ] stale 연결이 자동 복구된다

### 운영형 완료

- [ ] 블록/도로 레이어를 토글할 수 있다
- [ ] 선택 차량 상세 패널이 열린다
- [ ] block/point 정보가 패널에 보인다
- [ ] point/arc 레이어를 lazy load할 수 있다

### 고급 완료

- [ ] 최근 15분/1시간/4시간 trajectory를 조회할 수 있다
- [ ] raw와 corrected trajectory를 동시에 비교할 수 있다
- [ ] LOADING/UNLOADING 구간이 강조된다
- [ ] 의도 경로가 목적지 해석 결과에 따라 그려진다
- [ ] live tail이 trajectory 끝단에 자연스럽게 이어진다

---

## 13. 비기능 요구사항

- 차량 수백 대 수준에서도 UI가 버벅이지 않아야 한다.
- WebSocket burst를 받아도 렌더링은 프레임 단위 배치여야 한다.
- 느린 네트워크/일시 끊김에서 자동 복구되어야 한다.
- 정적 geometry와 설정성 데이터는 공격적으로 캐시 가능해야 한다.
- 선택 기능, trajectory 기능, point/arc 기능은 서로 느슨하게 결합되어야 한다.

이 명세를 만족하면 기존 구현을 보지 않고도 Live Map을 신규 구축할 수 있다.
