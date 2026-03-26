# API TODO (2026-03-26 코드베이스 점검 반영)

## 목적

이 문서는 `rust/src/server.rs`, `rust/src/app.rs`, `rust/src/index.rs`, `docs/openapi.yaml` 기준으로

1. **현재 서버에 실제 구현된 HTTP API**
2. **아직 남아 있는 API 확장 작업**

을 분리해 관리한다.

---

## 1) 현재 구현된 API (코드 기준 확정)

### 라우트 목록

- `POST /server/ping`
- `GET /system/status`
- `GET /models/status`
- `POST /models/whisper/prepare`
- `POST /models/llama/prepare`
- `POST /jobs`
- `GET /jobs`
- `GET /jobs/completed`
- `GET /jobs/by-source?source_path=...`
- `POST /jobs/upload`
- `GET /jobs/{job_id}`
- `GET /jobs/{job_id}/status`
- `POST /jobs/{job_id}/stt`
- `GET /jobs/{job_id}/stt`
- `GET /jobs/{job_id}/stt/texts`
- `GET /jobs/{job_id}/stt/texts/{transcript_id}`
- `POST /jobs/{job_id}/summary`
- `GET /jobs/{job_id}/summary`
- `GET /jobs/{job_id}/summary/text`
- `GET /jobs/{job_id}/files`
- `GET /jobs/{job_id}/files/{*file_name}`

> 결론: 현재 API는 ffmpeg/stt/summary 제출 + 모델 준비/상태조회 + 산출물 조회 + 파일 업로드 진입점까지 포함한다.

### 핵심 동작 요약

#### `POST /jobs`
- 요청: `{ "input_path": "..." }`
- 의미: 서버에서 접근 가능한 로컬 파일 경로 기반 ffmpeg job 제출
- 응답:
  - `200 OK`: 완료된 기존 job 재사용(`reused=true`)
  - `202 Accepted`: 신규 제출 또는 실행중 job deduplicate

#### `POST /jobs/upload`
- 요청: `multipart/form-data` (`file` 필드 1개 필수)
- 의미: 업로드 바이트를 `db/uploads/<content_hash>.bin`으로 저장한 뒤 ffmpeg job 제출
- 응답:
  - `200 OK`: 동일 콘텐츠 기반 완료 job 재사용
  - `202 Accepted`: 신규 제출 또는 실행중 job deduplicate

#### `POST /jobs/{job_id}/stt`
- 요청: `{ "audio_files": [...], "mono_mix_only": false }`
- 규칙:
  - `mono_mix_only=true`면 내부적으로 `mono_mix.wav`만 대상
  - `audio_files` + `mono_mix_only=true` 동시 지정 시 `400`
- 응답: `202 Accepted` (message/reused/deduplicated로 세부 상태 전달)

#### `POST /jobs/{job_id}/summary`
- 요청: `{ "force_regenerate": false }` (body 파싱 실패/미제공 시 false 취급)
- 응답: `202 Accepted` (message/reused/deduplicated로 세부 상태 전달)

#### 상태 조회 API
- `GET /jobs/{job_id}/stt`, `GET /jobs/{job_id}/summary`
- 본문 산출물이 아닌 task 레코드 상태 조회 용도 (`running|completed|failed`)

#### 본문 조회 API
- `GET /jobs/{job_id}/stt/texts`
- `GET /jobs/{job_id}/stt/texts/{transcript_id}`
- `GET /jobs/{job_id}/summary/text`

#### 파일 조회 API
- `GET /jobs/{job_id}/files`, `GET /jobs/{job_id}/files/{*file_name}`
- 허용 경로 제한:
  - 루트: `mono_mix.wav`, `channel_*.wav`
  - 하위: `stt/*`, `summary/*`
- 보안 제한: 절대경로, `..`, 역슬래시 포함 경로 차단

#### 시스템 점검 API
- `GET /system/status`
- 필드:
  - `ffmpeg_available`, `whisper_available`, `llama_available`
  - `whisper_model_ready`, `llama_model_ready`
  - `errors[]`

#### 모델 준비/상세 상태 API
- `GET /models/status`
  - 모델별 `available`, `ready`, `error`, `preparation(status/started_at/finished_at/heartbeat_at/last_error)` 반환
- `POST /models/whisper/prepare`, `POST /models/llama/prepare`
  - `200 OK`: 이미 준비된 모델 확인 (`already_ready=true`)
  - `202 Accepted`: 신규 준비 시작 또는 실행중 preparation deduplicate
  - 준비 상태는 `db/index.json`의 `model_preparations`에 저장되고, 런타임 자동 준비/CLI와 공유된다.

---

## 2) 상태/중복처리 모델 (현행)

- SoT: `db/index.json` (`IndexStore`, `db/index.lock` 사용)
- Job 상태: `running | completed | failed`
- Task 타입: `ffmpeg | stt | summary`
- Task 상태: `running | completed | failed`
- Model preparation 상태: `idle | running | completed | failed`

### deduplicate / reuse 규칙

- ffmpeg
  - 동일 `source_path` 완료 job + 산출물 유효 시 `reused`
  - 동일 `source_path` 실행중 job 있으면 `deduplicated`
- stt
  - 동일 task 실행중이면 `deduplicated`
  - 요청 subset 기준 transcript 산출물이 이미 있으면 `reused`
- summary
  - 동일 task 실행중이면 `deduplicated`
  - `summary/result.txt`가 있고 `force_regenerate=false`면 `reused`

---

## 3) 미구현 TODO (우선순위)

### P2 (조회 편의 / 확장)

1. 파일 제공 전략 고도화
   - 대용량 파일 스트리밍/Range 지원
   - 로컬 앱 연동 UX(복사/열람) 최적화

2. Whisper 진행률 조회 API
   - `GET /jobs/{job_id}/stt/progress`
   - 비고: 우선 whisper 현재 진행률을 polling 가능한 형태로 응답

3. 상태 모델 고도화
   - task progress/phase 필드
   - 워커 큐 기반 비동기 실행 모델 확장 검토

---

## 4) 체크리스트

### 구현 완료

- [x] ffmpeg job 생성/조회 API
- [x] 업로드 기반 job 생성 API (`POST /jobs/upload`)
- [x] STT task 제출/상태 조회 API
- [x] Summary task 제출/상태 조회 API
- [x] STT 텍스트 본문 조회 API
- [x] Summary 텍스트 본문 조회 API
- [x] job별 파일 목록/다운로드 API
- [x] 시스템/모델 가용성 상태 조회 API (`GET /system/status`)
- [x] 모델 준비/상세 상태 API (`GET /models/status`, `POST /models/whisper/prepare`, `POST /models/llama/prepare`)
- [x] job/task/model preparation 상태를 `db/index.json`에 일관 저장

### 미완료

- [x] 완료 job / source_path 기반 전용 조회 API
- [ ] 대용량 파일 전달 최적화(스트리밍/Range)
- [ ] whisper 진행률 조회 API (`GET /jobs/{job_id}/stt/progress`)
- [ ] task progress/phase 모델 확장

---

## 5) 설계 메모

- 배포 가정은 **프론트엔드와 서버가 동일 PC에서 함께 동작하는 단일 머신 구성**이다.
- 입력 진입점은 **로컬 경로(`POST /jobs`) + 업로드(`POST /jobs/upload`)** 두 가지를 모두 지원한다.
- API/CLI는 동일 도메인 로직(`app.rs`, `index.rs`)을 공유한다.
- 모델 준비는 HTTP API, CLI(`prepare-llama-model`), STT/Summary 런타임 자동 준비가 동일한 preparation 상태/heartbeat/deduplicate 규칙을 공유한다.
- API 추가 시 `IndexStore`를 단일 상태 SoT로 유지하고 reuse/deduplicate 규칙을 우선 검증한다.
- 업로드 입력은 해시 기반 저장 경로를 사용하므로 path 기반 재사용 규칙과 구분해 관리한다.
