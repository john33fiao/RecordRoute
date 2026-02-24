# WBS 1.2 재완료 게이트 — OpenAPI/API 계약 1:1 매핑 체크리스트

이 문서는 WBS `1.2 OpenAPI/API 계약 재정렬`의 **재완료 판정 기준**을 고정합니다.

목표는 Rust 구현 라우트/파라미터와 OpenAPI path/param 사이의 1:1 매핑을 증적 기반으로 검증하여,
운영 점검(주간 계약 드리프트 점검)과 CI 정적 점검의 판정 기준을 일치시키는 것입니다.

## 1) 적용 범위

- 구현 기준 엔드포인트(고정):
  - `GET /healthz`
  - `GET /readyz`
  - `GET /metrics`
  - `POST /jobs`
  - `GET /jobs/{job_id}`
- OpenAPI 기준 문서:
  - `docs/openapi.yaml`
  - `docs/swagger/openapi.yaml`

## 2) 재완료 판정 체크리스트 (모두 충족 시에만 READY)

아래 항목 중 하나라도 미충족이면 WBS 1.2는 **NOT READY**입니다.

1. **경로 집합 일치**
   - 구현 라우트 집합과 OpenAPI path 집합이 기준 엔드포인트 5종에서 완전 일치한다.
2. **파라미터 1:1 매핑 일치**
   - 각 엔드포인트의 path/query 파라미터 이름·필수 여부·위치가 구현과 OpenAPI에서 1:1로 일치한다.
3. **경로 파라미터 명칭 일치**
   - 잡 조회 경로 파라미터 명칭이 `job_id`로 통일되어 있다(`GET /jobs/{job_id}`).
4. **상태 enum 일치**
   - 잡 상태 enum이 `queued|running|completed|failed|timeout|canceled|rejected`로 구현/문서/OpenAPI에 동일하다.
5. **주간 점검 항목 활성화**
   - `docs/operations/weekly-drill/README.md`의 계약 드리프트 점검 체크리스트가 최신 기준으로 유지된다.
6. **CI 정적 계약 점검 증적 확보**
   - 필수 path/param 존재 + path-param 명칭 일치 검증 결과가 CI에서 확인 가능하다.
7. **증적 링크/경로 기록**
   - 회차 문서(또는 WBS 로그)에 정적 점검 산출물 링크/경로를 남긴다.
   - 예: `artifacts/contracts/<run-id>/contract-drift-report.json`

## 3) 판정 절차

1. 수동 대조: 구현 라우트/파라미터 ↔ OpenAPI path/param 체크리스트를 먼저 수행합니다.
2. 자동 대조: CI 정적 계약 점검 결과를 확인합니다.
3. 증적 기록: 주간 점검 문서 또는 WBS 로그에 결과(PASS/FAIL)와 산출물 링크/경로를 남깁니다.
4. 최종 판정: 2장 체크리스트 7개 항목이 모두 충족될 때만 `READY`로 판정합니다.

## 4) FAIL 처리 규칙

- 불일치가 하나라도 발견되면 즉시 `NOT READY`로 판정합니다.
- 액션 아이템(원인, 담당자, 기한, 재검증 방법)을 등록하고 다음 점검 회차에 재검증합니다.
- 재검증에서도 실패 시, WBS 1.2 완료 판정은 유지할 수 없습니다.

## 5) 운영 문서 연계

- 주간 운영 점검 기준: `docs/operations/weekly-drill/README.md`
- 진행/완료 추적: `docs/operations/weekly-drill/STATUS.md`, `TODO/TODO.md`

