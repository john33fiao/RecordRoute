# API TODO (2026-03-25 기준 최신화)

## 목적

이 문서는 **현재 코드베이스 기준으로 이미 구현된 HTTP API**와, 아직 남아 있는 API 작업을 분리해서 정리한다.

핵심 원칙:

1. `db/index.json`(`IndexStore`)를 단일 상태 저장소로 유지한다.
2. API와 CLI는 동일한 도메인 로직(`app.rs`, `index.rs`)을 공유한다.
3. 중복 요청 deduplicate / 완료 결과 재사용(reuse) 동작을 깨지 않는다.

---

## 현재 구현 상태 (사실 기준)

### 서버 라우트

현재 서버(axum)는 다음 라우트를 제공한다.

- `POST /server/ping`
- `POST /jobs`
- `GET /jobs`
- `GET /jobs/{job_id}`
- `GET /jobs/{job_id}/status`
- `POST /jobs/{job_id}/stt`
- `GET /jobs/{job_id}/stt`
- `POST /jobs/{job_id}/summary`
- `GET /jobs/{job_id}/summary`
- `GET /jobs/{job_id}/files`
- `GET /jobs/{job_id}/files/{*file_name}`

즉, 과거 문서의 "ping만 구현" 상태는 더 이상 유효하지 않다.

### 작업 실행 모델

- `POST /jobs`는 요청을 수락한 뒤 백그라운드에서 ffmpeg 작업을 실행한다.
  - 신규 실행이면 `202 Accepted`
  - 재사용이면 `200 OK`
- `POST /jobs/{job_id}/stt`, `POST /jobs/{job_id}/summary`도 백그라운드 실행 + `202 Accepted` 모델이다.

### deduplicate / reuse

- ffmpeg:
  - 같은 `source_path`의 완료 job + 유효 산출물 존재 시 `reused`
  - 같은 `source_path`의 실행중 job 존재 시 `deduplicated`
- stt:
  - 동일 task 실행중이면 `deduplicated`
  - 대상 transcript가 이미 모두 존재하면 `reused`
- summary:
  - 동일 task 실행중이면 `deduplicated`
  - summary 파일이 이미 있고 `force_regenerate=false`면 `reused`

### 상태 저장

`JobRecord`는 다음을 보관한다.

- ffmpeg job 상태: `status` (`running`/`completed`/`failed`)
- 작업 시간: `started_at`, `finished_at`
- 입력/출력 메타: `source_path`, `source_file_name`, `probe`, `outputs`
- 오류: `error_message`
- 단계별 task 상태: `tasks[]` (`ffmpeg`/`stt`/`summary`, `running`/`completed`/`failed`, `last_error`, `retry_count`)

---

## 현재 API와 사용자 최소 시나리오 매핑

사용자 시나리오 관점에서 이미 가능한 항목:

1. 오디오 경로 전달 후 ffmpeg 작업 시작
   - `POST /jobs`
2. 기존 완료 job 재사용
   - `POST /jobs` 응답의 `reused`
3. ffmpeg 완료/진행 포함 전체 job 조회
   - `GET /jobs`, `GET /jobs/{job_id}`
4. 특정 job STT 시작
   - `POST /jobs/{job_id}/stt`
5. 특정 job summary 시작
   - `POST /jobs/{job_id}/summary`
6. 산출물 파일 목록/다운로드
   - `GET /jobs/{job_id}/files`
   - `GET /jobs/{job_id}/files/{*file_name}`

보완이 필요한 항목:

- STT/summary 본문 전용 조회 API 부재(현재는 파일 다운로드 기반)
- 시스템/모델 상태 조회 API 부재

---

## 미구현 TODO (우선순위 최신화)

### P0 (먼저)

1. [x] `GET /system/status`
   - 목적: 운영 상태 점검
   - 응답 권장 필드:
     - `ffmpeg_available`
     - `whisper_available`
     - `llama_available`
     - `whisper_model_ready`
     - `llama_model_ready`
     - `errors[]`

### P1 (다음)

2. STT 본문 조회 API
   - `GET /jobs/{job_id}/stt/texts`
   - `GET /jobs/{job_id}/stt/texts/{transcript_id}`

3. Summary 본문 조회 API
   - `GET /jobs/{job_id}/summary/text`

4. 모델 준비 API (HTTP 노출)
   - `POST /models/whisper/prepare`
   - `POST /models/llama/prepare`

### P2 (확장)

5. 조회 편의 API
   - `GET /jobs/completed`
   - `GET /jobs/by-source?source_path=...`

6. 파일 제공 전략 고도화
   - 대용량 파일 응답 최적화
   - 필요 시 signed URL / 외부 스토리지 매핑

7. 상태 모델 고도화
   - task 세부 단계(progress) 도입
   - 필요 시 비동기 job 큐(워커)로 확장

---

## 설계 메모 (현재 코드와 맞춘 제약)

1. 입력은 현재 `input_path`(서버 로컬 파일 경로) 기반이다.
   - 업로드 API는 아직 없다.

2. 현재도 비동기 UX(`accepted`)를 사용 중이다.
   - 따라서 상태 조회 API를 먼저 강화하는 것이 맞다.

3. API에서 `job_id` 중심 흐름은 이미 적용되었다.
   - 기존 CLI의 번호 선택 인터랙션은 API 경로에 직접 반영되지 않는다.

4. 실패/재시도 정보는 `tasks[]`와 `error_message`를 기준으로 표준화한다.

---

## 최종 체크리스트 (현행)

### 이미 충족

- [x] 오디오 파일 경로로 ffmpeg 작업 시작
- [x] 기존 완료 job 재사용
- [x] 특정 job STT 시작
- [x] 특정 job summary 시작
- [x] 산출물 파일 목록 조회
- [x] 산출물 파일 다운로드

### 추가 필요

- [x] 단계 통합 상태 조회 (`GET /jobs/{job_id}/status`)
- [ ] STT 텍스트 본문 조회
- [ ] summary 텍스트 본문 조회
- [x] 시스템/모델 상태 조회
- [ ] 모델 준비 API
- [ ] 완료 job/원본 기준 조회 편의 API

---

## 이번 최신화에서 정리한 결론

- 현재 RecordRoute는 이미 "ping-only" 단계가 아니다.
- 기본 Job/STT/Summary/File API는 동작 중이며, 실질적으로 1차 API 골격이 마련되어 있다.
- 다음 작업의 핵심은 **상태/본문/운영 API 보강**이다.
