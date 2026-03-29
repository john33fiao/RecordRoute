# RecordRoute 아키텍처 (2026-03-29 코드 기준)

이 문서는 현재 `rust/src/**` 구현을 기준으로 RecordRoute의 실행 구조와 저장 모델을 요약한다.
이제 저장소는 `db/index.json`과 `db/<job_id>` 파일 트리가 아니라, 메타데이터 DB와 오디오 전용 경로 저장소로 분리되어 있다.

## 1. 런타임 루트

런타임 루트는 `rust/src/runtime_root.rs`가 다음 우선순위로 결정한다.

1. `RECORDROUTE_RUNTIME_ROOT`
2. 실행 파일 옆 `.recordroute-runtime-root`
3. 실행 파일 옆 `RecordRouteServer(.exe)`
4. 개발 환경에서는 저장소 루트

이 루트를 기준으로 `db/`, `models/`, `logs/`, `.env`를 읽는다.

## 2. 주요 모듈

- `rust/src/app.rs`
  - CLI/API 공용 facade와 공용 타입
- `rust/src/app/ffmpeg_stage.rs`
  - 입력 소스 publish, ffmpeg job 제출/실행, 재사용/중복 제거
- `rust/src/app/stt_stage.rs`
  - STT task 제출/실행, transcript DB 저장
- `rust/src/app/summary_stage.rs`
  - summary task 제출/실행, summary DB 저장
- `rust/src/app/embedding_stage.rs`
  - summary embedding 생성과 검색
- `rust/src/app/queue.rs`
  - 영속 queue 상태와 dispatcher 로직
- `rust/src/index/store.rs`
  - 메타데이터 facade
- `rust/src/index/sqlite.rs`
  - `rusqlite` 기반 메타DB 구현
- `rust/src/index/postgres.rs`
  - PostgreSQL 기반 메타DB 구현
- `rust/src/audio_store.rs`
  - 경로 기반 오디오 저장소
- `rust/src/storage.rs`
  - `.env` 기반 저장소 설정 로드
- `rust/src/server.rs`, `rust/src/server/routes/*.rs`
  - axum HTTP 서버와 API 라우트

## 3. 저장 모델

### 3.1 기본 경로

기본 개발 모드는 모두 런타임 루트의 `db/` 아래를 사용한다.

- 메타DB 기본값: `db/index.sqlite3`
- 오디오 루트 기본값: `db/audio/`
- 오디오 cache 기본값: `db/audio-cache/`
- 오디오 spool 기본값: `db/audio-spool/`

### 3.2 환경 변수

저장소는 `.env`만으로 설정한다.

- `RECORDROUTE_METADATA_DRIVER=sqlite|postgres`
- `RECORDROUTE_METADATA_SQLITE_PATH`
- `RECORDROUTE_METADATA_POSTGRES_URL`
- `RECORDROUTE_AUDIO_ROOT`
- `RECORDROUTE_AUDIO_CACHE_ROOT`
- `RECORDROUTE_AUDIO_SPOOL_ROOT`

기본값을 그대로 쓰면 메타DB와 오디오 둘 다 `db/` 아래에 놓인다.
운영에서 외부 분리가 필요하면 메타DB를 PostgreSQL로 바꾸거나 `RECORDROUTE_AUDIO_ROOT`를 SMB 마운트, OneDrive 동기화 폴더 등으로 바꾼다.

### 3.3 권위 저장소

권위 저장소는 다음 둘이다.

- 메타DB
- 오디오 루트

`audio-cache`와 `audio-spool`은 재생성 가능한 작업 디렉터리다.
로컬 cache/spool을 비워도 완료 job의 transcript/summary/embedding 조회와 후속 처리는 가능해야 한다.
legacy `db/index.json` 파일이 남아 있어도 현재 backend는 이를 권위 저장소로 읽지 않는다.

## 4. 메타데이터 DB 스키마

현재 메타DB는 다음 논리 엔터티를 가진다.

- `jobs`
  - job 상태, source ref, source kind, source hash, source file name, probe, split strategy, 오류, timestamps
- `tasks`
  - ffmpeg/stt/summary/embedding task 상태, retry, 오류, timestamps
- `model_preparations`
  - whisper/llama/llama_embedding 준비 상태
- `queue_state`
  - active batch, pending batch, burst limit
- `stt_dictionary_keywords`
  - user keyword와 summary 기반 auto keyword 저장
- `audio_artifacts`
  - job별 logical file name과 오디오 storage key
- `transcripts`
  - transcript id, file name, text
- `summaries`
  - summary file name, markdown text, optional one-line alias
- `summary_embeddings`
  - embedding metadata와 벡터

`job_dir`와 절대 파일 경로는 영속 메타데이터에 저장하지 않는다.
현재 index 포맷 버전은 `4`다.

## 5. 오디오 저장 규약

### 5.1 입력 소스

업로드와 로컬 파일 입력은 모두 먼저 spool에 기록한 뒤 SHA-256을 계산하고, 오디오 루트의 content-addressed 경로로 publish한다.

예시:

- `audio/sources/<sha256>/source.wav`
- `audio/sources/<sha256>/source.bin`

Job 재사용과 중복 제거는 경로가 아니라 `source_content_sha256` 기준으로 판단한다.

### 5.2 ffmpeg 산출물

ffmpeg 결과 오디오는 오디오 루트 아래 job별 디렉터리에 저장한다.

예시:

- `audio/jobs/<job_id>/mono_mix.wav`
- `audio/jobs/<job_id>/channel_01.wav`
- `audio/jobs/<job_id>/channel_02.wav`

메타DB에는 실제 절대 경로 대신 logical file name과 storage key를 기록한다.

## 6. 파이프라인

### 6.1 ffmpeg

1. 입력 소스를 publish한다.
2. 같은 `source_content_sha256`의 완료 job이 있고 오디오 산출물이 유효하면 `Reused`
3. 같은 hash의 queued/running job이 있으면 `Deduplicated`
4. 아니면 새 job을 만들고 `ffmpeg` queue에 넣는다.
5. 실행 시 source를 cache로 materialize하고 `ffprobe`, `ffmpeg`를 실행한다.
6. 결과 오디오는 오디오 루트에 남고, 메타DB에 `audio_artifacts`와 job 상태를 기록한다.

### 6.2 stt

1. ffmpeg 완료 job만 대상이다.
2. logical audio file subset 또는 `mono_mix_only`를 해석한다.
3. transcript DB 레코드가 모두 있으면 `Reused`
4. queued/running이면 `Deduplicated`
5. 아니면 `stt` queue에 넣는다.
6. 실행 시 오디오를 읽고 whisper 결과를 spool에 잠시 만든 뒤 transcript text를 DB에 저장한다.
7. 현재 STT prompt 주입에는 `user` source keyword만 사용하고 `auto` keyword는 아직 자동 주입하지 않는다.

### 6.3 summary

1. transcript DB 레코드가 준비된 job만 대상이다.
2. summary DB 레코드가 있으면 `Reused`
3. queued/running이면 `Deduplicated`
4. 아니면 `llm` queue에 넣는다.
5. 실행 시 transcript text로 prompt를 만들고 summary markdown을 생성한 뒤 DB에 저장한다.
6. 같은 summary task 안에서 전체요약을 바탕으로 한줄요약(alias)을 생성해 같은 row에 저장한다.
7. 이어서 같은 generative LLM으로 summary 기반 핵심 keyword를 추출하고 전역 `stt_dictionary_keywords`의 `auto` source에 저장한다.

### 6.4 embedding

1. summary DB 레코드가 준비된 job만 대상이다.
2. 기존 embedding metadata의 `text_sha256`이 summary 본문과 같으면 재사용한다.
3. stale 하거나 없으면 `embed` queue에 넣는다.
4. 실행 시 summary 본문으로 embedding을 만들고 벡터를 DB에 저장한다.

검색은 현재 DB 내부 벡터 인덱스가 아니라 Rust 쪽 cosine similarity 계산으로 수행한다.

## 7. 큐와 상태 전이

큐 category는 다음 네 가지다.

- `ffmpeg`
- `stt`
- `llm`
- `embed`

동작 원칙:

- 한 시점에는 active batch 하나만 실행
- 같은 category는 batch에 병합
- 다른 category가 기다리면 `burst_limit`만큼 처리 후 rotate
- queue 상태도 메타DB에 영속화

Job/Task 상태는 항상 메타DB와 함께 갱신된다.

## 8. HTTP API 표면

핵심 변경점:

- `GET /jobs`는 optional query `source_ref`를 지원한다.
- `/jobs/by-source`는 제거되었다.
- Job 응답에는 `source_ref`, `source_kind`, `source_content_sha256`가 포함된다.
- `job_dir`, `source_path`는 API에서 제거되었다.
- `/jobs/{job_id}/files`는 logical artifact 목록을 반환한다.
- `/jobs/{job_id}/files/{*file_name}`는 저장 위치를 숨기고 오디오/DB 텍스트를 합성해서 반환한다.

## 9. 웹 UI

웹 UI는 선택한 job에 대해 다음 정보를 보여준다.

- job id
- status
- started/finished timestamps
- source ref
- source kind
- source content hash
- probe 정보
- task 상태
- transcript/summary/embedding 상태
- logical artifact 목록

`job_dir` 같은 내부 저장 경로는 더 이상 UI에 표시하지 않는다.
