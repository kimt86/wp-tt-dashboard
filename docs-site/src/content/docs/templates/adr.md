---
title: 템플릿 — 의사결정 기록 (ADR)
description: 되돌리기 어려운 선택을 남기는 ADR 골격.
sidebar:
  order: 2
---

:::note[복사해서 시작]
아래를 복사해 `decisions/NNNN-제목.md`로 만드세요(`NNNN`=4자리 순번). 한 ADR = 한 결정.
:::

````markdown
---
title: NNNN — 결정 제목
description: 한 줄 요약
sidebar:
  order: NNNN
---

**상태:** `제안됨` | `채택됨` | `폐기됨` | `대체됨(→ NNNN)` · **최종 검토:** YYYY-MM-DD

## 맥락
어떤 상황·제약에서 결정이 필요했나.

## 결정
무엇을 하기로 했나. 한 문장으로 분명하게.

## 대안
- A — 장단점, 기각 이유
- B — 장단점

## 결과
이 결정으로 무엇이 좋아지고 무엇을 감수하나. 후속 영향.
````
