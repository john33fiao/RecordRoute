# TODO 완료 이력

이 문서는 코드베이스 기반으로 완전 완료로 판단된 항목을 분리해 기록합니다.

기준일: 2026-02-28

## 완료 기준
- 해당 WBS 항목이 하위 항목 포함 모든 체크리스트가 `[x]` 이며, 근거 파일/동작이 코드베이스에 존재.
- 배포/스크립트 문서는 실제 실행 경로와 동기화됨.

## WBS 완료 항목 이관

### 1.0 아키텍처/계약 정합성
- 1.1 Rust 단일 진입점/엔진 경계 문서 기준 확정
  - 근거: `docs/architecture.md`, `docs/rust-cpp-backend-rewrite-plan.md`

### 2.0 런타임 스캐폴딩
- 2.1 Rust 실행 진입점 및 로깅 부트스트랩 구현 (`src/main.rs`, `tracing_subscriber` 사용)
- 2.2 HTTP 서버 최소 구현(`/healthz`, `/readyz`) (`src/main.rs` 라우팅/핸들러)

### 3.0 엔진 통합 기반
- 3.1 엔진별 클라이언트/포트 설정 (`18101`, `18102`, `18103`) (`src/main.rs`)
- 3.3 큐 포화/엔진 포화 `429` 규약 및 메트릭 라벨 분리 (`src/main.rs`)

### 4.0 잡 모델/오류 계약
- 4.1 잡 상태 전이 모델 (`queued` 초기 상태 + 조회 스켈레톤) (`src/main.rs`, `src/domain.rs`)
- 4.2 타임아웃 계층 분리의 HTTP/job 예산 계산 분리 근거 항목
  - `engine_connect_timeout`, `engine_request_timeout`, `audio_ms` 기반 예산 계산 근거가 코드에 존재.
- 4.3 에러 코드/응답 필드 계약 포맷 고정 (`ErrorBody { code, message }`) (`src/main.rs`, OpenAPI 스키마)

### 5.0 오디오 전처리/처리량 정책
- 5.1 `symphonia` 기반 오디오 정규화 (16kHz/16-bit mono WAV)
- 5.2 길이/예산 계산 기반 timeout 산정식 적용
- 5.3 whisper-server 추론 책임 한정(변환 책임 제거)

### 6.0 슈퍼비전/운영 안정성
- 6.2 graceful shutdown + 강제 종료 fallback (`src/engine_manager.rs`, `src/main.rs`)

### 7.0 배포/문서 분리
- 7.3 운영 점검 시나리오(장애/복구/부하) 문서화 (`docs/operations-runbook-scenarios.md`)
- 7.4 운영 점검 정례화(주기/역할/합격 기준/누적 완료조건)
  - 7.4.1 ~ 7.4.5 완료(운영 정책 문서/실행 이력 및 누적 완료 조건)

### 8.0 설치/실행 자동화 스크립트
- 8.1 프로젝트 설치 스크립트(Windows `.bat`) 작성 (`scripts/install_windows.bat`)
- 8.2 프로젝트 설치 스크립트(macOS/Linux `.sh`) 작성 (`scripts/install_unix.sh`)
- 8.3 프로젝트 실행 스크립트(Windows `.bat`) 작성 (`scripts/run_windows.bat`)
- 8.4 프로젝트 실행 스크립트(macOS/Linux `.sh`) 작성 (`scripts/run_unix.sh`)

### 9.0 Tauri 데스크톱 앱 전환
- 9.1 체크 헤더 하위 중 런타임 엔드포인트 해석 정책 단일화
  - `frontend/src/runtime/endpoints.ts`
  - `frontend/src/hooks/useWebSocket.ts`
  - 프론트-런처 정합성 기준(`resolveApiBaseUrl`, `resolveWebSocketUrl`)
- 9.2 프론트-백 계약 정합성 선결 (`docs/tauri-frontend-backend-contract-alignment.md`)
- 9.5 릴리스 품질 게이트 (`check_contract_drift` CI 연동, contract-drift/workflow 근거)

### 작업 로그/우선순위 상태 이관
- 7.4.5 운영 점검 정례화(7.4.1~7.4.5) 완료 이력 반영
- WBS 1.2 재완료 게이트 점검 항목 완료 (`docs/openapi-wbs-1.2-recompletion-gate.md`, `docs/openapi-impl-path-param-manual-checklist.md`)
- WBS 8.0 설치/실행 자동화 스크립트: 완료 상태로 이전

### 코드완성도 보강(임시 이력)
- `src/audio.rs` 정규화 유틸 연동 정리 (`normalize_to_wav_mono_16k`, `collect_mono_f32_from_f32`, `linear_resample`, `build_wav_mono_i16` 호출 경로 정리)
- `JobStatus::Canceled` 및 `mark_canceled` 취소 흐름 정리 (`frontend` 취소 이벤트/timeout 연동)
- `EngineManager::shutdown` 종료 경로 정리 (`src/engine_manager.rs`와 종료 플래그 경로 연동)
