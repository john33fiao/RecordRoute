# Rust 리팩토링 점검 메모 (2026-03-26, P1 반영 후 기준)

## 범위 재정렬

- 대상: `rust/src/*.rs`, `rust/src/app/*`, `rust/src/server/*`, `rust/src/index/*`
- 기준:
  - 단순 라인 수보다 먼저 계약 불일치, 상태 전이 ownership, 저장소 안정성, 테스트 가독성을 본다.
  - 이미 반영된 구조 개선과 아직 남은 과제를 분리해서 관리한다.

## 1. 현재 상태 요약

### 이미 해결된 항목

#### summary 산출물 파일 규약 통일

- `rust/src/app/artifacts.rs`
  - summary canonical 파일명을 `summary/result.txt`로 고정했다.
  - legacy `*.md` 산출물은 `ensure_summary_output_path`에서 canonical 경로로 승격한다.
- `rust/src/server/files.rs`
  - summary 조회는 공통 artifact helper를 통해 canonical 경로를 읽는다.

의미:

- 초기 점검 시 발견된 `app.rs` vs `server.rs` 간 summary 파일명 불일치는 해소됐다.
- 이제 summary contract 관련 후속 작업은 문서/테스트 정리 위주다.

#### `db/index.json` 원자적 쓰기 도입

- `rust/src/index/store.rs`
  - `index.<uuid>.tmp`에 먼저 기록하고 sync 후 replace 하는 흐름으로 저장한다.
- `rust/src/index.rs`
  - atomic replace 이후 temp 파일이 남지 않는 테스트가 있다.

의미:

- 초기 점검 시의 `fs::write` 직접 덮어쓰기 위험은 이미 제거됐다.
- 저장소 안정성 이슈의 우선순위는 이제 lock 경합 처리 쪽으로 옮겨간다.

#### 부분 모듈 분리 시작

- `rust/src/app/artifacts.rs`, `rust/src/app/stages.rs`
- `rust/src/server/errors.rs`, `rust/src/server/files.rs`, `rust/src/server/upload.rs`
- `rust/src/index/store.rs`, `rust/src/index/lock.rs`, `rust/src/index/types.rs`

의미:

- 후속 리팩토링은 전면 재작성보다 남은 responsibility를 단계적으로 떼는 방향이 맞다.

## 2. 이번 1차에서 정리한 항목

### P0. `index.lock` 경합 시 AccessDenied를 stale lock 오류로 오판하지 않도록 수정

근거:

- Windows에서 다른 스레드가 잡고 있는 `db/index.lock`를 읽는 동안 `PermissionDenied/AccessDenied`가 발생할 수 있었다.
- 이 경우 기존 구현은 stale 검사 중 즉시 오류를 반환해 API polling 테스트가 간헐적으로 실패했다.

반영:

- `rust/src/index/lock.rs`
  - lock 내용 읽기나 metadata 조회가 `PermissionDenied`면 stale로 보지 않고 "현재 사용 중인 lock"으로 간주한다.
  - timeout/polling 동작은 유지한다.
- `rust/src/index.rs`
  - active handle을 실제로 잡아 둔 상태에서도 `AccessDenied` 대신 timeout으로 처리되는 테스트를 추가했다.

기대 효과:

- `server::tests::post_jobs_returns_accepted_then_job_transitions_to_completed`가 Windows에서도 안정적으로 통과해야 한다.

### P0. 문자열 prefix 기반 HTTP 에러 분류 제거

근거:

- 기존 server는 `starts_with("job not found:")`, `starts_with("summary not found:")` 같은 문자열 규칙으로 HTTP status를 정했다.
- 메시지 문구가 바뀌면 status 분류가 같이 깨지는 구조였다.

반영:

- `rust/src/error.rs`
  - `AppErrorKind`, `AppError`, `AppResult<T>`를 추가했다.
- `rust/src/app.rs`
  - server가 직접 호출하는 submit/status/model 준비 경계에 typed error wrapper를 추가했다.
- `rust/src/server/upload.rs`, `rust/src/server/files.rs`
  - request/lookup/file access 계열 오류를 `AppError`로 직접 반환하도록 바꿨다.
- `rust/src/server/errors.rs`
  - 문자열 prefix 분기를 없애고 `AppErrorKind -> StatusCode` 매핑만 담당하게 바꿨다.
- `rust/src/server.rs`
  - handler가 더 이상 문자열을 직접 분류하지 않는다.

현재 kind 기준:

- `BadRequest`: 잘못된 입력 path, 잘못된 multipart body, 잘못된 transcript/file selector
- `NotFound`: job 없음, transcript 없음, summary 없음, 파일 없음
- `DependencyUnavailable`: ffmpeg/whisper/llama 툴체인 또는 모델 준비 불가
- `Internal`: 나머지 저장소/파일시스템/비동기 실행 오류

## 3. 이번 P1 배치 완료 상태

### P1. `app.rs`, `server.rs` 추가 분리 완료

현재:

- `rust/src/app.rs`는 facade만 남기고 구현을 `app/cli.rs`, `app/ffmpeg_stage.rs`, `app/stt_stage.rs`, `app/summary_stage.rs`, `app/models.rs`로 분리했다.
- `rust/src/server.rs`는 router bootstrap만 남기고 `server/types.rs`, `server/app_api.rs`, `server/routes/{ping,jobs,models,stages}.rs`로 분리했다.
- server 전용 `String -> AppError` 분류 래퍼를 `server/app_api.rs`로 이동했고, `server/errors.rs`, `server/files.rs`, `server/upload.rs`는 지원 모듈로 유지했다.

의미:

- 공개 인터페이스(`router()`, `router_with_repo_root()`, `serve()` 및 HTTP path/request/response shape)는 유지하면서 책임 경계를 분명히 했다.
- 이후 작업은 `app.rs`, `server.rs`의 대형 파일 구조를 다시 헤치지 않고 세부 모듈 단위로 진행할 수 있다.

### P1. inline test 분리 완료

현재:

- `rust/src/app/tests.rs`, `rust/src/server/tests.rs`를 추가하고 기존 inline test를 `app/tests/*`, `server/tests/*`로 옮겼다.
- fake toolchain/script writer/temp workspace/request helper는 각 `support.rs`로 모았다.
- `cargo test --manifest-path rust/Cargo.toml` 기준 103개 테스트가 모두 green이다.

의미:

- 프로덕션 코드와 테스트 코드의 읽기 경계가 분리돼 이후 리뷰 속도와 추가 분해 판단이 좋아졌다.
- 이번 배치는 계약 변경 없이 구조만 바꾸는 P1 목표를 충족했다.

### P2. `llama.rs`, `whisper.rs` 구조 공통화

현재:

- 툴체인 탐지, 모델 준비, 실행, 오류 후처리 패턴이 비슷하지만 아직 공통화되지 않았다.

권장:

- `toolchain`, `model`, `runtime`, `output` 성격으로 느슨하게 정리
- 과도한 generic 추상화보다 공통 helper 1~2개부터 시작

## 4. 우선순위 업데이트

1. 이번 P1 분리 이후에도 `cargo test --manifest-path rust/Cargo.toml` green 상태를 유지한다.
2. `llama.rs`, `whisper.rs` 공통 패턴을 helper 수준으로 묶는다.
3. 필요 시 `rust/tests/` integration 승격 여부를 재평가한다.
4. contract 변경이 생길 때만 OpenAPI/architecture 문서를 후속 동기화한다.

## 5. 최종 판단

- 초기 점검에서 가장 급했던 `summary 파일 규약 불일치`와 `index.json 원자성 부족`은 이미 해결됐다.
- 이번 배치로 `app.rs`, `server.rs` 책임 분리와 inline test 외부화가 완료됐다.
- 현재 남은 다음 우선순위는 `llama.rs`, `whisper.rs` 공통 패턴 정리이며, public contract 변경 없이 점진 분리로 가는 방향이 맞다.
