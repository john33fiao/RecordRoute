# RecordRoute

RecordRoute는 음성/문서 처리 파이프라인을 **Rust 중심 아키텍처**로 전환 중인 프로젝트입니다.

현재 저장소는 아래 두 영역으로 나뉩니다.

- `frontend/`: 현재 유지 중인 웹 프론트엔드
- `docs/`: Rust 전환/설계 문서

Rust 백엔드는 `/healthz`/`/readyz` + `POST /jobs`/`GET /jobs/{id}`와 엔진별 bounded queue/worker 기반 처리 흐름(queued→running→completed|failed|timeout|canceled|rejected)까지 반영되어 있으며, 엔진 슈퍼비전/전처리/Swagger 분리 배포 구성을 단계적으로 이전합니다.


> 문서 동기화: 2026-02-23 기준 운영 안정화(고정 worker 동시성 상한, connect+request timeout 관철, job_id 검증/로그 위생, queue depth guard) 반영 상태와 정렬됨.

## 운영 안정화 메모 (Phase B-2)

- 동시성 상한은 semaphore 대기 태스크 누적 대신 **고정 worker 개수**로 강제합니다.
- bounded queue 백프레셔를 유지하여 과부하 시 `POST /jobs`가 `429(queue_full)`로 떨어지도록 합니다.
- 엔진 HTTP 호출은 `connect_timeout` + read/write timeout을 적용해 connect 지연과 응답 지연 모두 시간 상한 내 실패합니다.
- `job_id`는 `[a-zA-Z0-9_-]`, 1..64 규칙으로 검증되며 invalid 입력은 `400 invalid_job_id`로 응답합니다.
- queue depth는 **근사 지표(accepted enqueue 기준)**로 정의하고 RAII guard Drop으로 감소 정합성을 보장합니다.

오디오 전처리/변환은 기존 `ffmpeg` 실행 방식 대신 Rust `symphonia` 크레이트 기반 구현을 목표 기준으로 문서화합니다.

## RTD(배포준비) 점검 규칙

- 코드 작업 시작 전 RTD Step 1~4(계획/검토/과도성 제거)를 수행하고 단계별 PASS/FAIL + 핵심 근거를 기록합니다.
- 구현 완료 후 커밋 전 RTD Step 5~18을 순서대로 수행합니다. FAIL 단계는 원인 해결 후 해당 단계부터 재수행합니다.
- 보안 리스크 또는 롤백 경로가 불명확하면 PASS할 수 없으며, Step 1~18 전체 PASS(READY)일 때만 커밋/PR을 진행합니다.

## 문서 우선순위

1. 아키텍처/작업 규칙: `AGENTS.md`
2. 사용자/운영 개요: `README.md` (이 문서)
3. 아키텍처 기준선: `docs/architecture.md`
4. 전환 설계: `docs/rust-cpp-backend-rewrite-plan.md`
5. 배포/자산 정책: `docs/deployment-asset-policy.md`
6. 전환 실행 WBS: `TODO/TODO.md`
7. 에이전트 요약: `CLAUDE.md`, `GEMINI.md`

## 현재 상태 (2026-02 기준)

- Python 기반 구 구현은 현재 저장소에 포함되어 있지 않으며, 필요 시 별도 레거시 보관소를 참조합니다.
- Rust 오케스트레이터 + C++(llama.cpp/whisper.cpp) 엔진 분리 아키텍처를 목표로 합니다.
- 프론트엔드는 유지하되, 향후 Rust API 계약에 맞춰 점진적으로 연결합니다.

## 개발 시작

### 프론트엔드 실행

```bash
cd frontend
npm install
npm run dev
```

### 프론트엔드 빌드

```bash
cd frontend
npm run build
```

## Rust 전환 가이드

- 단일 진입점/엔진 경계 기준은 `docs/architecture.md`를 먼저 확인합니다.
- 상세 목표/포트/큐/타임아웃/슈퍼비전 정책은 `docs/rust-cpp-backend-rewrite-plan.md`를 단일 기준으로 따릅니다.
- 실행 단위 일정/의존성 관리는 `TODO/TODO.md`를 기준으로 추적합니다.
- 전환 우선순위는 **read-heavy API 및 검색 경로 최적화**를 먼저 수행하고, `/process` 전체 전환은 Go/No-Go 판단 이후 진행합니다.
- 기존 Python 동작과의 계약 호환(응답 필드/에러 규약/정렬/페이징)은 반드시 테스트로 고정합니다.

## 레거시 코드 다룰 때

- 현재 저장소에는 `deprecated/` 디렉터리가 없으므로 레거시 코드는 기본 작업 범위가 아닙니다.
- 레거시 이슈 재현이 필요하면 별도 레거시 보관소/브랜치에서 수행하고, 결과만 본 저장소 문서에 반영합니다.

## 주의

- 이 저장소 루트에는 Rust 실행 코드(`Cargo.toml`)가 존재합니다.
- 따라서 Rust 코드 변경 시 `cargo test`, `cargo clippy`를 포함한 검증을 수행해야 합니다.


Swagger 문서 서버는 `scripts/run-swagger.sh`로 `:14000`에서 분리 실행할 수 있습니다.
