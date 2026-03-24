# RecordRoute API

이 문서는 현재 `rust/` 코드 기준으로 외부에 공개된 HTTP API를 정리한 문서다. 내부 sidecar API(`whisper`, `llama`)는 포함하지 않는다. 내부 구조와 런타임 구성은 [architecture.md](./architecture.md)를 참고한다.

## 1. 개요

- Base URL: `http://{APP_BIND_ADDR}`
- 기본 응답 형식: `application/json`
- 업로드 요청만 `multipart/form-data`를 사용한다.
- 인증/권한 처리는 현재 구현되어 있지 않다.
- 모든 ID는 UUID 문자열이다.
- 시간 필드는 UTC 기준 `RFC 3339` 문자열로 직렬화된다.
- 업로드 완료는 곧 처리 완료를 의미하지 않는다. `POST /v1/recordings`는 업로드 저장과 job 생성만 수행하고, 실제 전사/요약/임베딩은 백그라운드 워커가 비동기로 처리한다.

## 2. 공통 상태 값

### `ProcessingStatus`

| 값 | 의미 |
| --- | --- |
| `queued` | 업로드는 저장되었고 아직 처리 대기 중 |
| `processing` | 워커가 현재 처리 중 |
| `completed` | 전체 파이프라인 완료 |
| `failed` | 재시도 한도를 넘기거나 처리 실패 |

### `ProcessingStep`

| 값 | 의미 |
| --- | --- |
| `upload_saved` | 원본 업로드 저장 완료 |
| `wav_ready` | 표준 WAV 생성 완료 |
| `transcribed` | 전사 완료 |
| `summarized` | 요약 완료 |
| `embedded` | 임베딩 저장 완료 |

`recording.current_step`과 `job.step`은 같은 enum을 사용한다.

## 3. 엔드포인트 요약

| Method | Path | 설명 |
| --- | --- | --- |
| `POST` | `/v1/recordings` | 오디오 파일 업로드, recording/job 생성 |
| `GET` | `/v1/recordings` | recording 목록 조회 |
| `GET` | `/v1/recordings/:recording_id` | recording 상세 조회 |
| `GET` | `/v1/jobs/:job_id` | job 상태 조회 |
| `GET` | `/v1/search` | 키워드 + 임베딩 기반 검색 |

## 4. 공통 에러 응답

에러는 항상 아래 형태의 JSON으로 반환된다.

```json
{
  "error": "human readable message"
}
```

현재 코드 기준 상태 코드는 다음과 같다.

| 상태 코드 | 의미 |
| --- | --- |
| `400 Bad Request` | 잘못된 query, multipart 형식 오류, 지원하지 않는 파일 형식 등 |
| `404 Not Found` | 존재하지 않는 `recording_id`, `job_id` |
| `500 Internal Server Error` | 내부 예외 |

주의:

- `500` 응답 본문은 내부 상세 오류를 노출하지 않고 항상 `"internal server error"`를 반환한다.
- `400`과 `404`는 비교적 구체적인 메시지를 그대로 반환한다.

## 5. 엔드포인트 상세

### 5.1 `POST /v1/recordings`

오디오 파일을 업로드하고 즉시 `recording`과 `job`을 생성한다.

#### Request

- Content-Type: `multipart/form-data`
- multipart에서 `filename`이 있는 첫 번째 파일 필드만 처리한다.
- 첫 번째 파일을 저장한 뒤 나머지 필드는 무시한다.

#### 업로드 제약

- 허용 확장자: `mp3`, `m4a`, `wav`
- 허용 content-type:
  - `audio/mpeg`
  - `audio/mp3`
  - `audio/mp4`
  - `audio/x-m4a`
  - `audio/m4a`
  - `audio/wav`
  - `audio/x-wav`
- 파일명은 저장 시 영숫자, `.`, `_`, `-`만 유지하고 나머지는 `_`로 치환한다.
- 최대 업로드 크기는 `MAX_UPLOAD_SIZE_BYTES` 설정값을 따른다.

추가 동작:

- 파일 필드가 없으면 `400`
- 파일 확장자가 없으면 `400`
- field 단위 `content-type`은 있으면 검사하고, 없으면 확장자 기준만 검사한다.
- 업로드 저장 후 DB에 `recordings`, `jobs` row를 만들고 워커를 깨운다.

#### Example

```bash
curl -X POST "http://127.0.0.1:3000/v1/recordings" \
  -F "file=@sample.wav;type=audio/wav"
```

#### Response `200 OK`

```json
{
  "recording_id": "5e5b6b8f-4d7e-4eb0-8d17-cd3c61a2f4c9",
  "job_id": "2e66d2bf-0748-4bc3-b6c6-588d3cfd6964",
  "status": "queued",
  "step": "upload_saved"
}
```

#### 주요 실패 예시

```json
{
  "error": "multipart payload must include a file field"
}
```

```json
{
  "error": "unsupported file extension `aac`"
}
```

```json
{
  "error": "unsupported content type `audio/aac`"
}
```

### 5.2 `GET /v1/recordings`

recording 목록을 최신순으로 조회한다.

#### Query Parameters

| 이름 | 타입 | 필수 | 설명 |
| --- | --- | --- | --- |
| `status` | string | 아니오 | `queued`, `processing`, `completed`, `failed` 중 하나 |

정렬:

- `created_at DESC`
- 현재 구현에는 pagination이 없다.

#### Example

```bash
curl "http://127.0.0.1:3000/v1/recordings"
```

```bash
curl "http://127.0.0.1:3000/v1/recordings?status=completed"
```

#### Response `200 OK`

```json
{
  "items": [
    {
      "id": "5e5b6b8f-4d7e-4eb0-8d17-cd3c61a2f4c9",
      "original_filename": "sample.wav",
      "status": "completed",
      "current_step": "embedded",
      "language": "ko",
      "has_transcript": true,
      "has_summary": true,
      "last_error": null,
      "created_at": "2026-03-24T01:23:45Z",
      "updated_at": "2026-03-24T01:24:10Z"
    }
  ]
}
```

#### 잘못된 요청 예시

```json
{
  "error": "invalid status filter: unsupported processing status `done`"
}
```

### 5.3 `GET /v1/recordings/:recording_id`

recording 본문과 연결된 job 상태를 함께 조회한다.

#### Path Parameters

| 이름 | 타입 | 설명 |
| --- | --- | --- |
| `recording_id` | UUID | 조회할 recording ID |

#### Example

```bash
curl "http://127.0.0.1:3000/v1/recordings/5e5b6b8f-4d7e-4eb0-8d17-cd3c61a2f4c9"
```

#### Response `200 OK`

```json
{
  "recording": {
    "id": "5e5b6b8f-4d7e-4eb0-8d17-cd3c61a2f4c9",
    "original_filename": "sample.wav",
    "original_content_type": "audio/wav",
    "file_size_bytes": 123456,
    "original_rel_path": "5e5b6b8f-4d7e-4eb0-8d17-cd3c61a2f4c9/original/sample.wav",
    "wav_rel_path": "5e5b6b8f-4d7e-4eb0-8d17-cd3c61a2f4c9/wav/standard.wav",
    "language": "ko",
    "transcript": "회의 내용을 정리해 주세요.",
    "summary": {
      "title": "요약 제목",
      "abstract": "요약 본문",
      "bullet_points": [
        "핵심 사항 1",
        "핵심 사항 2"
      ]
    },
    "summary_canonical_text": "Title: 요약 제목\n\nAbstract:\n요약 본문\n\nBullet Points:\n- 핵심 사항 1\n- 핵심 사항 2",
    "status": "completed",
    "current_step": "embedded",
    "last_error": null,
    "created_at": "2026-03-24T01:23:45Z",
    "updated_at": "2026-03-24T01:24:10Z"
  },
  "job": {
    "id": "2e66d2bf-0748-4bc3-b6c6-588d3cfd6964",
    "recording_id": "5e5b6b8f-4d7e-4eb0-8d17-cd3c61a2f4c9",
    "status": "completed",
    "step": "embedded",
    "attempt_count": 1,
    "last_error": null,
    "created_at": "2026-03-24T01:23:45Z",
    "updated_at": "2026-03-24T01:24:10Z",
    "started_at": "2026-03-24T01:23:46Z",
    "completed_at": "2026-03-24T01:24:10Z",
    "next_attempt_at": "2026-03-24T01:23:45Z"
  }
}
```

주의:

- 현재 설계상 recording당 job은 1개지만, 응답 스키마의 `job` 필드는 nullable이다.
- `summary` 객체의 JSON 필드 이름은 `abstract_text`가 아니라 `abstract`다.

#### 존재하지 않는 ID 예시

```json
{
  "error": "recording `5e5b6b8f-4d7e-4eb0-8d17-cd3c61a2f4c9` was not found"
}
```

### 5.4 `GET /v1/jobs/:job_id`

job 상태만 개별 조회한다.

#### Path Parameters

| 이름 | 타입 | 설명 |
| --- | --- | --- |
| `job_id` | UUID | 조회할 job ID |

#### Example

```bash
curl "http://127.0.0.1:3000/v1/jobs/2e66d2bf-0748-4bc3-b6c6-588d3cfd6964"
```

#### Response `200 OK`

```json
{
  "id": "2e66d2bf-0748-4bc3-b6c6-588d3cfd6964",
  "recording_id": "5e5b6b8f-4d7e-4eb0-8d17-cd3c61a2f4c9",
  "status": "processing",
  "step": "transcribed",
  "attempt_count": 1,
  "last_error": null,
  "created_at": "2026-03-24T01:23:45Z",
  "updated_at": "2026-03-24T01:23:58Z",
  "started_at": "2026-03-24T01:23:46Z",
  "completed_at": null,
  "next_attempt_at": "2026-03-24T01:23:45Z"
}
```

#### 존재하지 않는 ID 예시

```json
{
  "error": "job `2e66d2bf-0748-4bc3-b6c6-588d3cfd6964` was not found"
}
```

### 5.5 `GET /v1/search`

키워드 검색과 임베딩 유사도 검색을 함께 수행한다.

#### Query Parameters

| 이름 | 타입 | 필수 | 기본값 | 설명 |
| --- | --- | --- | --- | --- |
| `query` | string | 예 | 없음 | 앞뒤 공백을 제거한 뒤 사용하며, 비어 있으면 `400` |
| `limit` | integer | 아니오 | `10` | 최대 `MAX_SEARCH_LIMIT`까지만 허용 |

#### 현재 검색 동작

1. `query` 문자열로 임베딩 생성을 시도한다.
2. 임베딩 생성에 성공하면 keyword score와 cosine similarity를 함께 사용한다.
3. 임베딩 생성이 실패하면 로그만 남기고 keyword-only 검색으로 자동 fallback한다.
4. keyword hit가 있거나 similarity score가 있는 recording만 결과에 포함한다.

정렬:

1. `keyword_hit`가 `true`인 항목 우선
2. `keyword_score` 내림차순
3. `similarity_score` 내림차순
4. `created_at` 내림차순

주의:

- `summary_excerpt`, `transcript_excerpt`라는 이름이지만 현재 구현은 하이라이트 일부가 아니라 저장된 전체 summary canonical text와 transcript를 그대로 반환한다.
- 임베딩 artifact가 없거나 깨져 있거나 차원이 다르면 그 recording의 `similarity_score`만 제외된다.

#### Example

```bash
curl "http://127.0.0.1:3000/v1/search?query=meeting&limit=10"
```

#### Response `200 OK`

```json
{
  "items": [
    {
      "id": "5e5b6b8f-4d7e-4eb0-8d17-cd3c61a2f4c9",
      "original_filename": "meeting.wav",
      "status": "completed",
      "current_step": "embedded",
      "summary_excerpt": "Title: Meeting\n\nAbstract:\nA brief meeting summary\n\nBullet Points:\n- Action item one\n- Action item two",
      "transcript_excerpt": "meeting transcript",
      "keyword_hit": true,
      "keyword_score": 1.0,
      "similarity_score": 0.9939,
      "created_at": "2026-03-24T01:23:45Z"
    }
  ]
}
```

#### 잘못된 요청 예시

```json
{
  "error": "query must not be empty"
}
```

## 6. 주요 응답 객체

### `Recording`

| 필드 | 타입 | 설명 |
| --- | --- | --- |
| `id` | UUID | recording ID |
| `original_filename` | string | 업로드 당시 원본 파일명 |
| `original_content_type` | string \| null | 업로드 시 전달된 content-type |
| `file_size_bytes` | integer | 원본 파일 크기 |
| `original_rel_path` | string | storage root 기준 상대 경로 |
| `wav_rel_path` | string \| null | 표준 WAV 상대 경로 |
| `language` | string \| null | whisper가 반환한 언어 코드 |
| `transcript` | string \| null | 전사 본문 |
| `summary` | object \| null | 구조화 요약 |
| `summary_canonical_text` | string \| null | 요약을 검색/임베딩용 텍스트로 펼친 값 |
| `status` | `ProcessingStatus` | 현재 상태 |
| `current_step` | `ProcessingStep` | 마지막 완료 또는 실패 단계 |
| `last_error` | string \| null | 마지막 오류 메시지 |
| `created_at` | datetime | 생성 시각 |
| `updated_at` | datetime | 수정 시각 |

### `StructuredSummary`

| 필드 | 타입 | 설명 |
| --- | --- | --- |
| `title` | string | 요약 제목 |
| `abstract` | string | 요약 본문 |
| `bullet_points` | string[] | 핵심 bullet 목록 |

### `Job`

| 필드 | 타입 | 설명 |
| --- | --- | --- |
| `id` | UUID | job ID |
| `recording_id` | UUID | 연결된 recording ID |
| `status` | `ProcessingStatus` | 현재 상태 |
| `step` | `ProcessingStep` | 현재 또는 마지막 단계 |
| `attempt_count` | integer | claim 횟수 |
| `last_error` | string \| null | 마지막 오류 |
| `created_at` | datetime | 생성 시각 |
| `updated_at` | datetime | 수정 시각 |
| `started_at` | datetime \| null | 최초 처리 시작 시각 |
| `completed_at` | datetime \| null | 완료 시각 |
| `next_attempt_at` | datetime | 다음 재시도 가능 시각 |

### `RecordingListItem`

| 필드 | 타입 | 설명 |
| --- | --- | --- |
| `id` | UUID | recording ID |
| `original_filename` | string | 원본 파일명 |
| `status` | `ProcessingStatus` | 현재 상태 |
| `current_step` | `ProcessingStep` | 현재 단계 |
| `language` | string \| null | 감지 언어 |
| `has_transcript` | boolean | transcript 존재 여부 |
| `has_summary` | boolean | summary 존재 여부 |
| `last_error` | string \| null | 마지막 오류 |
| `created_at` | datetime | 생성 시각 |
| `updated_at` | datetime | 수정 시각 |

### `SearchResult`

| 필드 | 타입 | 설명 |
| --- | --- | --- |
| `id` | UUID | recording ID |
| `original_filename` | string | 원본 파일명 |
| `status` | `ProcessingStatus` | 현재 상태 |
| `current_step` | `ProcessingStep` | 현재 단계 |
| `summary_excerpt` | string \| null | 현재 구현에서는 summary canonical text 전체 |
| `transcript_excerpt` | string \| null | 현재 구현에서는 transcript 전체 |
| `keyword_hit` | boolean | 키워드 히트 여부 |
| `keyword_score` | float | FTS/filename 기반 점수 |
| `similarity_score` | float \| null | query embedding과 artifact embedding의 cosine similarity |
| `created_at` | datetime | recording 생성 시각 |

## 7. 처리 흐름 관점에서의 사용 패턴

일반적인 클라이언트 사용 순서는 다음과 같다.

1. `POST /v1/recordings`로 파일 업로드
2. 응답에서 `recording_id`, `job_id` 확보
3. `GET /v1/jobs/:job_id`로 처리 상태 polling
4. `status=completed`가 되면 `GET /v1/recordings/:recording_id`로 transcript/summary 확인
5. 검색이 필요하면 `GET /v1/search` 호출

현재 제공하지 않는 API:

- recording 삭제
- job 취소
- 수동 재처리
- health check
- pagination

## 8. 구현 메모

현재 문서는 아래 구현을 기준으로 작성했다.

- 공개 라우트: `rust/src/api/mod.rs`
- 응답 타입: `rust/src/models.rs`
- 에러 형식: `rust/src/error.rs`
- 업로드 검증/검색 정렬: `rust/src/storage.rs`
