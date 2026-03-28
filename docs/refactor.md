# Rust 리팩터링 현황 (2026-03-28 코드 기준)

이 문서는 현재 코드베이스를 기준으로 리팩터링 상태를 다시 판정한 결과를 적는다.
핵심 목적은 두 가지다.

- 이미 끝난 축을 backlog에서 빼기
- 아직 남은 축만 보고 다음 작업을 이어갈 수 있게 하기

즉 이 문서는 과거 작업 기록보다 "지금 무엇을 다시 손댈 필요가 있는가"에 초점을 둔다.

## 1. 판정 기준

- 완료
  - 구조 분리가 이미 코드 경로와 테스트에 반영돼 있어, 같은 축의 대형 재정리를 다시 우선순위에 올릴 필요가 없는 상태
- 미완료
  - 구조적 부채가 남아 있어 후속 슬라이스로 바로 작업화할 수 있는 상태
- 기준 소스
  - `rust/src/**`
  - `docs/architecture.md`
  - `docs/openapi.yaml`
- 우선순위 표기
  - `P1`: 다음 슬라이스로 바로 잡아야 하는 항목
  - `P2`: P1 이후 이어서 정리할 구조 부채
  - `P3`: 기능상 막히지는 않지만 계속 드리프트를 만드는 항목

## 2. 완료된 항목

이 절의 항목은 현재 기준으로 "다시 같은 주제로 대형 리팩터링을 제안하지 않아도 되는 영역"이다.

### 2.1 앱/서버/인덱스 대형 모듈 분해는 완료됐다

- `app/*`는 `ffmpeg_stage`, `stt_stage`, `summary_stage`, `embedding_stage`, `models`, `queue`로 나뉘어 있다.
- `server/*`는 `routes/jobs|models|queue|stages|web`, `errors`, `types`, `upload`, `app_api`로 분리돼 있다.
- `index/*`는 `backend`, `store`, `types`, `sqlite`, `postgres`로 나뉘어 있다.

현재 우선순위는 "파일을 더 쪼갤까"가 아니라, 남은 교차 관심사를 어떻게 줄일까 쪽이다.

### 2.2 메타 저장소의 DB 전환은 완료됐다

- 기본 메타 저장소는 `db/index.sqlite3`다.
- `StorageConfig`를 통해 PostgreSQL backend도 지원한다.
- `db/index.json`은 더 이상 권위 저장소가 아니며, legacy JSON을 무시하는 현재 동작을 테스트로 검증한다.

즉 과거의 "JSON 인덱스 파일 구조를 더 다듬을 것인가"는 이제 주요 backlog가 아니다.

### 2.3 오디오 저장소 분리는 완료됐다

- `AudioStore`가 source publish, cache, spool, job artifact 경로를 전담한다.
- 입력 source는 content-addressed storage key로 publish된다.
- 재사용과 deduplicate는 경로가 아니라 source hash 기준으로 동작한다.

메타데이터 저장과 오디오 파일 저장을 분리하는 큰 방향은 이미 정착된 상태다.

### 2.4 Queue 영속화와 scheduler 분리는 완료됐다

- queue 책임이 `app/queue.rs` facade 아래 `entries`, `executor`, `planner`, `scheduler`로 나뉘어 있다.
- `TaskQueueState`, `ActiveQueueBatch`, `burst_limit`이 메타 저장소에 영속화된다.
- batch merge, burst rotation, interrupted active entry recovery까지 코드와 테스트가 있다.

즉 "단일 함수에서 큐를 돌리는 구조"는 이미 지난 단계다.

### 2.5 Summary embedding 파이프라인 내장은 완료됐다

- summary 이후 embedding 제출/실행/backfill/search 경로가 `app/embedding_stage.rs`로 독립돼 있다.
- summary 본문과 embedding metadata/vector 저장 경로가 분리돼 있다.
- `/jobs/{job_id}/summary/embedding`, `/summary/search`까지 API 표면에 반영돼 있다.

embedding은 이제 별도 실험 코드가 아니라 정식 stage로 보는 편이 맞다.

### 2.6 API 표면과 테스트 분리는 완료됐다

- `server.rs`는 라우팅 조립만 맡고, 실제 핸들러는 route 파일로 분리돼 있다.
- app/server 테스트는 각각 `rust/src/app/tests/*`, `rust/src/server/tests/*`로 분리돼 있다.
- `/jobs?source_ref=...` 같은 현재 API 표면도 route 단위 테스트로 커버되고 있다.

다음 리팩터링은 route 파일 분해보다 계약과 행동 일관성에 집중하는 편이 낫다.

## 3. 미완료 항목

이 절의 항목만 보면 다음 작업을 이어갈 수 있다.

### 3.1 P1. typed error가 end-to-end로 닫히지 않았다

- 현재 상태
  - `AppError`는 도입됐지만 `server/app_api.rs`는 여전히 문자열 prefix로 HTTP error kind를 분류한다.
  - 예: `"job not found:"`, `"input file not found:"`, `"failed to resolve input path "`
  - `server/errors.rs`도 setup 관련 오류를 prefix 목록으로 판정한다.
- 문제
  - status code와 에러 의미가 도메인 타입보다 문자열 문구에 의존한다.
  - 에러 메시지 문구를 바꾸면 분류가 쉽게 깨진다.
- 다음 슬라이스
  - app 계층이 도메인 오류 enum 또는 `AppError`를 직접 반환하도록 바꾸고, server는 매핑만 담당하게 줄인다.

### 3.2 P1. embedding 성공 경로의 상태 기록이 한 번에 묶여 있지 않다

- 현재 상태
  - `app/embedding_stage.rs`는 embedding vector를 `upsert_summary_embedding()`으로 저장한 뒤,
  - 별도로 `finalize_task_success()`와 `update_job()`을 호출해 task 완료와 `job.summary_embedding`을 기록한다.
- 문제
  - embedding vector 저장과 task 완료/metadata 갱신이 단일 저장 단위로 보장되지 않는다.
  - 짧은 순간이라도 sidecar/vector와 job/task 상태가 어긋날 수 있다.
- 다음 슬라이스
  - embedding 성공 시 job/task/summary embedding을 한 단위로 반영하는 저장 API를 만든다.

### 3.3 P2. model preparation과 toolchain discovery 로직이 여전히 크고 분산돼 있다

- 현재 상태
  - `app/models.rs`, `llama.rs`, `whisper.rs`에 준비 제출, 대기, stale 판정, discovery, download/cache, CPU fallback 책임이 나뉘어 있다.
- 문제
  - whisper/llama 공통 패턴이 파일 경계를 넘어 반복된다.
  - 후속 수정 시 모델별 예외 처리와 공통 흐름을 함께 추적해야 한다.
- 다음 슬라이스
  - 공통 helper를 먼저 추출하고,
  - `llama.rs`와 `whisper.rs`는 `discovery`, `download/cache`, `runtime`, `output parsing` 축으로 내부 분할한다.

### 3.4 P2. stage submit/execute 패턴 중복이 남아 있다

- 현재 상태
  - `ffmpeg_stage.rs`, `stt_stage.rs`, `summary_stage.rs`, `embedding_stage.rs`가 reuse/deduplicate/submitted 판정,
  - queue enqueue,
  - success/failure finalize,
  - 후속 stage 제출 흐름을 각자 반복한다.
- 문제
  - 정책 변경 시 여러 stage 파일을 함께 수정해야 한다.
  - payload 비교 규칙과 공통 lifecycle 코드가 섞여 있다.
- 다음 슬라이스
  - 공통 stage lifecycle helper를 만들고,
  - stage별 차이점은 payload 검증과 후속 chaining만 남긴다.

### 3.5 P2. SQLite/Postgres backend parity 테스트가 없다

- 현재 상태
  - backend abstraction과 두 구현은 존재한다.
  - 하지만 동일 시나리오를 두 backend에 공통 적용하는 전용 parity 테스트 묶음은 보이지 않는다.
- 문제
  - 저장 구조가 늘수록 sqlite와 postgres 구현이 조용히 어긋날 위험이 커진다.
  - 특히 transcript, summary, queue, model preparation, embedding round-trip의 동작 일치가 중요하다.
- 다음 슬라이스
  - backend 공통 fixture를 만들고 핵심 저장/조회 시나리오를 두 backend에 같은 테스트로 적용한다.

### 3.6 P3. interactive CLI가 실제 지원 명령과 동기화되지 않는다

- 현재 상태
  - 인자 기반 CLI는 `prepare-models`, `prepare-llama-model`, `embed-summaries`, `search-summaries`를 지원한다.
  - 하지만 no-args 프롬프트는 아직 `ffmpeg`, `stt`, `summary`, `server` 네 가지만 보여 준다.
- 문제
  - CLI 기능 추가가 대화형 UX에 자동 반영되지 않는다.
- 다음 슬라이스
  - 프롬프트 표시 목록을 `CliCommand` 정의와 같은 소스에서 만들거나, 적어도 한 곳에서만 관리하게 바꾼다.

### 3.7 P3. 문서 동기화는 아직 수동이다

- 현재 상태
  - `docs/architecture.md`, `docs/openapi.yaml`은 현재 구조를 어느 정도 반영하지만 자동 생성 체계는 없다.
  - 저장소 기준 문서 일부는 아직 legacy JSON 인덱스 전제를 남기고 있다.
- 문제
  - 코드 구조가 바뀔 때 문서 드리프트가 다시 생긴다.
- 다음 슬라이스
  - 최소한 저장소/런타임/메타 저장소 기준 문서를 현재 구조에 맞춰 한 번 더 정리한다.

## 4. 다음 작업 순서 제안

다음 작업은 아래 순서로 보는 편이 효율적이다.

1. `3.1 typed error 정리`
2. `3.2 embedding 상태 기록 원자화`
3. `3.5 backend parity 테스트`
4. `3.3 model/toolchain helper 축소`
5. `3.4 stage lifecycle helper 정리`
6. `3.6 CLI 표시 동기화`
7. `3.7 문서 동기화`

## 5. 사용법

- 이미 끝난 축만 확인하려면 2장만 보면 된다.
- 이후 작업 backlog만 보려면 3장만 보면 된다.
- 실제 후속 구현은 3장의 항목을 위에서부터 하나씩 슬라이스로 잘라 진행하면 된다.
