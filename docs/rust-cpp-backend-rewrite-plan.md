# RecordRoute Rust/C++ 백엔드 전환 상세 계획

이 문서는 `docs/architecture.md`의 기준선을 **구현 가능한 단계**로 내린 실행 계획입니다.
WBS 추적은 `TODO/TODO.md`를 사용합니다.

## 0) 현재 코드베이스 스냅샷 (2026-02-22)

### 이미 구현됨
- 루트 Rust 바이너리 프로젝트가 존재 (`Cargo.toml`, `src/main.rs`)
- `tokio` 비동기 런타임으로 실행 진입점 구성
- `tracing`/`tracing-subscriber` 기반 bootstrap 로깅 구성
- `tokio::net::TcpListener` 기반 최소 API 서버 바인딩 (`:18000` 기본값)
- 헬스 엔드포인트 (`GET /healthz`, `GET /readyz`)
- 환경변수 기반 최소 설정 로더 (`RECORDROUTE_API_HOST`, `RECORDROUTE_API_PORT`)

### 아직 미구현/부분 구현
- 잡 API 스켈레톤 (`POST /jobs`, `GET /jobs/{id}`)
  - 현재: 인메모리 `queued` 상태 저장/조회 + `engine`별 수용량 기반 배압(`429`) 스켈레톤까지 구현
- 엔진 프로세스 관리/헬스체크/재시작
- 엔진별 큐/동시성 제어 (`stt/summarize/embed`)
- Rust `symphonia` 오디오 전처리
- Swagger 분리 프로세스 (`:14000`)

> 본 계획의 목적은 "목표 상태"를 유지하되, 문서/구현 간 간극을 단계별로 닫는 것입니다.

### 방금 반영된 단계 (Phase B-1)
- `POST /jobs?engine=<stt|summarize|embed>` 엔진 라우팅 스켈레톤 추가 (기본값 `stt`)
- 엔진별 인메모리 bounded capacity 확인 후 포화 시 `429(queue_full)` 반환
- 엔진별 큐 포화가 다른 큐 접수에 영향 주지 않는 단위 테스트 추가

### 직전 반영 단계 (Phase A+)
- `POST /jobs` → `202` + `job_id` 동작 스켈레톤 추가
- `GET /jobs/{id}` → 인메모리 저장소 조회 스켈레톤 추가
- 잡 생성 시 초기 상태는 `queued`로 고정

## 1) 목표 아키텍처 (고정)

### 1.1 단일 진입점
- 외부 트래픽은 Rust API(`:18000`)로만 진입
- Rust가 인증/검증/큐 라우팅/상태/타임아웃/배압/슈퍼비전 담당

### 1.2 엔진 경계
- `llama-text`, `llama-embed`, `whisper-server`는 독립 프로세스
- Rust↔엔진은 내부 HTTP 계약 사용 (FFI 미사용)
- 엔진 포트 고정 + `127.0.0.1` 바인딩 강제
  - `18101`: llama-text
  - `18102`: llama-embed
  - `18103`: whisper-server

### 1.3 큐/동시성
- 단일 큐 금지, 엔진별 bounded queue 필수
  - `stt_queue`, `summarize_queue`, `embed_queue`
- 큐 포화와 엔진 포화를 구분해 `429` 및 메트릭 라벨 분리

## 2) API/배포 계약

### 2.1 API 서버 (`:18000`)
- `POST /jobs` → `202` + `job_id`
- `GET /jobs/{id}` → 상태 조회
- `GET /healthz` / `GET /readyz`

잡 상태 표준:
- `queued | running | completed | failed | timeout | canceled`

### 2.2 Swagger 서버 (`:14000`, 별도 프로세스)
- Swagger UI/문서만 담당
- API 가용성과 장애 도메인 분리

## 3) 오디오 전처리 기본안 (고정)

- 기본 경로는 Rust `symphonia` 사용
- 출력 규격: 16kHz, 16-bit, mono WAV
- 길이 추출은 Rust 내부 계산으로 수행
- `whisper-server`는 추론 전담

비기본(미채택) 경로:
- 외부 `ffmpeg`/`ffprobe` 실행
- `whisper-server --convert` 의존

## 4) 단계별 구현 계획

### Phase A — Rust HTTP 최소 기동
- 최소 HTTP 서버 도입(`tokio::net::TcpListener`) ✅
- `/healthz`, `/readyz` 구현 ✅
- 설정 구조체(포트 기본값) 도입 ✅

완료 조건:
- 로컬에서 `:18000` 바인딩 확인
- readiness가 내부 의존성 초기화 상태를 반영

### Phase B — 잡 모델/상태 API
- `POST /jobs`, `GET /jobs/{id}` 골격 구현
- 인메모리 잡 저장소(초기) + 상태 전이 정의
- 에러 응답 구조 초안 고정

완료 조건:
- 최소 단위 테스트로 상태 전이 검증
- 잘못된 요청에 대해 일관된 오류 응답 제공

### Phase C — 엔진별 큐/동시성
- `stt/summarize/embed` 큐 구현
- 큐별 워커 + semaphore
- 큐 포화/엔진 포화 분리 반환 (`429`)

완료 조건:
- 한 큐 포화가 다른 큐 처리량에 영향 주지 않음
- 포화 원인별 메트릭 라벨 분리

### Phase D — 엔진 슈퍼비전
- child spawn/healthcheck/startup gating
- 종료 감지 + backoff 재시작 + 상한 정책
- graceful shutdown + 강제 종료 fallback

완료 조건:
- 엔진 비정상 종료 시 자동 복구 동작
- 상한 초과 시 degraded 상태 전환

### Phase E — 전처리/타임아웃 정책
- `symphonia` 전처리 파이프라인 도입
- 처리율 기반 timeout 계산식 적용
- HTTP timeout vs Job timeout 분리

완료 조건:
- 긴 오디오/대형 입력에서 timeout 예측가능성 확보
- timeout 원인 분류 관측 가능

### Phase F — Swagger 분리 배포/운영 문서
- Swagger 프로세스 분리 운영 구성
- 모델/벤더/manifest 운영 정책 점검
- 장애/복구/부하 런북 문서화

완료 조건:
- 문서 서버 장애가 API 서버 영향 없이 격리됨
- 운영 체크리스트로 복구 절차 재현 가능

## 5) 리스크 및 완화

- 계약 드리프트(OpenAPI vs 구현):
  - 완화: API 변경 시 OpenAPI 동시 PR 규칙
- 엔진 장애 전파:
  - 완화: readiness gating + 큐 분리 + 재시작 상한
- 처리량 변동/timeout 오탐:
  - 완화: 처리율 파라미터화 + 안전 버퍼 + 상하한 clamp

## 6) 문서/추적 링크

- 기준선: `docs/architecture.md`
- 배포/자산 정책: `docs/deployment-asset-policy.md`
- 실행 추적: `TODO/TODO.md`
