# OpenAPI ↔ 구현 라우트/파라미터 수동 대조 체크리스트 (WBS 1.2)

WBS `1.2 OpenAPI/API 계약 재정렬(재검토)`의 재완료 조건 중
"구현 라우트/파라미터와 OpenAPI path/param 1:1 수동 대조" 증적 문서입니다.

## 점검 범위

- 구현 기준: `src/main.rs` 라우팅 분기
- 문서 기준: `docs/openapi.yaml`, `docs/swagger/openapi.yaml`
- 고정 엔드포인트: `/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}`

## 수동 대조 결과

| Endpoint | 구현(`src/main.rs`) | OpenAPI(`docs/openapi.yaml`) | Swagger OpenAPI(`docs/swagger/openapi.yaml`) | 판정 |
| --- | --- | --- | --- | --- |
| `GET /healthz` | `("GET", "/healthz")` 분기 존재 | `/healthz` + `get` 존재 | `/healthz` + `get` 존재 | PASS |
| `GET /readyz` | `("GET", "/readyz")` 분기 존재 | `/readyz` + `get` 존재 | `/readyz` + `get` 존재 | PASS |
| `GET /metrics` | `("GET", "/metrics")` 분기 존재 | `/metrics` + `get` 존재 | `/metrics` + `get` 존재 | PASS |
| `POST /jobs` | `("POST", "/jobs")` 분기 + query `engine`, `audio_ms` 파싱 | `/jobs` + `post` + query `engine`, `audio_ms` 정의 | `/jobs` + `post` + query `engine`, `audio_ms` 정의 | PASS |
| `GET /jobs/{job_id}` | `("GET", path) if path.starts_with("/jobs/")` + `job_id` 파싱/검증 | `/jobs/{job_id}` + `get` + path param `job_id`(required) | `/jobs/{job_id}` + `get` + path param `job_id`(required) | PASS |

## 점검 메모

- 경로 파라미터 명칭은 구현/문서 모두 `job_id`로 일치합니다.
- 기준 엔드포인트 5종의 path/method 존재 여부를 `cargo run --quiet --bin check_contract_drift`로 재검증했습니다.
- 동일 조건을 `python scripts/check_contract_drift.py --spec ... --report artifacts/contracts/local/contract-drift-report.json`로 재검증했고, report 생성 경로를 확인했습니다.

## 결론

- WBS 1.2 재완료 조건 중 **수동 대조 체크리스트 항목은 충족(PASS)** 입니다.
- CI 정적 점검/최종 재완료 판정은 별도 항목(기존 TODO 미완료)으로 유지합니다.
