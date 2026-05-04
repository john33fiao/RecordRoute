# RecordRoute TODO

작성일: 2026-05-04

현재 코드베이스와 문서 기준으로 개선 여지가 큰 항목을 우선순위별로 정리한다. 점검 범위는 `rust/src/**`, `rust/web/*`, `frontend/`, `docs/**`, `README.md`, `.github/`, `.gitignore`이며, `cargo test --manifest-path rust/Cargo.toml` 실행 결과도 반영했다.

## 점검 요약

- Rust 테스트는 총 232개 중 230개가 통과했고 2개가 실패했다.
- 실패한 테스트는 모두 Windows 경로/스크립트 확장자 가정과 관련되어 있다.
- `frontend/`에는 실제 소스가 없고 `build/`, `node_modules/` 15,993개 파일이 Git 추적 대상이다.
- `.github/workflows`가 없어 현재 테스트/포맷/빌드가 CI에서 자동 검증되지 않는다.
- 서버와 큐는 구현과 테스트가 풍부하지만, 메타데이터 저장소의 read-modify-write 경합 방지 장치가 약하다.
- `README.md`에는 3단계 파이프라인과 `docs/API_TODO.md` 링크 등 현행 문서와 어긋난 내용이 남아 있다.

## P0

### 1. Windows 테스트 실패 수정

근거:

- `tool_runtime::tests::local_toolchain_layout_builds_expected_paths`가 `scripts/build_llama.sh`를 기대하지만 Windows에서는 `ffmpeg::build_script_extension()`이 `bat`를 반환한다.
- `storage::tests::env_overrides_take_precedence`가 `/tmp/rr-audio`를 Unix 절대경로처럼 기대하지만 Windows에서는 `C:/tmp/rr-audio`로 해석된다.
- `server/tests/support.rs::make_executable(path)`는 Windows에서 `path`가 사용되지 않아 warning이 발생한다.

개선:

- 플랫폼별 기대값을 `build_script_extension()` 또는 `cfg!(windows)` 기준으로 맞춘다.
- 테스트의 절대경로 fixture를 OS별로 분기하거나 `temp_dir()` 기반으로 바꾼다.
- Windows 전용 unused variable warning을 제거한다.

검증:

- `cargo test --manifest-path rust/Cargo.toml`
- `cargo test --manifest-path rust/Cargo.toml --no-run`

### 2. 메타데이터 read-modify-write 경합 방지

근거:

- `IndexStore::with_index_mut()`는 `read_index -> mutate -> write_index` 흐름이며, 서버 요청은 `spawn_blocking`, 큐 dispatcher는 별도 thread에서 동시에 실행될 수 있다.
- SQLite/PostgreSQL write 자체는 transaction으로 묶지만, 여러 writer가 같은 이전 스냅샷을 읽은 뒤 각각 전체 index를 다시 쓰면 update lost 가능성이 있다.
- `write_index_transaction()`은 jobs/tasks/model/queue를 지우고 전체 재삽입하는 방식이라 job 수가 늘수록 비용과 경합 영향이 커진다.

개선:

- 같은 runtime root 기준의 metadata mutation guard를 도입하거나 backend별 transaction 안에서 read/mutate/write가 닫히도록 API를 재구성한다.
- SQLite는 단일 프로세스 기준 lock 또는 `BEGIN IMMEDIATE` 성격의 쓰기 구간을 검토한다.
- PostgreSQL은 row lock, optimistic version, 또는 queue/job 단위 update API로 전체 index rewrite를 줄인다.
- 동시 `POST /jobs/upload`, stage enqueue, dispatcher 실행을 섞은 회귀 테스트를 추가한다.

검증:

- 동시 제출/dispatch 테스트에서 job/task/queue 누락이 없어야 한다.
- SQLite와 PostgreSQL parity test에 경합 시나리오를 추가한다.

### 3. 추적된 `frontend/build`와 `frontend/node_modules` 정리

근거:

- `git ls-files frontend` 기준 15,993개 파일이 추적 중이고 모두 `frontend/build` 또는 `frontend/node_modules` 계열이다.
- `frontend/` 루트에는 `src`, `package.json`, `vite.config` 같은 소스 기준 파일이 없다.
- `.gitignore`는 `package/`, `.build/`, `rust/target/`는 제외하지만 `frontend/build`, `frontend/node_modules`는 제외하지 않는다.

개선:

- React/Vite 프론트엔드를 유지할 계획이면 실제 source tree와 lockfile만 남기고 build/dependency 산출물은 제거한다.
- 현행 기준이 `rust/web/*` 내장 UI라면 `frontend/` 전체를 deprecated 또는 제거 대상으로 정리한다.
- `.gitignore`에 `frontend/build/`, `frontend/node_modules/`를 추가한다.

검증:

- `git ls-files frontend/build frontend/node_modules`가 비어 있어야 한다.
- 내장 UI 기준이면 `cargo test --manifest-path rust/Cargo.toml --lib server::tests::web`

## P1

### 4. CI 워크플로 추가

근거:

- `.github/workflows`가 없다.
- 로컬 전체 테스트가 현재 실패하는데 자동 게이트가 없으면 회귀가 계속 누적될 수 있다.

개선:

- 최소 CI: `cargo fmt --all --check`, `cargo test --manifest-path rust/Cargo.toml --no-run`, `cargo test --manifest-path rust/Cargo.toml`.
- Windows를 1차 runner로 두고, 가능하면 Linux도 추가해 경로/스크립트 분기 회귀를 잡는다.
- P0 테스트 실패 수정 후 `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings` 도입을 검토한다.

검증:

- PR 또는 push에서 CI가 자동 실행되고, 현재 로컬 테스트와 동일한 실패를 재현하거나 통과해야 한다.

### 5. `batch-process all`의 후속 stage 자동 연쇄

근거:

- 문서상 `batch-process all`은 계획 시점에 summary가 이미 있는 job만 embedding 대상으로 잡는다.
- 사용자는 "전체 처리"를 기대하지만 같은 요청에서 새로 생성될 summary의 embedding은 별도 호출이 필요하다.

개선:

- summary 성공 후 embedding을 자동 enqueue하는 옵션을 추가하거나, batch plan에 dependency-aware follow-up enqueue를 도입한다.
- 기본 동작 변경이 부담되면 `target=all_with_embedding` 같은 명시 옵션을 검토한다.

검증:

- ffmpeg만 완료된 job에 `POST /jobs/batch-process {"target":"all"}` 또는 신규 옵션을 호출했을 때 stt, summary, embedding까지 최종 완료되는 테스트를 추가한다.

### 6. Summary API의 malformed JSON 처리 일관화

근거:

- 대부분의 JSON POST API는 malformed body를 `400 invalid request body`로 처리한다.
- `POST /jobs/{job_id}/summary`는 JSON 파싱 실패도 `force_regenerate=false`처럼 처리한다고 문서화되어 있다.

개선:

- 빈 본문은 허용하되, 비어 있지 않은 malformed JSON은 `400`으로 반환한다.
- `docs/API_Doc.md`와 `docs/openapi.yaml`을 함께 갱신한다.

검증:

- malformed JSON, 빈 본문, `{}` 각각에 대한 서버 라우트 테스트를 추가한다.

### 7. 내장 Web UI 유지보수성 개선

근거:

- `rust/web/app.js`는 약 105KB, `app.css`는 약 33KB, `index.html`은 약 28KB 단일 파일이다.
- `server/tests/web.rs`는 UI 동작보다 문자열 포함 여부를 많이 확인해 리팩터에 취약하다.

개선:

- 현행 내장 UI를 유지하더라도 JS를 API client, state, render, actions 단위로 분리한 뒤 빌드 산출물을 `rust/web`에 넣는 흐름을 정한다.
- 브라우저 smoke test 또는 DOM 기반 테스트를 추가해 upload, queue, stats, dictionary, search 주요 흐름을 검증한다.
- UI 카피는 운영 도구 성격에 맞게 상태/범위/조치 중심으로 정리한다.

검증:

- 주요 탭 이동, job 선택, queue pause/resume, dictionary CRUD, summary search가 smoke test에서 통과해야 한다.

### 8. Postgres 운영성 보강

근거:

- PostgreSQL backend는 각 작업마다 새 connection을 열고 schema initialization을 수행한다.
- 현재 parity test는 충분하지만 운영 연결 관리, TLS, migration versioning, connection pooling 기준은 약하다.

개선:

- 연결 풀 또는 backend handle 재사용 전략을 검토한다.
- schema initialization과 migration을 명시 버전으로 분리한다.
- `NoTls` 외 운영 TLS 선택지를 설정으로 열어 둘지 결정한다.

검증:

- PostgreSQL parity test와 장시간 반복 enqueue/read test에서 connection 누수와 성능 저하가 없어야 한다.

## P2

### 9. README 최신화

근거:

- README 상단은 3단계 파이프라인까지만 설명하지만 현행 파이프라인은 `ffmpeg -> stt -> summary -> embedding`이다.
- README 하단 관련 문서 링크가 `docs/API_TODO.md`를 가리키지만 현재 활성 문서는 `docs/API_Doc.md`, 과거 TODO는 `docs/deprecated/API_TODO.md`다.
- embedding/search, queue/dictionary, reset 등 현재 사용자 기능 설명이 API 문서보다 약하다.

개선:

- README의 사용자 기능, 실행 흐름, API 예시, 관련 문서 링크를 현행 기준으로 맞춘다.
- `docs/API_Doc.md`를 1차 기준 문서로 명시한다.

검증:

- README의 엔드포인트/환경변수/문서 링크가 `AGENTS.md`, `docs/API_Doc.md`, `docs/architecture.md`와 충돌하지 않아야 한다.

### 10. OpenAPI와 구현 drift 방지

근거:

- `docs/openapi.yaml`은 수동 상세 명세이고 `docs/API_Doc.md`가 1차 기준 문서다.
- 라우트 추가/삭제 시 수동 갱신 누락 가능성이 있다.

개선:

- `rust/src/server.rs` 라우트 목록과 `docs/openapi.yaml` path 목록을 비교하는 lightweight 검증 스크립트를 추가한다.
- API 의미 변경 PR에서는 `docs/API_Doc.md`, `docs/openapi.yaml`, 서버 테스트를 같은 변경으로 묶는다.

검증:

- 검증 스크립트가 `/jobs/{job_id}/files/{*file_name}` 같은 axum wildcard 예외를 명시적으로 허용해야 한다.

### 11. 파일 다운로드 응답 품질 개선

근거:

- `/jobs/{job_id}/files/{*file_name}`는 raw bytes를 반환하지만 Content-Type과 Content-Disposition이 명시적이지 않을 수 있다고 문서화되어 있다.

개선:

- WAV, TXT, Markdown별 Content-Type을 설정한다.
- 다운로드 파일명을 안전하게 제공하는 Content-Disposition을 검토한다.

검증:

- 오디오, transcript, summary 각각에 대한 header 테스트를 추가한다.

### 12. 검색 확장성 기준 수립

근거:

- 현재 summary search는 DB 내부 vector index 없이 Rust에서 brute-force cosine similarity를 계산한다.
- 문서에서도 ANN index, query embedding cache, transcript chunk 검색은 범위 밖으로 정리되어 있다.

개선:

- job 수 기준으로 brute-force 검색 허용 한계를 정한다.
- 검색 지연이 사용자 체감 병목이 되는 시점의 전환 후보를 정리한다: query embedding cache, summary vector pagination, PostgreSQL vector extension, 외부 vector DB 등.

검증:

- synthetic summary/embedding fixture로 검색 latency benchmark를 추가한다.

### 13. 자동 키워드 활용 정책 명확화

근거:

- summary 기반 auto keyword는 저장되지만 STT prompt에는 user keyword만 자동 주입된다.
- UI에는 auto keyword promote/delete가 있으나, 사용자가 언제 promote해야 하는지 운영 기준은 약하다.

개선:

- auto keyword를 기본 주입하지 않는 현 정책을 유지할지, 신뢰도/횟수/사용자 승인 기준으로 주입할지 결정한다.
- 정책 결정 후 `docs/API_Doc.md`, `docs/architecture.md`, UI 카피를 맞춘다.

검증:

- auto keyword가 STT payload에 들어가거나 들어가지 않는 기준을 테스트로 고정한다.

### 14. 운영 로그와 진단성 개선

근거:

- dispatcher와 stage 실행 경로는 `eprintln!` 중심이다.
- 패키지 런처는 `logs/server.log`를 사용하지만 API 요청, queue dispatch, 외부 toolchain 실패를 구조화해서 추적하기 어렵다.

개선:

- `tracing` 기반 로그 레벨과 request/job/task context를 도입한다.
- `/system/status`, `/models/status`, `/queue` 외에 최근 실패 task 또는 last dispatch error를 보는 진단 API가 필요한지 검토한다.

검증:

- 실패한 ffmpeg/stt/summary/embedding task에서 job_id, task_id, toolchain, sanitized error가 로그와 API에 일관되게 남아야 한다.

