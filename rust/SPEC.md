# RecordRoute Rust Rewrite Specification

이 문서는 **현재 Python 백엔드를 제거하고 Rust 기반 런타임으로 재구성**하기 위한 초기 사양서입니다. 목표는 지금 바로 구현을 시작하는 것이 아니라, **계획이 충분히 구체적인지 검증할 수 있는 수준의 설계 기준선**을 만드는 것입니다.

---

## 1. 목표와 비목표

### 1.1 목표
- Python 백엔드(`sttEngine/*`)를 단계적으로 걷어내고 Rust 서비스로 대체한다.
- 기존 프론트엔드(`frontend/src/*`)의 view/UX는 가능한 한 유지한 채, **현행 HTTP/WebSocket API를 Rust 기준으로 전면 대체**한다.
- STT는 `whisper.cpp`, LLM/Embedding은 `llama.cpp`를 사용하며, 1차에서는 둘 다 **프로세스 래퍼**로 통합한다.
- `whisper.cpp`, `llama.cpp`는 저장소에 **git submodule**로 포함하고 pinned commit 기준으로 관리한다.
- 1차 릴리스는 현재 사용자 기능을 동일하게 제공하되, diarization 실행은 제외하고 계약 호환용 `skipped` 동작만 유지한다.
- 최초 실행/설치 시점에 모델 파일 존재 여부를 검증하고, 누락 시 **수동 설치 경로와 예상 파일명**을 안내한다.
- 운영 관점에서 Python 의존성 없이 빌드/배포/실행 가능한 Windows 우선 경로를 확보한다.

### 1.2 비목표
- 현재 프론트엔드 UX를 동시에 전면 재설계하지 않는다.
- 1차 목표에서 STT/LLM 품질 자체를 바꾸는 것이 목적은 아니다.
- 기존 데이터 포맷(`DB/...` alias, history/registry/index JSON)을 초기에 모두 폐기하지 않는다.
- 1차 범위에 diarization 실제 실행을 포함하지 않는다.
- 1차 범위에 모델 자동 다운로드를 포함하지 않는다.
- 초기에 새로운 분산 시스템이나 복잡한 외부 큐를 도입하지 않는다.

---

## 2. 현재 코드베이스 분석 요약

현행 시스템은 다음 특성을 가진다.

### 2.1 백엔드 구조
- 진입점은 `sttEngine/server.py`이며 실제 서버는 `sttEngine/http_api/app.py`의 `ThreadingHTTPServer` 기반 구현이다.
- HTTP 핸들링은 `sttEngine/http_api/handler.py`와 `sttEngine/http_api/routes/*`에 분산되어 있다.
- `/process`는 `sttEngine/server/routes/process.py` → `sttEngine/server/services/file_service.py` → `sttEngine/server/tasks/queue.py` → `sttEngine/http_api/workflow.py` 순으로 흘러간다.
- WebSocket 진행 이벤트는 `sttEngine/http_api/ws.py`에서 별도 스레드로 기동된다.

### 2.2 핵심 상태/데이터
- 업로드 히스토리: `DB/upload_history.json`
- 파일 레지스트리: `DB/file_registry.json`
- 벡터 인덱스 메타: `DB/vector_store/index.json`
- 파일 경로는 절대경로 대신 `DB/...` alias 중심으로 저장/교환된다.

### 2.3 병목/재작성 동기
- 서버가 `ThreadingHTTPServer` 기반이라 고동시성/관측성/구조화된 shutdown 제어가 약하다.
- 워크플로우는 파일 I/O, 외부 실행, JSON 상태 갱신이 느슨하게 결합돼 있다.
- 검색은 Python 루프와 개별 벡터 파일 로딩(`np.load`) 중심이라 대규모 데이터셋에서 비효율 가능성이 높다.
- llama.cpp 경로는 이미 존재하지만 Python 런타임과 강하게 얽혀 있고, 설치/실행/모델 검증 절차가 분산돼 있다.

---

## 3. 대상 아키텍처

## 3.1 최상위 디렉터리 제안
```text
rust/
├── AGENTS.md
├── SPEC.md
├── TODO.md
├── fixtures/
│   └── contracts/
│       ├── http/
│       ├── ws/
│       └── meta/
├── scripts/
│   └── capture_contracts.py
└── docs/
    ├── api-contract.md
    ├── storage-layout.md
    └── migration-plan.md
```

> 참고: 위 트리는 **P0 기준 실제 존재/우선 생성 대상**을 먼저 반영합니다. `Cargo.toml`, crates, vendor submodule, bootstrap 스크립트는 이번 단계 범위 밖이며 P1 이후에 추가합니다.

### 3.1.1 P1 skeleton 반영 상태 (2026-03-23)
현재 `rust/`에는 아래 workspace/crate skeleton이 추가되었습니다.

```text
rust/
├── Cargo.toml
├── rustfmt.toml
├── clippy.toml
└── crates/
    ├── recordroute-core/
    ├── recordroute-storage/
    ├── recordroute-server/
    ├── recordroute-workflow/
    ├── recordroute-models/
    ├── recordroute-search/
    └── recordroute-cli/
```

crate 책임 경계 초안:
- `recordroute-core`: typed config, path alias, 공통 에러/기초 타입
- `recordroute-storage`: DB JSON/sidecar 파일 접근 계층
- `recordroute-server`: axum app, router, middleware, API 계약 테스트 진입점
- `recordroute-workflow`: `/process` 파이프라인 orchestration
- `recordroute-models`: `whisper.cpp`/`llama.cpp` wrapper와 모델 검증
- `recordroute-search`: search/history/similarity 조회 로직
- `recordroute-cli`: `doctor`, `bootstrap`, `verify-models` 같은 운영용 CLI

현재 구현 범위는 `recordroute-core`의 config/path 기초, `recordroute-server`의 `GET /health`, 그리고 최소 integration test harness까지다. 다른 crate는 의존 경계만 고정한 placeholder 상태로 유지한다.

### 3.2 런타임 구성
- **API 서버**: `axum` 권장
  - 이유: Tokio 생태계와의 결합, Tower middleware, WebSocket 지원, 타입 안전성.
- **비동기 런타임**: `tokio`
- **직렬화**: `serde`, `serde_json`
- **에러 모델**: `thiserror`, `anyhow`는 내부 도구성 계층에 한정
- **관측성**: `tracing`, `tracing-subscriber`, OpenTelemetry 확장 가능 구조
- **설정**: `.env` + 환경변수 + typed config loader

### 3.3 외부 모델 연동 방식
- `whisper.cpp`: **submodule 빌드 + CLI 프로세스 래퍼**로 통합한다.
- `llama.cpp`: **submodule 빌드 + server/CLI 프로세스 래퍼**로 통합한다.
- text generation과 embedding은 동일한 프로세스 실행/감시 계층에서 관리한다.
- 1차 구현에서는 FFI/C API 직접 연동을 시도하지 않는다. 안정적인 빌드/배포가 우선이다.

### 3.4 권장 초안
- **1차**: submodule을 CMake로 빌드하고 Rust에서는 프로세스 래퍼로 통합
- **1차**: `setup.bat`와 `recordroute-cli`는 모델 존재 여부를 검증만 하고 자동 다운로드는 수행하지 않음
- **2차**: 성능이 정말 필요할 때만 특정 경로를 FFI 또는 C API 직접 연동으로 대체

이유:
- 빌드 복잡도와 플랫폼 차이를 초기부터 과도하게 키우지 않기 위함
- 먼저 계약/운영/모델 검증 흐름을 안정화하는 편이 위험이 낮음

---

## 4. 기능 요구사항

## 4.1 API 호환 범위
Rust 1차 버전은 아래 엔드포인트를 우선 지원해야 한다.

### 읽기/조회
- `GET /`
- `GET /assets/*`
- `GET /health`
- `GET /history`
- `GET /tasks`
- `GET /progress/{task_id}`
- `GET /segments/{file_identifier}`
- `GET /download/{uuid_or_path}`
- `GET /search`
- `GET /api/similarity-graph`
- `GET /models`

### 쓰기/작업
- `POST /upload`
- `POST /process`
- `POST /cancel`
- `POST /shutdown`
- `POST /reset`
- `POST /update_filename`
- `POST /incremental_embedding`
- `POST /check_existing_stt`
- `POST /update_stt_text`
- `POST /reset_summary_embedding`
- `POST /reset_all_tasks`
- `POST /similar`
- `POST /delete`
- `POST /delete_records`

### 실시간
- WebSocket 진행 이벤트 (`ws://.../ws` 또는 현행과 호환되는 경로)

### 4.1.1 P0 fixture 동결 범위
이번 단계에서 실제 fixture로 동결하는 범위는 다음과 같다.
- GET: `/history`, `/tasks`, `/progress/{task_id}`, `/segments/{file_identifier}`, `/download/{uuid_or_path}`, `/search`, `/api/similarity-graph`, `/models`
- POST: `/upload`, `/process`, `/cancel`, `/reset`, `/reset_all_tasks`, `/update_filename`, `/update_stt_text`, `/check_existing_stt`, `/reset_summary_embedding`, `/similar`, `/delete`, `/delete_records`, `/shutdown`
- WebSocket: `/ws` progress frame

이번 단계에서 fixture를 만들지 않는 P0 범위 밖 엔드포인트:
- `GET /file_search`
- `GET /similar/{uuid_or_path}`
- `GET /api/documents/metadata`
- `GET /cache/stats`
- `GET /cache/cleanup`
- `GET /metrics/workflow`

## 4.2 계약 호환 규칙
다음 계약은 **초기 Rust 이행의 고정 조건**으로 본다.
- `/process`의 `steps`는 소문자/중복 제거 정규화 유지
- `summarize` → `summary` alias 유지
- `diarize` 요청은 받을 수 있지만 1차에서는 실행하지 않고 계약 호환용 `skipped` 응답을 반환
- 표준 오류 필드 유지: `error`, `error_code`, `retryable`, `failed_step`
- STT 세그먼트 스키마 유지: `{start, end, text, speaker}`
- `/search`의 `contract_version: search-v2` 유지
- `DB/...` 경로 alias 계약 유지
- `DB_FOLDER_PATH` 환경변수 우선 해석 규칙 유지
- 파괴적 API safe mode와 token/session 보호 규칙 유지
- Rust 1차 task registry는 **memory-only**로 유지
- `/` 및 `/assets/*` 정적 서빙은 **Rust 서버 직접 서빙 + 프록시 호환**을 기본 계약으로 유지
- fixture 저장 전 generated id, timestamp, localhost port, temp path, OS 절대경로, 동적 URL 조각은 정규화

## 4.3 계약 fixture 자산
P0 계약 동결 기준물은 아래 경로를 단일 진실 공급원으로 사용한다.
- `rust/fixtures/contracts/http/*.json`
- `rust/fixtures/contracts/ws/*.json`
- `rust/fixtures/contracts/meta/manifest.json`
- `rust/docs/api-contract.md`
- `rust/scripts/capture_contracts.py`

규칙:
- HTTP fixture는 케이스별 JSON 1파일이며 `method`, `path`, `query`, `headers`, `body`, `expected_status`, `expected_body`, `normalization_rules`, `invariants`를 유지한다.
- WebSocket fixture는 ordered `frames[]` 배열을 사용하며 각 frame은 `task_id`, `message`, `stage`, `error_code`, `retryable`, `failed_step`, `progress_percent`, `eta_seconds`, `error` 필드를 보존한다.
- capture 방식은 **hybrid**다.
  - ephemeral HTTP/WS 서버로 재현 가능한 케이스는 server capture
  - `models`, destructive safe-mode guard처럼 논리 중심 케이스는 handler/unit-test 패턴 재사용
- fixture drift는 사양 변경으로 간주하며, 의도적이면 fixture와 `rust/SPEC.md`, `rust/TODO.md`를 같은 변경에서 함께 갱신한다.

---

## 5. 데이터/스토리지 사양

### 5.1 유지할 포맷
초기 Rust 버전은 아래 파일 포맷을 그대로 읽고 쓸 수 있어야 한다.
- `DB/upload_history.json`
- `DB/file_registry.json`
- `DB/vector_store/index.json`
- 현행 벡터 파일 레이아웃
- 결과물 markdown/text/segments sidecar 파일

### 5.2 스토리지 계층 원칙
- DB 루트 해석은 `DB_FOLDER_PATH` 환경변수 우선, 미지정 시 프로젝트 루트 `DB/`를 사용한다.
- 파일 경로 변환은 단일 모듈에서만 수행한다.
- `DB/...` alias ↔ OS 절대경로 변환 함수를 중앙화한다.
- 레지스트리/히스토리 파일 쓰기는 원자적 갱신 방식을 사용한다.
  - 권장: temp file write 후 rename
- 동시성 충돌을 고려해 파일 잠금 전략을 명시한다.
  - Windows 우선 검증 필요

### 5.3 향후 확장
- 장기적으로는 SQLite 또는 RocksDB 기반 메타스토어로 전환 가능하지만, 1차 마이그레이션 범위에서는 JSON 파일 호환을 우선한다.

---

## 6. 워크플로우 사양

## 6.1 표준 파이프라인
`/process`는 다음 스텝 조합을 지원해야 한다.
- `stt`
- `correct`
- `summary`
- `embedding`
- `diarize` 요청 수용
  - 1차에서는 실행하지 않고 `skipped` 응답으로 호환 유지

### 6.2 오케스트레이션 요구사항
- task_id 기준 진행 상태 저장
- 취소 요청 수용
- 단계별 progress percent와 ETA 제공
- 중간 산출물 파일 저장
- 실패 시 표준 오류 payload 생성
- 재시도 메타(`retry_mode`, `retry_of_task_id`) 유지

### 6.3 STT
- 입력 오디오를 `whisper.cpp`에 전달한다.
- 모델 파일 존재 여부를 실행 전에 검증한다.
- 출력은 markdown/text + `*.segments.json` sidecar를 생성한다.
- diarization이 실패해도 STT 텍스트는 유지해야 한다.

### 6.4 Correct/Summary/Embedding
- `llama.cpp` 기반으로 통합한다.
- 교정과 요약은 채팅/프롬프트 실행 계층을 공유한다.
- 임베딩은 동일 엔진 기반이더라도 별도 모델/파라미터를 가질 수 있어야 한다.
- 모델 미설치/모델 불일치/컨텍스트 초과 등은 표준 오류 코드로 변환한다.

### 6.5 Diarization
- diarization 실제 실행은 Rust 1차 범위에서 제외한다.
- `diarize` step이 요청되면 Rust 서버는 계약 호환용 `skipped` 상태를 반환한다.
- `results["diarize"]`, `stt_segments[].speaker`, 관련 오류/상태 필드는 현행 프론트가 깨지지 않도록 응답 스키마를 유지해야 한다.

---

## 7. 모델 및 설치 검증 사양

## 7.1 모델 검증이 필요한 이유
사용자 요구사항상, 최초 실행 시점에 `whisper`와 `llama`용 모델 파일이 설치됐는지 검증하는 단계가 필요하다. 현재는 이 역할이 `setup.bat`와 `recordroute-cli`에 들어간다고 보면 된다. 1차에서는 누락 모델을 자동으로 내려받지 않고, 수동 설치 안내만 제공한다.

### 7.2 검증 대상
- 기본 모델 루트는 `MODEL_ROOT_PATH`가 없으면 `./models`를 사용한다.
- Whisper 모델 파일
  - 예: `models/whisper/ggml-*.bin`
- Llama 모델 파일
  - 예: `models/llama/*.gguf`
- 필요 시 임베딩 전용 GGUF 모델
  - 예: `models/embedding/*.gguf`
- 선택적으로 tokenizer/template/config 메타파일
- 기존 `WHISPER_MODEL_DIR`, `LLAMA_CPP_MODEL_PATH`는 하위 호환 입력으로 허용한다.

### 7.3 검증 동작
`setup.bat` 또는 Rust bootstrap CLI는 다음을 수행해야 한다.
1. Rust 바이너리/런타임 준비 여부 확인
2. submodule checkout 상태 확인
3. `whisper.cpp`, `llama.cpp` 빌드 산출물 존재 여부 확인
4. 필수 모델 파일 존재 여부 확인
5. 누락 시
   - 다운로드 URL/사내 저장소/수동 설치 경로 안내
   - 자동 다운로드는 수행하지 않음
6. 검증 결과를 사람이 읽기 쉬운 표와 종료 코드로 제공

### 7.4 권장 명령 구조
- `recordroute-cli doctor`
- `recordroute-cli verify-models`
- `recordroute-cli bootstrap`

### 7.5 setup.bat 역할 재정의
현재 Python venv 중심인 `setup.bat`는 Rust 전환 이후 아래 역할로 바뀐다.
- Rust toolchain 확인 (`cargo`, `rustc`)
- git submodule 초기화/업데이트
- native dependency 확인 (`cmake`, C/C++ compiler, ffmpeg`)
- `cargo build` 또는 `cargo build --release`
- `recordroute-cli bootstrap`/`doctor`/`verify-models` 호출
- 모델 검증 및 수동 설치 안내
- 환경파일 템플릿 생성

---

## 8. 빌드/배포 사양

### 8.1 로컬 개발
- `git submodule update --init --recursive`
- `cargo build`
- `cargo run -p recordroute-server`

### 8.2 CI
- Linux, Windows 우선 지원
- 캐시 대상
  - Cargo target
  - submodule build output
  - model checksum manifest
- 빌드 파이프라인 단계
  1. checkout + submodule sync
  2. Rust fmt/clippy/test
  3. native libs build
  4. integration test
  5. release artifact packaging

### 8.3 Docker
- multi-stage Dockerfile 권장
- builder stage에서 Rust + submodule native build
- runtime stage는 바이너리 + 필요한 native artifact + models mount만 포함

---

## 9. 구체 설계가 아직 필요한 쟁점

이 섹션은 “계획이 충분히 구체적인가?”를 검증하는 핵심 체크리스트다.

### 9.1 아직 결정해야 하는 것
- Windows에서 native dependency 설치 가이드를 어느 수준까지 자동화할지
- 모델 존재 검증을 단순 파일 존재로 끝낼지, 해시/manifest 검증까지 포함할지

### 9.2 계획이 충분히 구체적이라고 볼 수 있는 기준
다음 질문에 모두 답할 수 있으면 “구현 시작 가능” 상태다.
- 어떤 crate가 어떤 책임을 갖는가?
- 어떤 API부터 구현하고 어떤 payload를 fixture로 고정하는가?
- 기존 JSON 파일과 path alias를 어떻게 유지하는가?
- 모델 파일이 없을 때 사용자에게 어떤 UX를 제공하는가?
- setup.bat와 Rust bootstrap CLI의 책임 경계는 무엇인가?
- 실패/취소/진행 상태를 어디에 저장하는가?
- llama.cpp/whisper.cpp 빌드 실패 시 오류 메시지와 복구 가이드는 무엇인가?
- 프론트엔드와 계약 검증은 어떤 테스트로 보장하는가?

만약 위 항목 중 2개 이상이 비어 있으면, 아직은 “막연한 방향성” 단계로 간주한다.

---

## 10. 권장 마이그레이션 단계

### Phase 0 — 계약 고정
- 현행 Python API/파일 포맷을 fixture와 문서로 고정
- `rust/fixtures/contracts/` + `rust/docs/api-contract.md` + `rust/scripts/capture_contracts.py`를 단일 기준으로 유지
- 특히 `/process`, `/search`, `/progress`, `/models`, `destructive safe mode`, `/ws` progress frame 계약을 스냅샷화

### Phase 1 — Rust skeleton
- `rust/` workspace 생성
- core/config/storage/path/api-error 모델 구현
- `GET /health`, `GET /history`, `GET /progress/:id`부터 시작

### Phase 2 — Search/Read API
- `/search`, `/similar`, `/download`, `/segments` 구현
- 기존 DB 포맷 호환 검증

### Phase 3 — Upload/Workflow
- `/upload`, `/process`, `/cancel` 구현
- task manager, WebSocket progress 구현

### Phase 4 — Model runtime integration
- submodule build 고정
- whisper.cpp/llama.cpp wrapper 안정화
- setup/bootstrap/model verification 완료

### Phase 5 — Cutover
- Python backend 제거 또는 legacy 브랜치 분리
- README/운영 문서 갱신
- Docker/Windows 실행 경로 전환

---

## 11. 테스트 전략

### 11.1 필수 테스트 층
- 단위 테스트
  - path alias 변환
  - process payload 정규화
  - error mapping
- 계약 테스트
  - 현행 API 응답 필드 호환
- 통합 테스트
  - 업로드 → 처리 → 조회 → 다운로드
- 회귀 테스트
  - 기존 fixture 데이터 기반 search/history/similar 동작
- 설치 테스트
  - submodule 없음 / 모델 없음 / ffmpeg 없음 / compiler 없음

### 11.2 검증 우선순위
1. API 계약 불변
2. 파일 포맷 불변
3. setup/bootstrap UX 명확성
4. 모델 실행 안정성
5. 성능 최적화

---

## 12. 현재 계획에 대한 평가

현재 방향성은 **P0 계약 동결 기준이 생긴 상태**이며, 다음 단계는 이 fixture를 기준으로 Rust workspace와 서버 skeleton을 시작하는 것이다.

### 이미 좋은 점
- Python 제거라는 목표가 분명하다.
- 모델 엔진이 `whisper.cpp` / `llama.cpp`로 고정돼 있어 의사결정 폭이 줄었다.
- submodule 포함, 프로세스 래퍼 전략, 수동 모델 검증 정책이 명확하다.

### 아직 더 구체화가 필요한 점
- `setup.bat`의 Windows prerequisite 점검 범위를 실제 스크립트 수준으로 내려야 한다.
- 모델 검증을 단순 존재 확인에서 어디까지 강화할지 결정해야 한다.
- release packaging 구조를 P4 수준까지 세분화해야 한다.

### 결론
이 문서 기준으로는 **“1차 원칙 확정 + P0 계약 동결 완료”** 상태다. 다음 단계는 추측이 아니라 `rust/fixtures/contracts/`와 `rust/docs/api-contract.md`를 기준으로 Rust workspace와 API skeleton을 여는 것이다.
