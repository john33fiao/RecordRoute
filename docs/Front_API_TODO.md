# Front API TODO

이 문서는 현재 `frontend/src` React UI를 그대로 유지하는 것을 전제로, Rust 백엔드에 추가로 필요한 API와 내부 작업을 정리한 TODO 문서다.

기준:

- 프론트의 화면/플로우/데이터 shape를 최대한 유지한다.
- 프론트의 현재 호출 경로(`/upload`, `/process`, `/history` 등)를 우선 유지한다.
- 기존 Rust 공개 API(`/v1/*`)는 코어 API로 남겨두고, 필요하면 프론트 호환용 compatibility layer를 추가한다.

관련 문서:

- 현재 Rust 공개 API: [API.md](./API.md)
- 현재 구현 구조: [architecture.md](./architecture.md)

## 1. 결론 요약

현재 React 프론트를 유지하려면 다음이 필요하다.

1. 프론트 호환용 추가 엔드포인트 다수
2. 단일 `recording/job` 모델을 넘는 task/progress 모델
3. 다운로드 가능한 artifact 식별자 체계
4. 더 넓은 업로드 포맷 지원 또는 호환 계층
5. 검색/유사도/그래프용 응답 shape 확장

중요:

- 이 작업은 "라우트 몇 개 추가" 수준이 아니다.
- 특히 `/process`, `/progress/:task_id`, `/ws`, `/cancel`, `/reset_*`는 현재 Rust 구현에 없는 task orchestration 계층이 필요하다.

## 2. 구현 전략

권장 전략은 다음과 같다.

### 전략 A: Compatibility API 추가

- 기존 `/v1/*`는 유지
- 프론트가 쓰는 루트 경로 API를 별도로 추가
- 내부적으로는 기존 repository/pipeline/storage를 최대한 재사용

장점:

- 프론트 수정 최소화
- 점진적 이행 가능
- 기존 `/v1/*` 문서와 소비자 계약 보존 가능

단점:

- API surface가 넓어진다
- 호환 응답 shape를 위한 adapter 코드가 필요하다

### 전략 B: 프론트 리라이트

이 문서 범위 밖이다. 본 문서는 전략 A를 전제로 한다.

## 3. 우선순위

### P0. 앱이 바로 깨지는 영역

- 업로드 계약
- 기록 목록 로드
- 개별 task 실행/진행률/취소
- 기본 검색

### P1. 현재 활성 탭/다이얼로그를 살리는 영역

- 텍스트 상세 보기
- 유사 문서
- 그래프
- 초기화/삭제
- 설정/모델 목록

### P2. 현재 React 코드에는 helper가 있으나 활성 UI 의존도가 낮은 영역

- `/tasks`
- `/reset`

## 4. 필수 엔드포인트 TODO

아래 항목들은 현재 프론트를 유지하려면 사실상 필요하다.

### 4.1 업로드

#### `POST /upload`

목적:

- 업로드 탭의 다중 파일 업로드를 그대로 지원

프론트 기대 요청:

- `multipart/form-data`
- 여러 파일을 같은 요청에 담아 전송

프론트 기대 응답:

```json
[
  {
    "file_path": "string",
    "file_type": "audio",
    "record_id": "uuid",
    "duplicate": false,
    "original_record_id": "uuid",
    "filename": "string"
  }
]
```

백엔드 TODO:

- 다중 파일 업로드 지원
- 현재 Rust가 허용하지 않는 포맷 검토 및 지원
  - 오디오: `flac`, `mp4`, `mpeg`, `mpga`, `oga`, `ogg`, `qta`, `webm`
  - 문서: `pdf`, `md`, `txt`, `text`, `markdown`
- duplicate 감지 정책 정의
- `file_type` 판별 규칙 정의
- 업로드 직후 자동 처리 여부와 `/process` 호출 관계 정리

메모:

- 현재 Rust `/v1/recordings`는 첫 파일 하나만 처리하므로 그대로는 재사용 불가
- 내부적으로는 파일 저장/recording 생성 로직 일부를 재사용 가능

### 4.2 기록 목록

#### `GET /history`

목적:

- 기록 탭의 기본 데이터 공급

프론트 기대 응답 핵심 shape:

```json
[
  {
    "id": "uuid",
    "filename": "string",
    "title_summary": "string",
    "file_type": "audio",
    "timestamp": "datetime",
    "file_hash": "string",
    "duration": "string",
    "file_path": "string",
    "completed_tasks": {
      "stt": true,
      "embedding": true,
      "summary": true
    },
    "download_links": {
      "stt": "/download/...",
      "summary": "/download/..."
    },
    "info": {}
  }
]
```

백엔드 TODO:

- 현재 `recordings` + artifact 존재 여부를 조합해 `completed_tasks` 계산
- `title_summary`를 summary artifact에서 파생
- `download_links` 생성 규칙 정의
- `file_hash`, `duration` 저장 또는 파생 정책 정리
- 파일 유형(`audio/document/other`) 저장 또는 판별

메모:

- 현재 `/v1/recordings` 응답은 목록 UI가 기대하는 shape와 다름
- read model 전용 DTO를 추가하는 편이 가장 현실적

### 4.3 파일명 수정

#### `POST /update_filename`

목적:

- 기록 탭 inline rename 지원

요청:

```json
{
  "record_id": "uuid",
  "filename": "new-name.wav"
}
```

백엔드 TODO:

- 원본 표시 이름 수정 정책 정의
- 실제 파일 rename 여부와 메타데이터만 수정할지 결정
- 검색 색인/summary/title 표시값 동기화 검토

## 5. Task/Queue 계층 TODO

이 구간이 현재 프론트와 Rust API의 가장 큰 차이다.

### 5.1 개별 task 실행

#### `POST /process`

목적:

- 기록 탭/작업 큐 탭에서 `stt`, `embedding`, `summary`를 개별로 실행

프론트 요청:

```json
{
  "file_path": "string",
  "steps": ["stt", "correct"],
  "record_id": "uuid",
  "task_id": "string",
  "model_settings": {
    "whisper": "string",
    "summarize": "string",
    "embedding": "string",
    "language": "string",
    "provider": "ollama"
  },
  "retry_mode": "new_task",
  "retry_of_task_id": "string"
}
```

프론트 기대 응답:

```json
{
  "accepted": true,
  "task_id": "string",
  "error": null,
  "failed_step": null,
  "retryable": true
}
```

백엔드 TODO:

- recording 단위 job 외에 "frontend task id"를 추적할 계층 추가
- 단계별 실행(`stt`만, `summary`만, `embedding`만) 지원
- `correct` 단계 의미 정의
- 기존 자동 파이프라인과 수동 `/process` 호출의 충돌 방지
- retry semantics 설계

### 5.2 진행률 조회

#### `GET /progress/:task_id`

목적:

- 작업 큐 탭 polling

프론트 기대 응답:

```json
{
  "task_id": "string",
  "message": "string",
  "stage": "transform",
  "progress_percent": 42,
  "eta_seconds": 18,
  "error_code": null,
  "retryable": true,
  "failed_step": null,
  "error": null
}
```

백엔드 TODO:

- progress state 저장소 추가
- stage mapping 정의
  - `upload`
  - `transform`
  - `correct`
  - `summary`
- ETA 계산은 초기에는 nullable 허용

### 5.3 실시간 이벤트

#### `GET /ws` 또는 WebSocket `/ws`

목적:

- 작업 큐 탭 실시간 progress push

백엔드 TODO:

- WebSocket broadcaster 추가
- 최소 payload:
  - `task_id`
  - `message`
  - `stage`
  - `progress_percent`
  - `eta_seconds`
  - `error_code`
  - `retryable`
  - `failed_step`
  - `error`

메모:

- 초기 버전은 polling 우선 + WebSocket optional로도 가능
- 다만 현재 프론트는 `ws`를 기본 경로로 시도한다

### 5.4 취소

#### `POST /cancel`

목적:

- 현재 실행 중인 task 취소

요청:

```json
{
  "task_id": "string"
}
```

백엔드 TODO:

- cancellation token 또는 cooperative cancel 설계
- ffmpeg / sidecar 호출 중단 처리 정의
- 취소 상태를 프론트의 `cancelled`로 매핑

### 5.5 STT 선행 여부 확인

#### `POST /check_existing_stt`

목적:

- summary/embedding 실행 전 STT 존재 여부 확인

요청:

```json
{
  "file_path": "string"
}
```

응답:

```json
{
  "has_stt": true,
  "stt_file": "string"
}
```

백엔드 TODO:

- `file_path` 기반 또는 `record_id` 기반 lookup 규칙 정의
- 현재 저장소 구조에서 transcript artifact 존재 여부 노출

## 6. 검색/탐색 API TODO

### 6.1 검색

#### `GET /search`

목적:

- 검색 탭의 메인 결과

프론트 기대 query:

- `query`
- `limit`
- `start_date`
- `end_date`
- `sort_by`
- `sort_order`
- `min_score`
- `page`
- `page_size`
- `include_timing`
- `file_type`
- `status`
- `status_task`

프론트 기대 응답:

```json
{
  "keywordMatches": [],
  "similarDocuments": [],
  "query": "string",
  "limit": 10,
  "sort": { "by": "similarity", "order": "desc" },
  "filters": {},
  "pagination": { "page": 1, "pageSize": 5, "returned": 5, "hasNext": false },
  "timing": {},
  "cache": { "hit": false },
  "contract_version": "string"
}
```

백엔드 TODO:

- 현재 `/v1/search` 결과를 호환 shape로 재가공
- keyword / similar 결과 분리 기준 정의
- `status=pending`을 Rust 상태(`queued`, `processing`)와 어떻게 매핑할지 정의
- 날짜 필터, 페이지네이션, 최소 점수 필터 추가
- `file_type` 필터 추가
- optional timing/cache 메타 추가

메모:

- 현재 Rust 검색은 `items[]`만 반환하므로 adapter가 필요하다

### 6.2 유사 문서

#### `POST /similar`

목적:

- 기록 탭 "유사 문서" 다이얼로그 지원

요청:

```json
{
  "file_identifier": "string",
  "user_filename": "string",
  "refresh": false
}
```

응답:

```json
[
  {
    "file_uuid": "string",
    "file": "string",
    "display_name": "string",
    "score": 0.91,
    "snippet": "string",
    "uploaded_at": "datetime",
    "source_filename": "string",
    "link": "/download/...",
    "record_id": "uuid"
  }
]
```

백엔드 TODO:

- 현재 임베딩 artifact similarity 재사용 가능성 검토
- 기준 문서 제외 규칙
- score/snippet/link 생성 규칙 정의

### 6.3 유사도 그래프

#### `GET /api/similarity-graph`

목적:

- 그래프 탭 지원

주요 query:

- `min_similarity`
- `max_nodes`
- `sampling`
- `doc_types`
- `start_date`
- `end_date`
- `keyword`
- `refresh`

응답:

```json
{
  "nodes": [
    {
      "id": "string",
      "label": "string",
      "file": "string",
      "file_type": "audio",
      "record_id": "uuid",
      "uploaded_at": "datetime"
    }
  ],
  "edges": [
    {
      "source": "string",
      "target": "string",
      "weight": 0.82
    }
  ],
  "meta": {}
}
```

백엔드 TODO:

- 전체 문서 간 유사도 그래프 계산 API 추가
- 노드 ID와 다운로드/기록 lookup 간 매핑 일관성 확보
- 샘플링 전략 정의

## 7. 상세 보기/편집 API TODO

### 7.1 다운로드

#### `GET /download/:file_identifier`

목적:

- 상세 보기 overlay
- 기록 탭 직접 다운로드
- 그래프 탭에서 문서 열기

백엔드 TODO:

- `file_identifier` 규칙 정의
  - `record_id`
  - artifact id
  - path-like identifier
- STT / summary / original file 중 무엇을 기본 다운로드 대상으로 할지 구분 규칙 추가
- text/plain / attachment 헤더 정책 결정

메모:

- 현재 프론트는 `file_path`, `record_id`, `file_uuid`가 혼용된다
- identifier normalization 계층이 필요하다

### 7.2 STT 세그먼트

#### `GET /segments/:file_identifier`

목적:

- STT 결과 overlay의 화자/세그먼트 보기

응답:

```json
[
  {
    "start": 0.0,
    "end": 3.2,
    "text": "string",
    "speaker": null
  }
]
```

백엔드 TODO:

- whisper raw artifact에서 세그먼트 파싱 가능 여부 확인
- 현재 화자 정보가 없으면 `speaker=null`로만 우선 지원

### 7.3 STT 텍스트 수정

#### `POST /update_stt_text`

목적:

- overlay에서 전사 텍스트 직접 수정

요청:

```json
{
  "file_identifier": "string",
  "content": "string"
}
```

백엔드 TODO:

- transcript overwrite 정책 정의
- 수정 후 summary/embedding invalidation 필요
- 검색 인덱스 재동기화 필요

### 7.4 summary/embedding 초기화

#### `POST /reset_summary_embedding`

목적:

- STT 수정 후 요약/임베딩만 초기화

요청:

```json
{
  "record_id": "uuid"
}
```

백엔드 TODO:

- summary artifact 삭제
- embedding artifact 삭제
- 관련 상태/메타데이터 재설정
- 이후 `/process` 재실행 가능 상태로 전환

## 8. 삭제/초기화/관리 API TODO

### 8.1 개별 artifact 삭제

#### `POST /delete`

목적:

- overlay에서 STT/summary 삭제

요청:

```json
{
  "file_identifier": "string",
  "file_type": "stt"
}
```

백엔드 TODO:

- summary/STT 단위 삭제 semantics 정의
- 삭제 후 history/completed_tasks 동기화

### 8.2 기록 일괄 삭제

#### `POST /delete_records`

목적:

- 기록 탭의 다중 삭제

요청:

```json
{
  "record_ids": ["uuid"]
}
```

백엔드 TODO:

- recordings/jobs/files/artifacts cascade delete
- running task/job 존재 시 처리 규칙 정의

### 8.3 전체 task 초기화

#### `POST /reset_all_tasks`

목적:

- 전체 초기화 다이얼로그

요청:

```json
{
  "tasks": ["stt", "embedding", "summary"]
}
```

백엔드 TODO:

- 선택 단계별 artifact 일괄 삭제
- DB 상태 재계산
- 대량 업데이트 시 성능 고려

### 8.4 모델 목록

#### `GET /models`

목적:

- 설정 다이얼로그 모델 선택 목록

응답 예시:

```json
{
  "models": [],
  "default": {
    "whisper": "string",
    "summarize": "string",
    "embedding": "string",
    "provider": "llamacpp"
  },
  "models_by_task": {
    "summary": [],
    "embedding": []
  }
}
```

백엔드 TODO:

- 현재 설정값/허용 모델 목록 노출
- provider(`ollama`/`llamacpp`) 정책 정리

메모:

- 현재 Rust는 고정 환경 변수 모델을 사용하므로 동적 모델 목록이 없다

### 8.5 서버 종료

#### `POST /shutdown`

목적:

- 설정 다이얼로그의 서버 종료 버튼

백엔드 TODO:

- 실제 제공 여부 재검토
- 운영 환경에서 위험하면 dev-only 또는 비활성화 고려

## 9. 보조/지연 항목

### `GET /tasks`

- client helper는 존재하지만 현재 React UI 핵심 플로우에서 강하게 쓰이지 않음
- running task snapshot API로 남겨둘 수 있음

### `POST /reset`

- client helper는 존재하지만 현재 활성 UI에서 직접 호출되지 않음
- record 단위 전체 초기화 API로 나중에 추가 가능

## 10. 엔드포인트 외 백엔드 TODO

엔드포인트만 추가해서는 부족하고 아래 내부 변경이 필요하다.

### 10.1 Task 모델 확장

- recording/job 외에 frontend task 추적 구조 추가
- 필드 예시:
  - `task_id`
  - `record_id`
  - `task_type`
  - `status`
  - `stage`
  - `progress_percent`
  - `eta_seconds`
  - `retry_of_task_id`
  - `error_code`
  - `retryable`

### 10.2 Artifact 식별 체계

- `file_identifier`로 STT/summary/original을 안정적으로 가리키는 규칙 필요
- 프론트가 `record_id`, `file_path`, `file_uuid`를 혼용하므로 normalization 레이어 필요

### 10.3 문서 포맷 지원

- 프론트 업로드 UX를 유지하려면 오디오 외 문서 포맷 ingest가 필요
- 최소 결정 사항:
  - PDF/TXT/MD를 summary/embedding 대상으로 허용할지
  - 문서형 record의 `completed_tasks.stt`는 항상 false로 둘지

### 10.4 검색 read model

- search/history/graph 응답은 현재 DB row를 그대로 내보내기보다 read model 전용 projection이 더 적합

### 10.5 권한/보호

- 프론트 client에는 destructive auth header 구조가 이미 있다
- 현재 UI에서 토큰을 실제로 넣진 않지만, 삭제/초기화/종료 API는 보호 여부를 결정해야 한다

## 11. 구현 순서 제안

### Phase 1. 프론트가 최소 동작하는 read-only 흐름

- `POST /upload`
- `GET /history`
- `GET /download/:file_identifier`
- `GET /search`

목표:

- 업로드
- 기록 조회
- 기본 검색
- 결과 열람

### Phase 2. 큐/처리 제어

- `POST /process`
- `GET /progress/:task_id`
- `/ws`
- `POST /check_existing_stt`
- `POST /cancel`

목표:

- 현재 작업 큐 탭 복원
- 개별 단계 재실행 복원

### Phase 3. 편집/초기화/삭제

- `POST /update_filename`
- `GET /segments/:file_identifier`
- `POST /update_stt_text`
- `POST /reset_summary_embedding`
- `POST /delete`
- `POST /delete_records`
- `POST /reset_all_tasks`

### Phase 4. 탐색/설정 확장

- `POST /similar`
- `GET /api/similarity-graph`
- `GET /models`
- `POST /shutdown`

## 12. 완료 기준

다음 조건을 만족하면 "프론트 유지 전제의 API 정합성 확보"로 본다.

- 업로드 탭이 현재 UI 그대로 동작한다
- 기록 탭이 `/history` 기반으로 정상 렌더링된다
- 작업 큐 탭이 progress polling/WebSocket과 함께 동작한다
- 검색 탭이 현재 필터/결과 shape를 유지한다
- 텍스트 overlay에서 보기/수정/삭제 흐름이 동작한다
- 유사 문서/그래프 탭이 에러 없이 동작한다
- 삭제/초기화/모델 조회 버튼이 현재 UX를 유지한다

## 13. 참고 대상

- `frontend/src/api/client.ts`
- `frontend/src/api/types.ts`
- `frontend/src/hooks/useTaskQueue.ts`
- `frontend/src/hooks/useHistory.ts`
- `frontend/src/components/UploadSection.tsx`
- `frontend/src/components/HistoryPanel.tsx`
- `frontend/src/components/SearchPanel.tsx`
- `frontend/src/components/TextOverlay.tsx`
- `frontend/src/components/SimilarDocsDialog.tsx`
- `frontend/src/components/SimilarityGraphPanel.tsx`
- `frontend/src/components/SettingsDialog.tsx`
- `rust/src/api/mod.rs`
- `rust/src/storage.rs`
- `rust/src/models.rs`
