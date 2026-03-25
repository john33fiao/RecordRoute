# API TODO (2026-03-25 코드베이스 기준)

## 목적

이 문서는 `rust/src/server.rs`, `rust/src/app.rs`, `rust/src/index.rs` 기준으로

1. **이미 구현된 HTTP API 사실**
2. **아직 남아 있는 API 작업(TODO)**

를 분리해 정리한다.

---

## 1) 현재 구현된 API (확정)

## 라우트 목록

- `POST /server/ping`
- `GET /system/status`
- `POST /jobs`
- `GET /jobs`
- `GET /jobs/{job_id}`
- `GET /jobs/{job_id}/status`
- `POST /jobs/{job_id}/stt`
- `GET /jobs/{job_id}/stt`
- `GET /jobs/{job_id}/stt/texts`
- `GET /jobs/{job_id}/stt/texts/{transcript_id}`
- `POST /jobs/{job_id}/summary`
- `GET /jobs/{job_id}/summary`
- `GET /jobs/{job_id}/files`
- `GET /jobs/{job_id}/files/{*file_name}`

즉 현재는 ping-only 단계가 아니라, job/task/file 조회·실행 API까지 구현된 상태다.
다만 입력 진입점은 아직 서버 로컬 경로 기반이며, 동일 PC에서 동작하는 프론트엔드용 업로드 API는 없다.

## 핵심 동작 요약

### `POST /jobs`

- 요청: `{ "input_path": "..." }` (서버 로컬 파일 경로)
- 동작: ffmpeg 파이프라인 작업을 등록하고 필요 시 백그라운드 실행
- 응답 코드:
  - `200 OK`: 완료된 기존 작업 재사용(`reused=true`)
  - `202 Accepted`: 신규 제출 또는 실행 중 작업 deduplicate

### `POST /jobs/{job_id}/stt`

- 요청: `{ "audio_files": [...], "mono_mix_only": false }`
  - `mono_mix_only=true` 이면 내부적으로 `mono_mix.wav`만 대상으로 처리
  - `audio_files`와 `mono_mix_only=true` 동시 사용은 `400`
- 동작: STT task 제출/재사용/deduplicate 후 필요 시 백그라운드 실행
- 응답 코드: 항상 `202 Accepted` (메시지/플래그로 reused·deduplicated 구분)

### `POST /jobs/{job_id}/summary`

- 요청: `{ "force_regenerate": false }` (body 없으면 false 취급)
- 동작: summary task 제출/재사용/deduplicate 후 필요 시 백그라운드 실행
- 응답 코드: 항상 `202 Accepted`

### `GET /jobs/{job_id}/stt`, `GET /jobs/{job_id}/summary`

- **본문 데이터가 아니라 task 상태 조회 API**
- 응답은 task 존재 여부/상태(`running|completed|failed`) 중심

### `GET /jobs/{job_id}/files`, `GET /jobs/{job_id}/files/{*file_name}`

- 파일 목록/다운로드 제공
- 허용 파일 경로 제한:
  - 루트: `mono_mix.wav`, `channel_*.wav`
  - 하위: `stt/*`, `summary/*`
- `..`, 절대경로, 백슬래시 경로는 차단

### `GET /system/status`

- 점검 필드 제공:
  - `ffmpeg_available`
  - `whisper_available`
  - `llama_available`
  - `whisper_model_ready`
  - `llama_model_ready`
  - `errors[]`

---

## 2) 상태/중복 처리 모델 (현재 구현과 일치)

- 상태 저장 SoT: `db/index.json` (`IndexStore`)
- Job 상태: `running`, `completed`, `failed`
- Task 타입: `ffmpeg`, `stt`, `summary`
- Task 상태: `running`, `completed`, `failed`

### deduplicate / reuse

- ffmpeg
  - 동일 `source_path` 완료 job + 산출물 유효 시 `reused`
  - 동일 `source_path` 실행 중 job 있으면 `deduplicated`
- stt
  - 동일 task 실행 중이면 `deduplicated`
  - 대상 transcript가 이미 있으면 `reused`
- summary
  - 동일 task 실행 중이면 `deduplicated`
  - summary 파일이 있고 `force_regenerate=false`면 `reused`

---

## 3) 미구현 TODO (우선순위)

## P0 (운영/클라이언트 사용성에 즉시 필요)

1. 파일 업로드 기반 job 생성 API
   - `POST /jobs/upload`
   - 요청: `multipart/form-data` (`file`)
   - 목적: 동일 PC에서 동작하는 웹 프론트엔드/Electron renderer가 선택한 오디오 파일을 localhost 서버에 제출
   - 동작:
     - 서버 관리 업로드 디렉터리에 원본 파일 저장
     - 내부적으로 기존 ffmpeg job 제출/실행 로직 재사용
     - 기존 `POST /jobs`는 CLI 또는 Electron main process 같은 path 접근 가능한 로컬 진입점으로 유지 가능
   - 응답 코드:
     - `200 OK`: 동일 파일의 기존 완료 결과 재사용
     - `202 Accepted`: 신규 제출 또는 실행 중 작업 deduplicate
   - 설계 메모:
     - 이 API는 원격 노드 간 파일 전송용이 아니라 localhost 단일 머신 배포를 위한 UI 진입점이다
     - 멀티 노드 실행, 외부 스토리지, signed URL 연계는 현재 범위 밖이다
     - 브라우저 기반 입력에서는 원본 절대경로를 신뢰하기 어려우므로, reuse key는 업로드 임시 경로 대신 별도 기준을 검토해야 한다

2. Summary 본문 조회 API
   - `GET /jobs/{job_id}/summary/text`
   - 목적: `summary/result.txt` 내용을 JSON으로 조회

## P1 (운영 자동화)

4. 모델 준비 API
   - `POST /models/whisper/prepare`
   - `POST /models/llama/prepare`
   - 목적: CLI 의존 없이 서버에서 모델 준비 작업 트리거

## P2 (확장/성능)

5. 조회 편의 API
   - `GET /jobs/completed`
   - `GET /jobs/by-source?source_path=...`

6. 파일 제공 전략 고도화
   - 대용량 응답 최적화(스트리밍/Range 등)
   - 동일 PC 로컬 앱 기준 파일 복사/열람 UX 최적화 검토
   - 외부 스토리지/서명 URL 연계는 현재 범위 밖

7. 상태 모델 고도화
   - task progress/phase 필드 확장
   - 워커 큐 기반 비동기 실행 모델로 확장 가능성 검토

---

## 4) 체크리스트

### 구현 완료

- [x] ffmpeg job 생성/조회 API
- [x] STT task 제출/상태 조회 API
- [x] Summary task 제출/상태 조회 API
- [x] job별 파일 목록/다운로드 API
- [x] 시스템/모델 가용성 상태 조회 API (`GET /system/status`)
- [x] job/task 상태 구조를 `db/index.json`에 일관 저장

### 미완료

- [ ] 파일 업로드 기반 job 생성 API (`POST /jobs/upload`)
- [x] STT 텍스트 본문 JSON 조회 (`GET /jobs/{job_id}/stt/texts`, `GET /jobs/{job_id}/stt/texts/{transcript_id}`)
- [ ] Summary 텍스트 본문 JSON 조회
- [ ] 모델 준비 HTTP API
- [ ] 완료 job/원본 경로 기반 조회 편의 API
- [ ] 대용량 파일 전달 최적화

---

## 5) 설계 메모

- 배포 가정은 **프론트엔드와 서버가 동일 PC에서 함께 동작하는 단일 머신 구성**이다.
- 현재 공개 입력 모델은 업로드가 아니라 **서버 로컬 경로(`input_path`) 기반**이다.
- 웹 프론트엔드/Electron renderer에서 파일 선택 UI를 쓰려면 `multipart/form-data` 기반 업로드 API가 별도로 필요하다.
- API/CLI는 동일 도메인 로직(`app.rs`, `index.rs`)을 공유한다.
- 향후 API 추가 시에도 `IndexStore`를 단일 상태 SoT로 유지하고,
  reuse/deduplicate 규칙을 깨지 않도록 우선 검증해야 한다.
- 업로드 API는 localhost UI 진입점으로 한정하고, 멀티 노드/원격 워커/외부 스토리지 전제는 두지 않는다.
- 특히 업로드 API는 임시 저장 경로가 매번 달라질 수 있으므로,
  기존 `source_path` 중심 재사용 규칙을 그대로 복사하지 말고 별도 식별 기준을 설계해야 한다.
