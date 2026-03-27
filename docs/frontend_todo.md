# Frontend Rust 연동 TODO

## 목적
- `frontend/`의 UI를 재사용하되, API 계약은 현재 Rust 서버 구현에 맞춘다.
- 기준 문서는 `rust/src/server.rs`, `rust/src/server/routes/*`, `rust/src/server/types.rs`, `docs/openapi.yaml`이다.
- 이번 정리는 "프론트가 무엇을 바꿔야 하는가"에 초점을 둔다.

## 한줄 결론
- 현재 `frontend/`는 별도 레거시 백엔드 계약을 전제로 작성되어 있어서, `frontend/src/api/client.ts`와 관련 상태/타입 계층을 Rust API 기준으로 사실상 다시 맞춰야 한다.
- 1차 연동 범위는 `오디오 업로드 -> ffmpeg 완료 -> STT -> summary -> 결과 조회/검색`으로 제한하는 것이 맞다.
- 그래프, 유사 문서, 삭제/초기화/취소, 이름 수정, 세그먼트 편집, 모델 선택/종료 같은 기능은 현행 Rust API와 직접 매핑되지 않으므로 숨기거나 후속 과제로 분리해야 한다.

## 현행 Rust API 기준 정리

### 사용 가능한 핵심 API
- 시스템/모델
  - `GET /system/status`
  - `GET /models/status`
  - `POST /models/whisper/prepare`
  - `POST /models/llama/prepare`
- Job
  - `POST /jobs`
  - `POST /jobs/upload`
  - `GET /jobs`
  - `GET /jobs/completed`
  - `GET /jobs/by-source`
  - `GET /jobs/{job_id}`
  - `GET /jobs/{job_id}/status`
- STT
  - `POST /jobs/{job_id}/stt`
  - `GET /jobs/{job_id}/stt`
  - `GET /jobs/{job_id}/stt/progress`
  - `GET /jobs/{job_id}/stt/texts`
  - `GET /jobs/{job_id}/stt/texts/{transcript_id}`
- Summary / Embedding / Search
  - `POST /jobs/{job_id}/summary`
  - `GET /jobs/{job_id}/summary`
  - `POST /jobs/{job_id}/summary/embedding`
  - `GET /jobs/{job_id}/summary/embedding`
  - `GET /jobs/{job_id}/summary/text`
  - `POST /summary/search`
- 파일
  - `GET /jobs/{job_id}/files`
  - `GET /jobs/{job_id}/files/{file_name}`

### 현재 없는 API
- `/upload`
- `/process`
- `/history`
- `/tasks`
- `/progress/{task_id}`
- `/ws`
- `/segments/{fileIdentifier}`
- `/search` (GET 기반 혼합 검색)
- `/api/similarity-graph`
- `/similar`
- `/delete`, `/delete_records`
- `/reset`, `/reset_all_tasks`, `/reset_summary_embedding`
- `/update_filename`
- `/models` (모델 목록 조회)
- `/shutdown`

## 프론트와 Rust 간 핵심 차이

| 주제 | 현재 frontend 가정 | 현행 Rust 동작 | 조치 |
| --- | --- | --- | --- |
| 업로드 엔드포인트 | `POST /upload`, 다중 `files`, 응답 배열 | `POST /jobs/upload`, 단일 `file`, 응답 단일 `JobSubmissionResponse` | 파일별 개별 업로드로 변경 |
| 식별자 | `record_id`, `fileIdentifier`, `download/{id}` | 핵심 식별자는 `job_id`, STT는 `transcript_id`, 파일은 `job_id + file_name` | 프론트 식별자 체계 전면 교체 |
| 처리 실행 | `POST /process` 하나로 여러 단계 처리 | ffmpeg/STT/summary/embedding이 각기 다른 엔드포인트 | 큐 로직 재작성 |
| 진행률 | WebSocket `/ws` + `/progress/{task_id}` | WebSocket 없음, STT만 `GET /jobs/{job_id}/stt/progress` 제공 | 폴링 기반으로 단순화 |
| 이력 | `/history` 전용 응답 | `GET /jobs`가 `JobRecord[]` 제공 | JobRecord -> 화면 ViewModel 변환 필요 |
| STT 표시 | 세그먼트/화자 정보 사용 | 텍스트 파일 목록만 제공, 세그먼트 API 없음 | 화자 필터/세그먼트 UI 제거 |
| 검색 | 키워드+벡터 혼합 검색, 필터/페이지/타이밍 포함 | `POST /summary/search`만 제공 | 의미 검색 단일 화면으로 축소 |
| 그래프/유사문서 | 전용 API 존재 가정 | 관련 API 없음 | 탭/다이얼로그 비활성화 |
| 설정 | 모델 목록/프로바이더 선택, 서버 종료 | 모델 상태 조회 및 준비만 가능 | 상태/준비 패널로 재구성 |
| 파괴적 기능 | 삭제/초기화/취소/이름 수정 가능 | 대응 API 없음 | 버튼 제거 또는 숨김 |

## 반드시 반영해야 할 TODO

### 1. API 클라이언트 재작성
대상:
- `frontend/src/api/client.ts`
- `frontend/src/api/types.ts`

작업:
- 레거시 엔드포인트 호출을 모두 Rust API로 교체한다.
- Rust 응답 타입을 직접 정의한다.
  - `JobRecord`
  - `JobSubmissionResponse`
  - `TaskSubmissionResponse`
  - `SttProgressResponse`
  - `SummaryTextResponse`
  - `SummarySearchResponse`
  - `ModelStatusResponse`
- 프론트 전용 화면 모델은 Rust 응답을 그대로 쓰지 말고 어댑터 함수로 변환한다.

메모:
- `record_id` 대신 `job_id`를 기본 식별자로 사용한다.
- `embedding`은 일반 문서 색인이 아니라 `summary embedding`으로 해석해야 한다.

### 2. 업로드 플로우를 Rust 방식으로 변경
대상:
- `frontend/src/components/UploadSection.tsx`

작업:
- 업로드 허용 확장자를 1차적으로 오디오 위주로 축소한다.
  - 현재 프론트가 허용하는 `pdf/md/txt`는 현행 Rust ffmpeg 잡과 직접 맞지 않는다.
- 다중 파일 선택은 유지하되, 내부적으로는 파일별 `POST /jobs/upload`를 반복 호출한다.
- multipart 필드명은 `files`가 아니라 `file`로 맞춘다.
- 업로드 직후 응답의 `job_id`, `status`, `source_path`, `source_file_name`을 기준으로 후속 큐를 구성한다.

주의:
- 현행 Rust `/jobs/upload`는 업로드 원본 파일명을 보존하지 않고 `db/uploads/{hash}.bin` 경로로 저장한다.
- 따라서 프론트만 수정해서는 "사용자가 올린 원래 파일명"을 이력에서 안정적으로 보여주기 어렵다.
- 이 부분은 아래 "결정 필요 항목"의 최우선 이슈다.

### 3. 큐 로직을 `job_id + task_type` 기준으로 재설계
대상:
- `frontend/src/hooks/useTaskQueue.ts`
- `frontend/src/components/JobQueue.tsx`
- `frontend/src/components/TaskActionControls.tsx`

작업:
- 현재 `/process` 단일 호출 모델을 버리고 단계별 호출로 변경한다.
- 권장 순서:
  1. 업로드 또는 `POST /jobs`
  2. ffmpeg 완료 대기
  3. `POST /jobs/{job_id}/stt`
  4. `POST /jobs/{job_id}/summary`
  5. 필요 시 `POST /jobs/{job_id}/summary/embedding`
- STT는 `job.status == completed` 이후에만 제출 가능하다.
- summary는 STT 산출물이 있어야 정상 동작한다.
- progress 갱신은 다음 방식으로 단순화한다.
  - 공통 상태: `GET /jobs/{job_id}/status`
  - STT 진행률: `GET /jobs/{job_id}/stt/progress`

주의:
- Rust에는 취소 API가 없다.
- 따라서 현재 큐의 `취소`, `모두 취소`, `AbortController` 기반 UX는 서버 취소가 아니라 프론트 대기열 제거 수준으로 축소해야 한다.
- 현재 프론트의 `upload -> transform -> correct -> summary` 단계 표시는 Rust 파이프라인과 맞지 않는다.
- Rust 기준 단계는 `ffmpeg -> stt -> summary -> embedding`으로 다시 잡아야 한다.

### 4. 이력 화면을 `GET /jobs` 기반으로 다시 구성
대상:
- `frontend/src/hooks/useHistory.ts`
- `frontend/src/components/HistoryPanel.tsx`
- `frontend/src/components/HistoryListItem.tsx`

작업:
- `/history` 대신 `GET /jobs` 또는 필요 시 `GET /jobs/completed`를 사용한다.
- `JobRecord`에서 화면용 상태를 계산한다.
  - `id` -> `job_id`
  - `filename` -> 일단 `source_file_name`
  - `timestamp` -> `started_at`
  - `completed_tasks` -> `tasks` 배열을 보고 계산
  - `download_links` -> 정적 링크가 아니라 `job_id + file_name` 조합으로 생성
- 현재 이력 카드의 "STT / 색인 / 요약" 버튼은 Rust 상태에 맞춰 활성/완료 여부를 계산한다.

주의:
- 원본 파일 다운로드 링크를 지금처럼 `file_path`로 직접 만들 수 없다.
- 현행 Rust의 파일 다운로드는 job 하위 결과물만 허용한다.
- 따라서 이력 카드의 다운로드 버튼은 "원본 파일"이 아니라 "결과 파일" 기준으로 다시 정의해야 한다.

### 5. 결과 뷰어를 job 기반 조회로 변경
대상:
- `frontend/src/components/TextOverlay.tsx`
- `frontend/src/hooks/app/useViewerState.ts`

작업:
- 식별자를 단일 `fileIdentifier`로 다루지 말고, 최소한 아래 정보로 분리한다.
  - `jobId`
  - `viewType` (`stt` or `summary`)
  - 필요 시 `transcriptId`
- summary 보기:
  - `GET /jobs/{job_id}/summary/text`
- STT 보기:
  - `GET /jobs/{job_id}/stt/texts`
  - 필요하면 `GET /jobs/{job_id}/stt/texts/{transcript_id}`
- 다운로드 버튼:
  - `GET /jobs/{job_id}/files/{file_name}`

주의:
- 현행 Rust에는 세그먼트, 타임코드, 화자 분리 API가 없다.
- 현재 `TextOverlay`의 화자 필터, 묶음 보기, 세그먼트 렌더링은 제거해야 한다.
- STT 편집 저장, 결과 삭제 기능도 대응 API가 없으므로 비활성화해야 한다.

### 6. 검색 화면을 `summary/search` 기반으로 축소
대상:
- `frontend/src/components/SearchPanel.tsx`

작업:
- 현재 GET `/search` 기반 파라미터와 응답 구조를 제거한다.
- `POST /summary/search`에 맞춰 요청 바디를 단순화한다.
  - `query`
  - `limit`
  - `min_score`
- 응답의 `results[]`에서 `job_id`, `source_file_name`, `summary_excerpt`, `score`를 사용한다.
- 상세 보기 클릭 시 summary 텍스트를 열도록 연결한다.

주의:
- 날짜 필터, 파일 타입 필터, status 필터, pagination, timing, cache, keyword/similar 분리 UI는 현행 Rust API에 없다.
- 1차 연동에서는 검색 화면을 "요약 의미 검색" 단일 목록으로 축소하는 것이 맞다.

### 7. 설정 화면을 상태/준비 화면으로 변경
대상:
- `frontend/src/components/SettingsDialog.tsx`
- `frontend/src/hooks/useModelSettings.ts`

작업:
- `/models` 호출과 모델 선택 UI를 제거한다.
- 대신 아래를 보여준다.
  - `GET /system/status`
  - `GET /models/status`
  - `POST /models/whisper/prepare`
  - `POST /models/llama/prepare`
- theme 저장은 유지 가능하다.

주의:
- 현행 Rust는 요청별 `model_settings`를 받지 않는다.
- 현재 프론트의 `whisper/summarize/embedding/provider/language` 설정은 서버 환경 변수/모델 준비 흐름과 맞지 않는다.
- `shutdown` 기능도 대응 API가 없으므로 제거해야 한다.

### 8. 숨기거나 제거해야 하는 화면/기능
대상:
- `frontend/src/components/SimilarityGraphPanel.tsx`
- `frontend/src/components/SimilarDocsDialog.tsx`
- `frontend/src/components/ConfirmDialogs.tsx`
- `frontend/src/hooks/useWebSocket.ts`

작업:
- 그래프 탭 제거 또는 feature flag 처리
- 유사 문서 다이얼로그 제거
- 전체 초기화 / STT 수정 후 reset 다이얼로그 제거
- WebSocket 진행률 코드 제거
- delete / reset / rename / cancel 관련 버튼 제거

## 권장 구현 순서
1. `api/types.ts`, `api/client.ts`를 Rust 계약 기준으로 먼저 정리한다.
2. 업로드 화면을 `POST /jobs/upload` 기준으로 바꾸고 오디오만 처리한다.
3. 이력 화면을 `GET /jobs` 기반으로 재구성한다.
4. 결과 뷰어를 `job_id` 기반 조회로 바꾼다.
5. 큐 로직을 단계별 API 호출과 폴링 방식으로 교체한다.
6. 검색 화면을 `POST /summary/search` 기준으로 단순화한다.
7. 설정 화면을 모델 상태/준비 화면으로 바꾼다.
8. 그래프/유사문서/파괴적 기능을 숨기고 잔여 코드를 정리한다.

## 결정 필요 항목

### 1. 업로드 원본 파일명 보존 문제
- 현재 `/jobs/upload`는 multipart에서 파일 바이트만 저장하고 원본 파일명을 잃는다.
- 프론트만 수정하면 이력에 해시 기반 `.bin` 이름이 노출될 가능성이 높다.
- 선택지:
  - 프론트 1차 연동에서는 로컬에서 선택한 파일명을 임시 표시하고 새로고침 후 이름이 바뀌는 것을 감수한다.
  - Rust 업로드 API를 확장해 원본 파일명을 보존한다.

권장:
- UX를 생각하면 이 항목은 프론트 작업 전에 짧게라도 Rust 보완 여부를 결정하는 것이 좋다.

### 2. 비오디오 문서 업로드 지원 범위
- 현재 프론트는 `pdf/md/txt`를 받지만 Rust 파이프라인은 ffmpeg 중심 job 생성 흐름이다.
- 1차 연동에서는 문서 업로드를 막고 오디오만 지원하는 편이 안전하다.

### 3. embedding 버튼 유지 여부
- Rust summary 실행 성공 시 embedding이 자동 실행될 수 있다.
- 다만 기존 summary 재사용 케이스에서는 embedding이 비어 있을 수 있다.
- 따라서 1차 연동에서는 다음 중 하나를 선택해야 한다.
  - 색인 버튼을 유지하되 의미를 `summary embedding`으로 명확히 바꾼다.
  - summary 성공 후 embedding 상태를 별도 보정 호출로 처리한다.

## 검증 시나리오
1. 오디오 파일 1개 업로드 후 `GET /jobs`에서 새 job이 보인다.
2. ffmpeg 완료 전에는 STT 제출이 막히거나 대기 처리된다.
3. STT 완료 후 transcript 목록이 열리고 텍스트가 표시된다.
4. summary 완료 후 summary 텍스트가 열리고 다운로드 가능하다.
5. summary embedding이 준비된 job은 `POST /summary/search` 결과에 포함된다.
6. 그래프/유사문서/삭제/이름수정 버튼이 화면에서 사라지거나 비활성화된다.

## 메모
- `frontend/legacy/`와 현재 React 프론트 모두 같은 레거시 API 전제를 공유한다.
- 따라서 부분 수정이 아니라 "현행 Rust API용 어댑터 + 화면 축소" 관점으로 보는 것이 맞다.
