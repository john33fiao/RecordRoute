# 구현 방식 정리 (작업 로그 대체 문서)

이 문서는 날짜별 변경 로그를 대체해, 현재 유지되는 구현 방식만 정리합니다.

## 1) 아키텍처/전환 기본값

- Rust 오케스트레이터를 기본 런타임으로 사용하고, C++ 엔진(whisper.cpp/llama.cpp)은 HTTP 계약으로 분리 호출한다.
- 엔진별 큐를 분리해 단일 큐 병목을 피하고, 큐 포화/재시작 시 오케스트레이터에서 명시적 백프레셔(`429 queue_full|engine_full`)를 반환한다.
- 오디오 전처리는 `ffmpeg` 의존 실행 대신 `symphonia` 기반 파이프라인을 우선 사용한다.

## 2) API/계약 구현 방식

- Rust 목표 엔드포인트 기본 집합은 `/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}`이다.
- `POST /jobs`는 엔진별 라우팅(`stt|summarize|embed`)을 수행하고, 처리 timeout은 길이 기반 예측 식(`audio_ms`)으로 산정한다.
- 오케스트레이터는 `job_id` 형식(`^[a-zA-Z0-9_-]{1,64}$`)을 검증해 `invalid_job_id`를 반환한다.
- 상태 enum은 `queued|running|completed|failed|timeout|canceled|rejected`, 에러 enum은 OpenAPI 기준으로 유지한다.
- 리젝션 관측성은 엔진/사유 라벨 분리(`engine`, `reason`) 카운트와 `/metrics` 노출 항목으로 고정한다.

## 3) Frontend runtime 방식

- 프론트는 `frontend/src/runtime/endpoints.ts`에서 실행 환경(웹/데스크톱)을 구분해 API/WS URL을 결정한다.
- `VITE_API_BASE_URL`, `VITE_WS_URL` 우선 정책을 따르고 `VITE_TAURI_BACKEND_URL`은 데스크톱 런타임 단일 오버라이드로 사용한다.
- 기본 fallback은 데스크톱 프로토콜(`tauri:`, `asset:`)에서 `127.0.0.1:8080` 기준으로 축소한다.

## 4) 운영/안전성 구현

- `/readyz`는 큐/디스패처 이상 시 `503 degraded`를 반환한다.
- `/metrics`는 readiness, 엔진별 queue/running, 리젝션 스냅샷을 JSON 형태로 제공한다.
- 오케스트레이터·Swagger 라이프사이클은 `.github/workflows/tauri-lifecycle-poc.yml` 내 smoke gate에서 기동/종료·포트 충돌·3OS 매트릭스를 포함해 검증한다.
- 회귀 로그는 `artifacts/tauri-lifecycle/<ts-os-pid>/`에 수집한다.

## 5) 배포/릴리스 방식

- 설치는 `scripts/install_*.sh|bat --check`를 게이트로 두고, 빌드/패키징/실행 검증을 한 흐름으로 결합한다.
- 빌드 산출물은 실행 후보(`frontend/dist`, `target/release/...`)와 중간 산출물(`target/**` 일부 등)을 구분해 추적/제외 정책을 적용한다.
- 1인 운영자 환경 복구는 설치 게이트 재실행, 런타임 재시작, 아티팩트 기반 로그 재확인 순으로 수행한다.

## 6) 자산/보안 운영 방식

- `models/`은 raw 파일을 추적하지 않고 `manifest.yml|yaml|json`만 추적한다.
- `vendor/` 소스는 버전 관리하고 빌드 산출물은 ignore 처리한다.
- 패키징은 `tauri.conf.json` allowlist/CSP, 앱 데이터/로그/모델 경로, 비밀값 마스킹 정책으로 최소 권한을 유지한다.

## 7) 검증·회귀 추적 방식

- WBS 1.2 계약 검증은 구현 라우트/파라미터와 OpenAPI path/param 매핑 체크를 중심으로 재검토한다.
- 정적 계약 점검 산출물은 회차별로 경로/로그를 남겨 감사 가능하게 관리한다.
- 주간 점검은 `docs/operations/weekly-drill/README.md` 기준으로 운영/회귀 항목을 반복한다.

