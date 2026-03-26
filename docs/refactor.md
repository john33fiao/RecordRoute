# Rust 리팩토링 점검 메모 (2026-03-26)

## 범위

- 대상: `rust/src/*.rs`
- 기준:
  - 500줄 초과 파일은 기능별 분리 후보로 우선 검토
  - 단순 줄 수 외에도 책임 혼합, 중복, 상태 전이 소유권, 파일/모델 경로 계약 불일치 여부를 함께 점검

## 1. 라인 수 기준 현황

| 파일 | 총 라인 | 프로덕션 라인 | 테스트 라인 | 1차 판단 |
| --- | ---: | ---: | ---: | --- |
| `rust/src/server.rs` | 2815 | 1378 | 1437 | 즉시 분리 대상 |
| `rust/src/app.rs` | 2805 | 1580 | 1225 | 즉시 분리 대상 |
| `rust/src/llama.rs` | 920 | 617 | 303 | 분리 대상 |
| `rust/src/index.rs` | 874 | 619 | 255 | 분리 대상 |
| `rust/src/whisper.rs` | 805 | 441 | 364 | 테스트 분리 우선, 이후 선택적 분리 |
| `rust/src/ffmpeg.rs` | 372 | 290 | 82 | 현재는 유지 가능 |
| `rust/src/lib.rs` | 18 | 9 | 9 | 유지 |
| `rust/src/main.rs` | 6 | 6 | 0 | 유지 |

메모:

- "총 라인 수" 기준으로는 `app.rs`, `server.rs`, `llama.rs`, `index.rs`, `whisper.rs`가 500줄을 넘습니다.
- "프로덕션 라인 수" 기준으로는 `app.rs`, `server.rs`, `llama.rs`, `index.rs`가 500줄을 넘습니다.
- `whisper.rs`는 프로덕션 코드 자체는 500줄 아래지만, 테스트가 같은 파일 안에 크게 붙어 있어 가독성이 빠르게 떨어집니다.

## 2. 먼저 정리해야 하는 항목

### P0. summary 산출물 파일 규약이 모듈마다 다름

근거:

- `rust/src/app.rs:535`, `rust/src/app.rs:614`, `rust/src/app.rs:795`, `rust/src/app.rs:1375-1385`
  - summary 출력 파일을 `<source_stem>.md`로 계산
- `rust/src/server.rs:921-944`, `rust/src/server.rs:1315-1328`
  - API는 `summary/result.txt`를 읽는다고 가정
- `docs/architecture.md:111`, `docs/openapi.yaml:558`
  - 문서도 `summary/result.txt` 기준

영향:

- CLI/도메인 로직과 HTTP API가 서로 다른 summary 파일 이름을 전제로 동작합니다.
- 현재 구조에서는 app 레이어에서 summary를 생성해도 API `GET /jobs/{job_id}/summary/text`가 결과를 못 찾을 수 있습니다.
- 이 상태에서는 "파일 분리"보다 먼저 산출물 계약을 하나로 맞춰야 합니다.

권장:

- summary/STT/오디오 산출물 경로 계산을 `artifacts` 또는 `job_files` 모듈로 모읍니다.
- 파일 이름 규칙은 한 곳에서만 정의하고, `app.rs`, `server.rs`, 문서가 모두 그 헬퍼를 사용하도록 바꿉니다.

### P0. STT / Summary 상태 전이 소유권이 `app.rs`와 `server.rs`에 나뉘어 있음

근거:

- `rust/src/app.rs:374-430`
  - ffmpeg는 성공/실패 모두 `app.rs`에서 인덱스 상태까지 갱신
- `rust/src/app.rs:486-521`, `rust/src/app.rs:591-637`
  - STT / Summary는 성공 시에만 task 완료 처리
- `rust/src/server.rs:624-641`, `rust/src/server.rs:857-873`
  - 실패 시 task 실패 기록은 서버의 백그라운드 spawn closure에서 보정
- `rust/src/app.rs:674-833`
  - CLI 경로는 `submit_*` / `execute_*`를 재사용하지 않고 별도 흐름으로 실행

영향:

- 동일 도메인 규칙이 API와 CLI에서 다르게 유지됩니다.
- 실패 처리 책임이 app 서비스 계층이 아니라 server 핸들러 안에 숨어 있습니다.
- 이후 라우트를 늘리거나 실행 경로를 추가하면 상태 전이 누락이 발생하기 쉽습니다.

권장:

- `ffmpeg/stt/summary` 각각에 대해 "submit + execute + fail"을 완결하는 stage service를 둡니다.
- server는 그 service를 호출만 하고, 상태 보정 로직을 직접 가지지 않도록 정리합니다.
- CLI도 `run_stt_with_repo_root`, `run_summary_with_repo_root` 내부에서 같은 stage service를 사용하도록 맞춥니다.

### P0. `db/index.json` 쓰기가 원자적이지 않고 lock 파일도 stale 처리 전략이 없음

근거:

- `rust/src/index.rs:529-570`
  - lock 획득 후 전체 JSON을 읽고 `fs::write`로 다시 덮어씀
- `rust/src/index.rs:577-617`
  - lock은 단순 create/remove 방식이며 stale lock 정리 정보가 없음

영향:

- 저장 도중 프로세스가 죽으면 `db/index.json`이 손상될 수 있습니다.
- 비정상 종료 시 `db/index.lock`가 남으면 수동 개입이 필요할 수 있습니다.
- 이 저장소는 SoT이므로, 이 부분은 단순 스타일 문제가 아니라 안정성 문제입니다.

권장:

- `index.json.tmp`에 기록 후 rename하는 방식으로 바꿉니다.
- lock 파일에 pid / timestamp를 남기거나, stale lock 감지 규칙을 둡니다.
- 가능하면 저장과 상태 전이 보조 헬퍼를 함께 묶어 "읽기-변경-원자적 쓰기" 경계를 명확히 합니다.

## 3. 파일별 리팩토링 메모

### `rust/src/app.rs`

현재 역할:

- CLI 파싱 / 프롬프트
- ffmpeg / stt / summary stage submit/execute
- 모델 준비 orchestration
- 파일 경로 계산
- 프롬프트 생성
- 출력 포맷팅

문제점:

- 한 파일에 "CLI UI", "도메인 서비스", "모델 준비", "파일 규칙", "프롬프트 템플릿"이 모두 섞여 있습니다.
- `run_stt_with_repo_root` / `run_summary_with_repo_root`가 stage service를 재사용하지 않아 API와 CLI가 분기됩니다.
- `collect_model_status_entry`, `inspect_model_preparation`, `ensure_model_with_toolchain`는 `Whisper` / `Llama` 분기 코드가 반복됩니다.
- `is_model_preparation_stale` (`rust/src/app.rs:1107-1126`)는 timestamp 문자열 비교에 의존하고 있어 로직이 취약합니다.

권장 분리안:

```text
rust/src/app/
  mod.rs
  cli.rs
  ffmpeg_stage.rs
  stt_stage.rs
  summary_stage.rs
  model_prepare.rs
  artifacts.rs
  prompt.rs
```

우선 옮길 후보:

- `main_cli`, `resolve_cli_command`, `prompt_for_mode`, `print_*` -> `cli.rs`
- `submit_ffmpeg_job`, `execute_ffmpeg_job`, `wait_for_ffmpeg_job_completion` -> `ffmpeg_stage.rs`
- `submit_stt_job`, `execute_stt_job`, `run_stt_with_repo_root` -> `stt_stage.rs`
- `submit_summary_job`, `execute_summary_job`, `run_summary_with_repo_root`, `build_summary_prompt` -> `summary_stage.rs` / `prompt.rs`
- `submit_model_preparation`, `execute_model_preparation`, `wait_for_model_preparation`, `collect_model_status_snapshot` -> `model_prepare.rs`
- `supported_audio_files`, `transcript_output_path`, `summary_output_path` -> `artifacts.rs`

### `rust/src/server.rs`

현재 역할:

- HTTP DTO 정의
- Router 등록
- 모델 / job / stt / summary 라우트 핸들러
- multipart 파싱
- 파일 조회
- 상태 응답 가공
- 에러 문자열을 HTTP status로 매핑

문제점:

- 라우트 정의, DTO, 파일 접근, 업로드 파싱, 응답 변환, 상태 코드 분류가 한 파일에 몰려 있습니다.
- `parse_uploaded_file` (`rust/src/server.rs:432-519`)는 수동 multipart parser이며, `post_jobs_upload` (`rust/src/server.rs:359-405`)는 요청 바디 전체를 메모리에 올립니다.
- `classify_*_error` (`rust/src/server.rs:1056-1097`)는 문자열 prefix에 강하게 결합되어 있습니다.
- `post_jobs`, `post_stt`, `post_summary` 사이에 reused / deduplicated 응답 status 처리 방식도 일관되지 않습니다.

권장 분리안:

```text
rust/src/server/
  mod.rs
  state.rs
  dto.rs
  errors.rs
  upload.rs
  files.rs
  routes/
    system.rs
    models.rs
    jobs.rs
    stt.rs
    summary.rs
```

우선 옮길 후보:

- DTO 전체 (`rust/src/server.rs:29-179`) -> `dto.rs`
- router / state -> `mod.rs`, `state.rs`
- `post_jobs_upload`, `parse_uploaded_file`, `parse_boundary`, `stable_content_hash` -> `upload.rs`
- `collect_job_files`, `read_stt_transcripts`, `read_summary_text`, `read_job_file` -> `files.rs`
- `classify_*_error`, `error_response` -> `errors.rs`

추가 권장:

- 가능하면 수동 multipart 파서는 없애고 axum/multer extractor로 교체합니다.
- background spawn 공통 wrapper를 두고, 실패 시 task 상태 갱신까지 한 곳에서 처리합니다.

### `rust/src/index.rs`

현재 역할:

- index 데이터 모델
- job/task/model preparation 상태 전이
- JSON 저장소 구현
- lock 구현

문제점:

- 도메인 타입과 저장소 구현, 파일 lock이 모두 한 파일에 섞여 있습니다.
- `update_job` (`rust/src/index.rs:420-434`)는 `FnOnce(&mut JobRecord) -> JobRecord` 형태라 호출부에서 계속 clone 전체 교체 패턴을 강제합니다.
- `update_task` (`rust/src/index.rs:265-291`)는 task가 없으면 조용히 새 task를 만들어 버려, 상태 전이 invariant가 약합니다.

권장 분리안:

```text
rust/src/index/
  mod.rs
  types.rs
  store.rs
  lock.rs
  transitions.rs
```

우선 리팩토링 포인트:

- `JobRecord`, `TaskRecord`, `ModelPreparationRecord` -> `types.rs`
- `IndexStore` -> `store.rs`
- `LockGuard` -> `lock.rs`
- `mark_*`, `complete_task`, `fail_task` 류 전이 로직 -> `transitions.rs`

추가 권장:

- `update_job`은 `FnOnce(&mut JobRecord) -> Result<(), AppError>` 형태로 바꾸는 편이 낫습니다.
- "없는 task를 성공/실패 처리"는 조용히 생성하지 말고 에러로 돌리는 쪽이 안전합니다.

### `rust/src/llama.rs`

현재 역할:

- 툴체인 탐지
- 모델 source 결정
- Hugging Face 다운로드 / 캐시
- summary 실행
- 출력 텍스트 정제

문제점:

- runtime 실행, 다운로드, 캐시 승격, stdout 파싱까지 모두 한 파일에 있습니다.
- `whisper.rs`와 구조가 매우 비슷한데도 공통화되지 않았습니다.
- `download_hugging_face_model` (`rust/src/llama.rs:325-447`)이 spawn / poll / 파일 탐지 / rename까지 한 함수에 몰려 있습니다.

권장 분리안:

```text
rust/src/llama/
  mod.rs
  toolchain.rs
  download.rs
  cache.rs
  output.rs
```

우선 리팩토링 포인트:

- model source / cache path 계산 -> `toolchain.rs`, `cache.rs`
- Hugging Face 다운로드와 poll loop -> `download.rs`
- `extract_summary_text`, `normalize_summary_text` -> `output.rs`

### `rust/src/whisper.rs`

현재 역할:

- 툴체인 탐지
- 모델 경로 해석 / 다운로드
- 전사 실행
- backend fallback

문제점:

- 프로덕션 코드만 보면 당장 강제 분리까지는 아니지만, `llama.rs`와의 공통 패턴이 많습니다.
- 현재는 테스트 비중이 커서 파일을 읽기 어렵습니다.

권장:

- 1차로 테스트를 별도 파일로 떼어내고,
- 2차로 `toolchain.rs`, `model.rs`, `runtime.rs` 정도로만 가볍게 분리하면 충분합니다.
- `stderr -> stdout -> unknown error` 패턴과 CPU fallback 판정 로직은 `llama.rs`와 공통 헬퍼 후보입니다.

### `rust/src/ffmpeg.rs`

판단:

- 현재는 책임이 비교적 선명합니다.
- 다만 `build_script_path`, `target_dir_name`, `locate_command`는 `whisper.rs` / `llama.rs`에서도 재사용하므로, 장기적으로는 `toolchain_paths.rs` 같은 공통 유틸로 옮길 가치가 있습니다.

### `rust/src/main.rs`, `rust/src/lib.rs`

판단:

- 현재 구조로 충분합니다.
- 리팩토링 우선순위는 매우 낮습니다.

## 4. 공통 리팩토링 항목

### 4.1 산출물 경로 규칙을 공유 모듈로 승격

후보:

- audio 파일 판별
- transcript 파일 판별
- STT transcript 출력 경로
- summary 출력 경로
- job 내 허용 파일 목록

현재 중복:

- `rust/src/app.rs:1293-1385`
- `rust/src/server.rs:1099-1369`

권장 모듈명 예시:

- `rust/src/artifacts.rs`
- 또는 `rust/src/job_files.rs`

### 4.2 문자열 기반 에러 분류를 typed error로 교체

현재:

- server가 error string prefix를 보고 `BAD_REQUEST`, `SERVICE_UNAVAILABLE` 등을 결정

문제:

- 에러 메시지 문구를 바꾸는 순간 HTTP status 분류가 깨집니다.

권장:

- `AppError` / `ErrorKind` enum을 두고
- app 레이어는 enum을 반환
- server 레이어는 `IntoResponse` 또는 변환 테이블만 담당

### 4.3 테스트를 같은 파일 안에 두지 말고 분리

현재:

- `server.rs`, `app.rs`, `whisper.rs`, `llama.rs`, `index.rs` 모두 테스트가 매우 큼

권장:

- 우선 `mod tests;` + 별도 파일 분리
- 필요하면 `rust/tests/` integration test로 일부 승격

효과:

- 프로덕션 코드 리뷰 속도가 바로 빨라집니다.
- 줄 수 기준 분리 판단도 더 정확해집니다.

## 5. 권장 실행 순서

1. summary 산출물 경로 규약을 하나로 통일합니다.
2. STT / Summary 실패 상태 갱신을 app service 계층으로 끌어올립니다.
3. `index.json` 원자적 쓰기와 lock stale 처리부터 보강합니다.
4. 공통 산출물 경로 / 파일 접근 모듈을 먼저 추출합니다.
5. `app.rs`, `server.rs`를 기능별 디렉터리 모듈로 분리합니다.
6. `index.rs`, `llama.rs`, `whisper.rs`를 책임별로 나눕니다.
7. 마지막으로 테스트를 파일 밖으로 분리해 구조를 정리합니다.

## 6. 최종 판단

- 가장 먼저 쪼개야 할 파일은 `app.rs`, `server.rs`입니다.
- 그 다음은 `index.rs`, `llama.rs`입니다.
- `whisper.rs`는 "프로덕션 코드 분리"보다 "테스트 분리"가 먼저입니다.
- `ffmpeg.rs`는 지금 당장 나눌 필요는 크지 않습니다.
- 다만 이번 점검에서 가장 중요한 것은 "500줄 초과 파일 분리" 자체보다도,
  `summary 파일 규약 불일치`, `상태 전이 소유권 분산`, `index.json 원자성 부족`을 먼저 정리하는 것입니다.
