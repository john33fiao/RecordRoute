# Rust 리팩토링 메모 (2026-03-28 코드 기준)

이 문서는 현재 코드베이스를 기준으로 이미 정리된 구조와 아직 남아 있는 리팩터링 포인트를 구분해서 적는다.
과거 배치 기록이 아니라 "지금 다시 손댄다면 어디를 먼저 볼지"에 초점을 둔다.

## 1. 현재 구조에서 이미 안정된 부분

### 1.1 대형 모듈 분리

초기의 단일 파일 집중 구조는 상당 부분 분해돼 있다.

- `app/*`
  - ffmpeg / stt / summary / embedding / models / cli / artifacts 분리
- `server/*`
  - routes / files / upload / errors / types / app_api 분리
- `index/*`
  - types / store / lock 분리

즉 현재 구조는 "전면 재작성"보다 각 하위 모듈 단위의 점진적 정리가 맞다.

### 1.2 테스트 분리

테스트는 프로덕션 파일 inline 비중이 낮고 다음 구조로 나뉘어 있다.

- `rust/src/app/tests/*`
- `rust/src/server/tests/*`
- 각 영역별 `support.rs`

리팩터링 시 테스트 보조 코드까지 함께 움직일 수 있는 형태는 이미 갖춰져 있다.

### 1.3 인덱스 저장 안정성

`IndexStore`는 다음 특성을 갖는다.

- `db/index.lock` 기반 동기화
- 임시 파일 기록 후 원자적 replace
- stale lock 판단 시 `PermissionDenied` 계열을 별도 취급

즉 현재 우선순위는 "인덱스 파일이 깨지는가"보다 "상태 갱신을 얼마나 일관되게 한 번에 묶느냐" 쪽이다.

### 1.4 산출물 규약 정리

summary 결과는 canonical 경로 `summary/result.md`로 정리돼 있다.
legacy `result.txt`, `<source_stem>.md`는 `app/artifacts.rs`가 읽는 시점에 승격한다.

## 2. 지금도 남아 있는 구조적 부채

### 2.1 도메인 오류가 끝까지 typed 하지는 않다

`AppError` 계층은 도입돼 있지만, `server/app_api.rs`는 여전히 여러 경로에서 문자열 prefix를 보고 `BadRequest`, `NotFound`, `DependencyUnavailable`를 분류한다.

예:

- `error.starts_with("job not found:")`
- `error.starts_with("input file not found:")`

의미:

- HTTP status 분류가 도메인 오류 타입보다 에러 메시지 문구에 여전히 일부 의존한다.
- app 계층에서 typed error를 직접 올릴 수 있도록 바꾸면 server 계층 단순화 여지가 있다.

### 2.2 상태 갱신 원자성이 stage마다 완전히 동일하지 않다

특히 embedding 경로는 다음 두 단계를 분리해서 기록한다.

1. task 완료 기록
2. `summary_embedding` metadata 기록

둘 다 결국 `IndexStore::update_job()`로 저장되지만, 한 번의 클로저 업데이트로 묶여 있지 않다.
짧은 순간 task 상태와 metadata가 완전히 동시에 보이지 않을 수 있다.

### 2.3 model/toolchain 준비 로직이 여러 층에 나뉘어 있다

현재 관련 책임이 다음 파일에 흩어져 있다.

- `app/models.rs`
- `whisper.rs`
- `llama.rs`

중복되는 패턴:

- 모델 소스 해석
- 준비 가능 여부 점검
- 준비 실행
- CPU fallback
- 실행 파일 탐색

generic 추상화를 크게 넣기보다, 공통 helper를 더 줄일 수 있는 후보가 남아 있다.

### 2.4 stage submit / execute 패턴도 반복된다

`ffmpeg_stage.rs`, `stt_stage.rs`, `summary_stage.rs`, `embedding_stage.rs`는 각각 다음 흐름을 비슷하게 반복한다.

- reusable / deduplicated / submitted 판정
- running task upsert
- execute
- success / failure finalize

각 stage의 의미 차이는 유지하되, 반복되는 update 패턴을 더 줄일 수는 있다.

### 2.5 CLI UX가 지원 명령과 완전히 맞물리지는 않는다

직접 인자 모드에서는 다음 명령까지 지원한다.

- `prepare-models`
- `prepare-llama-model`
- `embed-summaries`
- `search-summaries`

하지만 no-args 대화형 프롬프트는 아직 `ffmpeg`, `stt`, `summary`, `server`만 보여 준다.
CLI 기능 추가가 인터랙티브 UX에 자동 반영되지는 않는 상태다.

### 2.6 문서 동기화가 자동화돼 있지 않다

- `docs/openapi.yaml`
- `docs/architecture.md`
- `docs/embeddings.md`

이 문서들은 수동 유지다.
라우트나 응답 타입이 바뀌면 코드와 문서가 다시 어긋날 가능성이 높다.

## 3. 다음 우선순위 제안

### 우선순위 1

app 계층에서 typed error를 직접 반환하게 만들어 `server/app_api.rs`의 문자열 기반 분류를 줄인다.

### 우선순위 2

embedding 완료와 metadata 기록처럼 한 job의 상태를 두 번 쓰는 경로를 한 번의 update로 묶는다.

### 우선순위 3

`llama.rs`, `whisper.rs`, `app/models.rs` 사이의 모델 준비 공통 패턴을 helper 수준으로 정리한다.

### 우선순위 4

CLI 프롬프트와 문서가 실제 지원 명령을 자동으로 반영하도록 최소한의 동기화 장치를 둔다.

## 4. 리팩터링 원칙

- public contract를 바꾸지 않는 구조 개선을 우선한다.
- `db/index.json`을 단일 SoT로 유지한다.
- reuse / deduplicate 규칙을 흐트러뜨리지 않는다.
- 문서와 테스트를 같이 옮긴다.

## 5. 요약

현재 RecordRoute는 이미 모듈 분리와 인덱스 저장 안정성 면에서는 초반 단계를 지났다.
다음 리팩터링은 파일 쪼개기보다 다음 두 가지에 초점을 맞추는 편이 맞다.

- 문자열 의존 오류 분류를 줄여 계약을 단단하게 만드는 일
- 상태 전이와 metadata 기록을 더 일관되게 묶는 일
