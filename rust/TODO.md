# RecordRoute Rust Rewrite TODO

이 문서는 `rust/SPEC.md`를 실제 구현 가능한 작업 단위로 쪼갠 백로그이며, 2026-03-23 기준 1차 결정사항을 반영합니다.

---

## 1차 고정 원칙
- 프론트엔드 뷰와 UX는 가능한 한 현행 유지합니다. Rust 이행의 중심은 백엔드 HTTP/WebSocket API 전환입니다.
- 1차 릴리스는 현재 제공 중인 사용자 기능을 동일하게 제공합니다.
  - 기준 범위: upload/process/progress/tasks/history/search/similar/download/models/reset 계열 API와 WebSocket 진행 이벤트
- `whisper.cpp`, `llama.cpp`는 모두 1차에서 프로세스 래퍼로 통합합니다.
  - 직접 FFI/C API 연동은 2차 최적화 과제로만 남깁니다.
- diarization은 1차 범위에서 제외합니다.
  - 단, 계약 호환을 위해 `diarize` 요청/응답 스키마는 `skipped` 중심으로 유지합니다.
- 벡터 인덱스 포맷은 현행 유지합니다.
  - `DB/vector_store/index.json`과 기존 벡터 파일 레이아웃을 그대로 읽고 씁니다.
- DB 루트 지정 방식은 기존과 동일하게 `DB_FOLDER_PATH` 환경변수를 유지합니다.
  - `DB/...` alias와 주요 JSON 포맷(`upload_history.json`, `file_registry.json`, `vector_store/index.json`)도 1차 릴리스 동안 유지합니다.
- 모델 자동 다운로드는 1차에서 제외합니다.
  - `setup.bat`와 Rust CLI는 누락 모델을 진단하고 수동 설치 경로만 안내합니다.
- 모델 저장 경로 표준안은 아래를 사용합니다.
  - 기본 루트: `MODEL_ROOT_PATH`가 없으면 `./models`
  - Whisper: `models/whisper/`
  - Llama generation: `models/llama/`
  - Embedding: `models/embedding/`
  - 기존 `WHISPER_MODEL_DIR`, `LLAMA_CPP_MODEL_PATH` 입력도 하위 호환으로 수용합니다.
- submodule은 `vendor/whisper.cpp`, `vendor/llama.cpp`에 pinned commit으로 관리합니다.
  - 업그레이드는 명시적 PR에서만 수행합니다.
  - 로컬 패치는 submodule 내부보다 상위 wrapper/build script에서 처리합니다.
- `setup.bat`에는 Rust bootstrap이 포함됩니다.
  - `git submodule update --init --recursive`
  - native build prerequisite 점검
  - `cargo build`
  - `recordroute-cli bootstrap/doctor/verify-models`

---

## 상태 정의
- `P0`: 구현 시작 전에 반드시 고정하거나 문서화해야 하는 항목
- `P1`: 초기 Rust skeleton
- `P2`: 읽기/조회 API 및 데이터 호환
- `P3`: 쓰기 API, 워크플로우, 모델 연동
- `P4`: 설치/빌드/배포 전환

---

## P0 — 계획 구체화 및 계약 고정

### 아키텍처 의사결정
- [x] Rust 웹 프레임워크 확정 (`axum`)
- [x] `whisper.cpp` 연동 방식 결정
  - 1차는 CLI 프로세스 래퍼로 고정
  - 종료 코드, stderr, timeout을 구조적으로 매핑
- [x] `llama.cpp` 연동 방식 결정
  - 1차는 CLI/server 프로세스 래퍼로 고정
  - text generation과 embedding 모두 공통 프로세스 관리 계층 사용
- [x] diarization 정책 확정
  - 1차 범위에서 제외
  - `diarize` step은 계약 호환을 위해 `skipped` 응답 전략 유지
- [x] 벡터 인덱스 1차 저장 포맷 확정
  - `DB/vector_store/index.json` 및 현행 벡터 파일 레이아웃 유지

### 운영/설치 의사결정
- [x] `setup.bat`와 Rust bootstrap CLI 책임 분리안 작성
  - `setup.bat`: toolchain 확인, submodule sync, native prerequisite 점검, `cargo build`, CLI 호출
  - `recordroute-cli`: `doctor`, `bootstrap`, `verify-models` 세부 진단/검증 제공
- [x] 모델 자동 다운로드 정책 확정
  - 1차는 수동 설치 only
  - 누락 모델은 경로와 설치 안내만 제공
- [x] 모델 저장 경로 표준화
  - 기본 루트 `./models` 또는 `MODEL_ROOT_PATH`
  - `models/whisper/`, `models/llama/`, `models/embedding/` 사용
  - 기존 경로 환경변수는 하위 호환으로 허용
- [ ] Windows 빌드 전제조건 목록 확정
  - `git`, `cargo`, `cmake`, MSVC build tools, `ffmpeg`
- [x] submodule 관리 정책 문서화
  - `vendor/whisper.cpp`, `vendor/llama.cpp` pinned commit 사용
  - 업데이트는 명시적 upgrade PR에서만 진행
  - submodule 내부 hotfix보다 상위 wrapper/build glue를 우선

### 계약/범위 확정
- [x] 1차 Rust 릴리스에 포함할 API 범위 확정
  - 목표는 현행 프론트가 사용하는 HTTP/WebSocket API 전면 대체
  - 읽기/조회, 검색, 파일 다운로드, 태스크/진행률, 모델 조회, reset/delete 계열 포함
- [x] 1차 Rust 릴리스에서 stub/미지원 처리할 범위 확정
  - HTTP 라우트 제거 없이 유지
  - diarization 실행만 1차에서 제외하고 `skipped` 중심으로 계약 호환
  - 모델 자동 다운로드는 미지원
- [x] 프론트엔드 수정 허용 범위 확정
  - `frontend/src`의 view/layout 구조는 가능한 한 유지
  - 허용되는 변경은 Rust API 계약에 맞춘 최소한의 client/config/type 조정
- [x] 기존 `DB/...` alias와 JSON 포맷 유지 시점 결정
  - 최소 1차 Rust cutover 완료 시점까지 유지
- [x] 현행 프론트 사용 API와 payload를 fixture로 동결
  - 기준 위치: `rust/fixtures/contracts/http`, `rust/fixtures/contracts/ws`, `rust/fixtures/contracts/meta/manifest.json`
  - 범위: 프론트 사용 API + 핵심 고위험 계약
- [x] diarization 제외 정책을 프론트/클라이언트 계약 테스트에 반영
  - `POST /process` fixture에 `summarize -> summary` 정규화와 `diarize.status=skipped` payload 반영
  - `/ws` error frame fixture에 `failed_step=diarize`, `error_code`, `retryable`, nested `error` 객체 반영
- [x] Rust 1차 task registry를 memory-only로 고정
- [x] 정적 서빙 전략을 direct serving + proxy compatibility로 고정
- [x] hybrid contract capture harness 추가
  - `rust/scripts/capture_contracts.py`
  - 재생성 2회 byte-stable 검증 포함
- [x] 계약 인벤토리 문서 추가
  - `rust/docs/api-contract.md`
- [x] P0 범위 밖 엔드포인트 명시
  - `/file_search`, `/similar/{uuid_or_path}`, `/api/documents/metadata`, `/cache/stats`, `/cache/cleanup`, `/metrics/workflow`

### Exit Criteria
- [x] `rust/SPEC.md`와 `rust/TODO.md`가 동일한 1차 전제를 공유
- [x] 현행 프론트 기준 API/WS 계약 fixture 확보
- [ ] “구현 시작 가능” 체크리스트에서 미정 항목이 Windows prerequisite 정도로 축소

---

## P1 — Rust workspace skeleton

### 저장소 구조
- [x] `rust/Cargo.toml` workspace 생성
- [x] crate 생성
  - [x] `recordroute-core`
  - [x] `recordroute-storage`
  - [x] `recordroute-server`
  - [x] `recordroute-workflow`
  - [x] `recordroute-models`
  - [x] `recordroute-search`
  - [x] `recordroute-cli`
- [x] 공통 lint/fmt 설정 추가 (`rustfmt.toml`, `clippy.toml` 필요 시)

### 공통 기반
- [x] typed config 로더 구현
- [x] path alias (`DB/...`) 변환 모듈 구현
- [x] `DB_FOLDER_PATH` 해석 규칙과 기본값 호환 구현
- [x] 모델 경로 해석 규칙 구현
  - `MODEL_ROOT_PATH` 기본값 `./models`
  - 기존 `WHISPER_MODEL_DIR`, `LLAMA_CPP_MODEL_PATH` 하위 호환 처리
- [x] 공통 에러 타입 및 API 에러 응답 매퍼 구현
- [x] 구조화 로그 초기화 구현
- [x] health endpoint 구현

### 테스트 기반
- [x] API contract fixture 디렉터리 설계
- [x] Python 현행 응답 캡처용 baseline fixture 생성
- [x] Rust integration test harness 생성

### Exit Criteria
- [x] `cargo test`가 최소 skeleton 수준에서 통과
- [x] `GET /health` 동작
- [x] config/path/error 기반 모듈 테스트 통과

이번 스프린트 메모:
- 첫 스프린트는 **workspace 뼈대 + config/path/health 최소 실행 경로**까지 구현했습니다.
- `recordroute-storage`, `recordroute-workflow`, `recordroute-models`, `recordroute-search`, `recordroute-cli`는 현재 책임 경계를 고정하기 위한 placeholder 수준이며, 다음 스프린트에서 실제 기능을 채웁니다.
- 두 번째 스프린트에서는 **P2 읽기 기초층**으로 `history/tasks/progress/segments/download`와 storage loader/contract test를 구현했습니다.

---

## P2 — 읽기/조회 API 및 데이터 호환

### 데이터 읽기
- [x] `upload_history.json` 읽기 구현
- [x] `file_registry.json` 읽기 구현
- [ ] `vector_store/index.json` 읽기 구현
- [ ] 현행 벡터 파일 로딩 구현
- [x] segments sidecar 읽기 구현

### 조회 API 구현
- [ ] `GET /`
- [ ] `GET /assets/*`
- [x] `GET /health`
- [x] `GET /history`
- [x] `GET /tasks`
- [x] `GET /progress/{task_id}`
- [x] `GET /segments/{file_identifier}`
- [x] `GET /download/{uuid_or_path}`
- [ ] `GET /file_search`
- [ ] `GET /search`
- [ ] `GET /api/similarity-graph`
- [ ] `GET /api/documents/metadata`
- [ ] `GET /similar/{uuid_or_path}`
- [ ] `GET /models`
- [ ] `GET /cache/stats`
- [ ] `GET /cache/cleanup`
- [ ] `GET /metrics/workflow`

### 검색/유사도
- [ ] `/search` 파라미터 정규화 구현
  - [ ] `sort_by`
  - [ ] `sort_order`
  - [ ] `status`
  - [ ] `status_task`
  - [ ] `min_score`
- [ ] keyword search 구현
- [ ] vector similarity search 구현
- [ ] 차원 불일치 문서 자동 제외 및 로그 기록 구현
- [ ] `contract_version: search-v2` 응답 유지
- [ ] `/similar` read path 구현
- [ ] similarity graph 응답의 `meta.filters`, `sampling`, `neighbor_strategy`, `incremental` 진단 유지

### 호환성 검증
- [ ] Python baseline 대비 응답 필드 비교 테스트
- [ ] 정렬/필터/페이지네이션 회귀 테스트
- [ ] 파일 없음/차원 불일치/깨진 JSON 방어 테스트
- [ ] 현행 프론트가 조회 API만으로 수정 없이 로드되는지 smoke test

### Exit Criteria
- [ ] 읽기/조회 API가 프론트 수정 없이 동작
- [ ] 검색/유사도 계약 테스트 통과

---

## P3 — 쓰기 API, 워크플로우, 모델 연동

### 업로드/파일 관리
- [ ] multipart upload 구현
- [ ] 업로드 UUID 디렉터리 생성 규칙 고정
- [ ] history/registry 원자적 갱신 구현
- [ ] destructive API safe mode 구현

### 태스크 런타임
- [ ] in-memory task registry 구현
- [ ] 취소 토큰 구현
- [ ] progress percent / ETA 계산기 구현
- [ ] WebSocket progress 스트림 구현

### 쓰기 API 구현
- [ ] `POST /upload`
- [ ] `POST /process`
- [ ] `POST /cancel`
- [ ] `POST /shutdown`
- [ ] `POST /reset`
- [ ] `POST /update_filename`
- [ ] `POST /incremental_embedding`
- [ ] `POST /check_existing_stt`
- [ ] `POST /update_stt_text`
- [ ] `POST /reset_summary_embedding`
- [ ] `POST /reset_all_tasks`
- [ ] `POST /similar`
- [ ] `POST /delete`
- [ ] `POST /delete_records`

### `/process` 구현
- [ ] payload validation 구현
- [ ] step normalization 구현
- [ ] `retry_mode`, `retry_of_task_id` 유지
- [ ] 단계별 오류 매핑 구현
- [ ] `summary`/`summarize` alias 유지

### 모델 통합
- [ ] `whisper.cpp` wrapper 구현
- [ ] `llama.cpp` text generation wrapper 구현
- [ ] `llama.cpp` embedding wrapper 구현
- [ ] 모델 timeout / stderr / exit code 매핑 구현
- [ ] 모델 미설치 오류 코드 정의
- [ ] 모델 경로 누락 시 수동 설치 안내 메시지 구현

### diarization
- [ ] `diarize` step 요청 시 `skipped` payload 반환 구현
- [ ] `results["diarize"]`, `stt_segments[].speaker`, 표준 오류 필드의 호환 규칙 문서화 및 테스트 추가

### Exit Criteria
- [ ] upload → process → progress → history → download 흐름 통합 테스트 통과
- [ ] WebSocket 진행 이벤트가 프론트와 연결 가능
- [ ] diarization 제외 상태에서도 프론트/클라이언트가 깨지지 않음

---

## P4 — 설치, 빌드, 배포 전환

### Submodule 및 native build
- [ ] `vendor/whisper.cpp` submodule 추가
- [ ] `vendor/llama.cpp` submodule 추가
- [ ] submodule 초기화 스크립트 작성
- [ ] submodule pinned commit 문서화
- [ ] CMake/native build 스크립트 작성
- [ ] Windows 빌드 검증
- [ ] Linux 빌드 검증

### 모델 검증/부트스트랩
- [ ] `recordroute-cli doctor` 구현
- [ ] `recordroute-cli verify-models` 구현
- [ ] `recordroute-cli bootstrap` 구현
  - [ ] submodule 상태 확인
  - [ ] native build 산출물 확인
  - [ ] `cargo build` 실행 또는 결과 확인
  - [ ] 필수 모델 존재 여부 확인
- [ ] 모델 자동 다운로드 명령은 1차 범위에서 제외한다고 명시
- [ ] 누락 모델 안내 메시지 UX 작성
- [ ] `setup.bat`에서 Rust CLI 호출 통합 설계

### 패키징
- [ ] release binary 산출물 구조 정의
- [ ] Dockerfile 초안 작성
- [ ] 환경 변수 문서화
- [ ] README 전환 계획 작성

### Cutover
- [ ] Python backend와 동등성 최종 점검
- [ ] legacy Python 코드 동결 브랜치/태그 정책 결정
- [ ] 메인 실행 경로를 Rust로 전환
- [ ] Python 의존성 제거 계획 실행

### Exit Criteria
- [ ] 신규 설치가 Python 없이 완료
- [ ] `setup.bat`가 Rust bootstrap + 모델 검증 흐름을 제공
- [ ] release artifact와 Docker 이미지 생성 가능

---

## P5 — Legacy 정리 및 저장소 슬림화

### 정리 대상 인벤토리
- [ ] Rust cutover 이후 미사용 자산 목록 작성
  - [ ] Python backend 코드 (`sttEngine/*`, Python entrypoint, provider glue)
  - [ ] Python 테스트 코드 (`tests/*`, Python 회귀 fixture 중 Rust 검증에 더 이상 필요 없는 항목)
  - [ ] 프론트엔드 예제/실험 코드 중 현행 UI에서 더 이상 참조하지 않는 항목
  - [ ] 초기 계획 단계에서 폐기된 폴더/스크립트
  - [ ] Rust 기준으로 미사용인 문서와 TODO
  - [ ] Python 전용 Dockerfile/compose profile/requirements/setup 스크립트
- [ ] 삭제와 archive 대상을 구분하는 기준 문서화
  - [ ] 운영 fallback으로 잠시 보존할 항목
  - [ ] fixture/비교 기준으로 archive할 항목
  - [ ] 즉시 삭제 가능한 항목

### 코드/테스트 정리
- [ ] Python backend 제거 또는 `legacy/` archive 경로로 이동
- [ ] Python 테스트 코드 정리
  - [ ] Rust 계약 검증에 계속 필요한 fixture만 보존
  - [ ] 더 이상 실행하지 않는 pytest suite 제거
  - [ ] Python 전용 benchmark/utility 스크립트 제거 또는 archive
- [ ] 프론트엔드 예제/legacy/fallback 코드 정리
  - [ ] Rust cutover 이후에도 필요한 fallback만 남김
  - [ ] 빌드/배포 경로에서 쓰이지 않는 예제/샘플 제거

### 배포/스크립트 정리
- [ ] Python 전용 설치 스크립트 정리
  - [ ] `setup.sh`, `setup.bat`, `run.sh`, `run.bat`의 Python 전제 제거
  - [ ] `requirements*.txt`, venv 안내, Python launcher 설명 제거
- [ ] Docker 자산 정리
  - [ ] Rust 기준으로 더 이상 쓰지 않는 `Dockerfile.backend`, Python compose 설정, legacy image 경로 제거
  - [ ] Rust runtime 기준 Dockerfile/compose만 남기고 문서 갱신
- [ ] CI 스크립트 정리
  - [ ] pytest/job 제거
  - [ ] cargo 기반 검증만 남기도록 workflow 정리

### 문서 정리
- [ ] Rust cutover 이후 기준 문서 재편
  - [ ] `README.md`를 Rust 실행/설치 기준으로 전환
  - [ ] 루트 `AGENTS.md`의 현재 구조/실행 명령/테스트 항목을 Rust 기준으로 갱신
  - [ ] `rust/SPEC.md`, `rust/TODO.md`에서 완료된 migration 문맥 정리
- [ ] 미사용 문서 정리
  - [ ] Python 전용 운영 문서 제거 또는 `docs/legacy/` archive
  - [ ] 초기 migration 초안, 폐기된 설계 메모, obsolete TODO 정리
  - [ ] Rust 기준으로 더 이상 유지하지 않는 docs 링크 제거

### 최종 검증
- [ ] 저장소 전체에서 Python backend/pytest/venv 기준 안내가 남아 있지 않은지 검사
- [ ] 프론트엔드, Rust 서버, Docker, 문서가 동일한 실행 경로를 설명하는지 교차 검토
- [ ] archive 대상은 이유와 복구 경로를 남기고, 삭제 대상은 changelog 수준으로 기록

### Exit Criteria
- [ ] Rust 운영에 필요 없는 Python 코드/테스트/스크립트/문서가 정리됨
- [ ] README, AGENTS, Docker, CI가 Rust 기준 단일 실행 경로를 가리킴
- [ ] 비교용 legacy 자산은 명시적 archive 위치로만 남고, 기본 개발 경로에서는 보이지 않음

---

## 검증 체크리스트

### 계획이 충분히 구체적인가?
- [ ] crate 책임이 모두 설명 가능한가?
- [ ] 최초 릴리스 범위가 API 단위로 닫혀 있는가?
- [ ] 모델 파일이 없을 때 UX가 정의되어 있는가?
- [ ] submodule 업데이트/빌드 실패 시 대응이 정의되어 있는가?
- [ ] 프론트엔드와의 계약 검증 방법이 정해져 있는가?
- [ ] Windows 기준 설치 순서가 문서로 재현 가능한가?

### 아직 부족하다고 판단하는 신호
- [ ] “일단 Rust로 옮기면 된다” 수준의 추상 문장이 남아 있다
- [ ] 모델 배포 정책이 사람 구두 합의에만 의존한다
- [ ] 현행 프론트가 실제로 호출하는 API fixture가 비어 있다
- [ ] `setup.bat`와 Rust CLI의 역할이 중복된다
- [ ] 어떤 legacy 자산을 삭제/보존/archive할지 기준이 없다

위 부족 신호가 하나라도 남아 있으면, 구현보다 먼저 설계를 보강합니다.
