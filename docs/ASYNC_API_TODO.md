# Async API TODO

## 목적

이번 1차 전환은 FFmpeg 작업만 비동기 job API로 분리했다.  
STT, summary, 파일 다운로드, 실시간 알림은 후속 단계로 남겨 둔다.

## 다음 단계

### 1. STT 비동기 job

- [x] `POST /jobs/{job_id}/stt`를 즉시 `202 Accepted`로 전환
- [x] transcript 생성 상태를 `running | completed | failed`로 조회
- [x] 기존 `stt/` 산출물 재사용 여부를 응답에 포함
- [x] 특정 오디오 파일 subset 실행 지원 (`audio_files`)

### 2. Summary 비동기 job

- [x] `POST /jobs/{job_id}/summary`를 즉시 `202 Accepted`로 전환
- [x] 기존 summary 재사용과 `force_regenerate` 정책 정리
- [x] summary 생성 실패 시 에러 메시지와 마지막 실행 시각 기록

### 3. 공통 task 모델

- [x] FFmpeg, STT, summary를 공통 `task_type`과 상태 모델로 통합
- [x] `job_id` 외에 단계별 `task_id` 도입
- [x] 단계별 진행 시각, 마지막 에러, 재시도 횟수 저장 구조 반영

### 4. 파일 제공 계층

- [x] 산출물 경로만 노출하지 말고 다운로드 API 추가
- [x] `GET /jobs/{job_id}/files`
- [x] `GET /jobs/{job_id}/files/{file_name}`
- [x] 경로 탈출 방지와 허용 파일 범위 검증

### 5. 실시간 알림 검토

- [x] 현재는 polling만 지원
- [x] SSE 또는 websocket 도입 필요성 1차 검토 완료
- [x] 알림 채널 도입 시 상태 저장소와 이벤트 순서 보장 방식 정리

#### 실시간 알림 1차 결론

- 현 구조(단일 프로세스 + 파일 인덱스 저장소)에서는 polling을 유지한다.
- SSE/WebSocket 도입은 외부 상태 저장소 또는 프로세스 간 이벤트 버스 도입과 함께 추진한다.
- 이벤트 순서 보장은 `task.started_at`, `task.finished_at`, `task_id`를 기준으로 정렬/중복 제거하는 방식이 적합하다.

## 현 시점에서 제외한 항목

- 업로드 API
- 다중 프로세스 worker
- 외부 큐
- job 취소
- 진행률 퍼센트
