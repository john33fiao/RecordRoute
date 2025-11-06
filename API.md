# RecordRoute API 문서

RecordRoute의 HTTP API 엔드포인트 전체 명세입니다.

## 기본 정보

- **Base URL**: `http://localhost:8000` (기본 포트)
- **Content-Type**: `application/json` (JSON 응답), `multipart/form-data` (파일 업로드)
- **인코딩**: UTF-8

---

## 📂 파일 관리 API

### 1. 파일 업로드
오디오 파일 또는 문서를 업로드합니다.

```http
POST /upload
Content-Type: multipart/form-data
```

**요청 본문:**
```
files: [파일 데이터]
```

**응답 (200 OK):**
```json
[
  {
    "file_path": "records/{uuid}/filename.m4a",
    "file_type": "audio",
    "record_id": "{record-uuid}"
  }
]
```

**응답 (중복 파일):**
```json
[
  {
    "duplicate": true,
    "original_record_id": "{existing-uuid}",
    "filename": "filename.m4a"
  }
]
```

**에러 (400):**
- No file uploaded
- Invalid content type

---

### 2. 파일 다운로드
파일 식별자 또는 경로로 파일을 다운로드합니다.

```http
GET /download/{file_identifier}
```

**경로 매개변수:**
- `file_identifier`: UUID 또는 파일 경로

**응답:**
- 파일 스트림 (Content-Type은 파일 유형에 따라 자동 설정)

---

### 3. 파일 삭제
특정 파일을 삭제합니다.

```http
POST /delete
Content-Type: application/json
```

**요청 본문:**
```json
{
  "file_identifier": "{uuid-or-path}",
  "file_type": "stt|corrected|summary"
}
```

**응답 (200 OK):**
```json
{
  "success": true
}
```

**에러 (400):**
```json
{
  "error": "에러 메시지"
}
```

---

### 4. 레코드 삭제
여러 레코드를 일괄 삭제합니다 (soft delete).

```http
POST /delete_records
Content-Type: application/json
```

**요청 본문:**
```json
{
  "record_ids": ["uuid1", "uuid2", "uuid3"]
}
```

**응답 (200 OK):**
```json
{
  "success": true,
  "results": {
    "uuid1": {"success": true, "message": "..."},
    "uuid2": {"success": false, "error": "..."}
  }
}
```

---

## 🎯 워크플로우 실행 API

### 5. 워크플로우 실행
STT, 교정, 요약 등의 처리 파이프라인을 실행합니다.

```http
POST /process
Content-Type: application/json
```

**요청 본문:**
```json
{
  "file_path": "records/{uuid}/file.m4a",
  "steps": ["transcribe", "correct", "summarize", "embedding"],
  "record_id": "{record-uuid}",
  "task_id": "{optional-task-uuid}",
  "model_settings": {
    "correct_model": "gemma3:12b-it-qat",
    "summary_model": "gpt-oss:20b"
  }
}
```

**요청 필드:**
- `file_path`: 처리할 파일 경로 (필수)
- `steps`: 실행할 작업 단계 배열 (필수)
  - `transcribe`: Whisper STT 실행
  - `correct`: Ollama 텍스트 교정
  - `summarize`: 구조화된 요약 생성
  - `embedding`: 벡터 임베딩 생성
- `record_id`: 레코드 UUID (선택)
- `task_id`: 작업 추적 UUID (선택, 없으면 자동 생성)
- `model_settings`: 모델 설정 (선택)

**응답 (200 OK):**
```json
{
  "task_id": "{task-uuid}",
  "status": "started"
}
```

---

### 6. 작업 취소
실행 중인 작업을 취소합니다.

```http
POST /cancel
Content-Type: application/json
```

**요청 본문:**
```json
{
  "task_id": "{task-uuid}"
}
```

**응답 (200 OK):**
```json
{
  "success": true
}
```

---

### 7. 작업 진행 상태 조회
특정 작업의 진행 상태를 조회합니다.

```http
GET /progress/{task_id}
```

**응답 (200 OK):**
```json
{
  "task_id": "{task-uuid}",
  "status": "running|completed|failed|cancelled",
  "progress": 0.75,
  "current_step": "correct",
  "message": "텍스트 교정 중...",
  "result": {
    "stt_file": "records/{uuid}/file_stt.md",
    "corrected_file": "records/{uuid}/file_corrected.md"
  }
}
```

---

### 8. 실행 중인 작업 목록
현재 실행 중인 모든 작업을 조회합니다.

```http
GET /tasks
```

**응답 (200 OK):**
```json
{
  "tasks": [
    {
      "task_id": "{task-uuid}",
      "file_path": "records/{uuid}/file.m4a",
      "steps": ["transcribe", "correct"],
      "status": "running",
      "started_at": "2025-11-06T10:30:00"
    }
  ]
}
```

---

## 📝 레코드 관리 API

### 9. 업로드 히스토리 조회
업로드된 모든 파일의 히스토리를 조회합니다.

```http
GET /history
```

**응답 (200 OK):**
```json
[
  {
    "id": "{record-uuid}",
    "filename": "meeting_20251106.m4a",
    "timestamp": "2025-11-06T10:00:00",
    "file_type": "audio",
    "duration": 1234.5,
    "tasks": {
      "transcribe": "completed",
      "correct": "completed",
      "summarize": "pending"
    },
    "tags": ["회의", "프로젝트A"],
    "deleted": false
  }
]
```

---

### 10. 파일명 업데이트
레코드의 표시 이름을 변경합니다.

```http
POST /update_filename
Content-Type: application/json
```

**요청 본문:**
```json
{
  "record_id": "{record-uuid}",
  "filename": "새로운_파일명.m4a"
}
```

**응답 (200 OK):**
```json
{
  "success": true
}
```

---

### 11. 레코드 리셋
특정 레코드의 처리 상태를 초기화합니다.

```http
POST /reset
Content-Type: application/json
```

**요청 본문:**
```json
{
  "record_id": "{record-uuid}"
}
```

**응답 (200 OK):**
```json
{
  "success": true
}
```

---

### 12. 전체 작업 리셋
모든 레코드의 특정 작업 상태를 초기화합니다.

```http
POST /reset_all_tasks
Content-Type: application/json
```

**요청 본문:**
```json
{
  "tasks": ["transcribe", "correct", "summarize"]
}
```

**응답 (200 OK):**
```json
{
  "success": true,
  "message": "3개 레코드의 작업이 리셋되었습니다",
  "counts": {
    "total": 10,
    "reset": 3,
    "skipped": 7
  }
}
```

---

### 13. 요약 및 임베딩 리셋
특정 레코드의 요약과 벡터 임베딩을 삭제합니다.

```http
POST /reset_summary_embedding
Content-Type: application/json
```

**요청 본문:**
```json
{
  "record_id": "{record-uuid}"
}
```

**응답 (200 OK):**
```json
{
  "success": true,
  "message": "요약 및 임베딩이 리셋되었습니다"
}
```

---

## 🔍 검색 API

### 14. 통합 검색 (키워드 + 벡터)
키워드 매칭과 의미론적 벡터 검색을 동시에 수행합니다.

```http
GET /search?q={query}&start={start_date}&end={end_date}
```

**쿼리 매개변수:**
- `q`: 검색 쿼리 (필수)
- `start`: 시작 날짜 (선택, YYYY-MM-DD)
- `end`: 종료 날짜 (선택, YYYY-MM-DD)

**응답 (200 OK):**
```json
{
  "keywordMatches": [
    {
      "file": "records/{uuid}/file_stt.md",
      "file_uuid": "{uuid}",
      "display_name": "file_stt.md",
      "preview": "...검색어를 포함하는 텍스트...",
      "uploaded_at": "2025-11-06T10:00:00",
      "source_filename": "meeting.m4a",
      "link": "/download/{uuid}"
    }
  ],
  "similarDocuments": [
    {
      "file": "records/{uuid}/file_summary.md",
      "file_uuid": "{uuid}",
      "display_name": "file_summary.md",
      "score": 0.85,
      "uploaded_at": "2025-11-05T14:30:00",
      "source_filename": "discussion.m4a",
      "link": "/download/{uuid}"
    }
  ]
}
```

**에러 (500):**
```json
{
  "error": "검색 중 오류가 발생했습니다",
  "details": "상세 에러 메시지"
}
```

---

### 15. 파일명/태그 검색
파일명 또는 태그로 빠르게 검색합니다.

```http
GET /file_search?q={query}
```

**쿼리 매개변수:**
- `q`: 검색 쿼리

**응답 (200 OK):**
```json
[
  {
    "id": "{record-uuid}",
    "filename": "meeting_20251106.m4a",
    "tags": ["회의", "프로젝트A"]
  }
]
```

---

### 16. 유사 문서 검색 (GET)
특정 파일과 유사한 문서를 찾습니다.

```http
GET /similar/{file_identifier}
```

**경로 매개변수:**
- `file_identifier`: UUID 또는 파일 경로

**응답 (200 OK):**
```json
{
  "similar_documents": [
    {
      "file": "records/{uuid}/other_file.md",
      "score": 0.92,
      "display_name": "other_file.md",
      "link": "/download/{uuid}"
    }
  ]
}
```

---

### 17. 유사 문서 검색 (POST)
파일 식별자와 추가 옵션으로 유사 문서를 검색합니다.

```http
POST /similar
Content-Type: application/json
```

**요청 본문:**
```json
{
  "file_identifier": "{uuid-or-path}",
  "user_filename": "optional_filename.md",
  "refresh": false
}
```

**요청 필드:**
- `file_identifier`: 파일 UUID 또는 경로 (필수)
- `user_filename`: 표시할 파일명 (선택)
- `refresh`: 캐시 무시 및 재계산 (선택, 기본값: false)

**응답**: GET /similar/{file_identifier}와 동일

---

## 📄 STT 텍스트 관리 API

### 18. STT 파일 존재 확인
특정 파일에 대한 STT 결과물이 있는지 확인합니다.

```http
POST /check_existing_stt
Content-Type: application/json
```

**요청 본문:**
```json
{
  "file_path": "records/{uuid}/file.m4a"
}
```

**응답 (200 OK):**
```json
{
  "has_stt": true,
  "stt_file": "records/{uuid}/file_stt.md"
}
```

---

### 19. STT 텍스트 업데이트
STT 결과 텍스트를 직접 수정합니다.

```http
POST /update_stt_text
Content-Type: application/json
```

**요청 본문:**
```json
{
  "file_identifier": "{uuid-or-path}",
  "content": "수정된 텍스트 내용..."
}
```

**응답 (200 OK):**
```json
{
  "success": true,
  "record_id": "{record-uuid}"
}
```

**에러 (400):**
```json
{
  "success": false,
  "error": "에러 메시지",
  "record_id": null
}
```

---

## 🧠 벡터 임베딩 API

### 20. 증분 임베딩 실행
아직 임베딩되지 않은 파일들을 자동으로 찾아 임베딩합니다.

```http
POST /incremental_embedding
```

**요청 본문:** 없음

**응답 (200 OK):**
```json
{
  "success": true,
  "processed_count": 5,
  "message": "증분 임베딩 완료: 5개 파일 처리됨"
}
```

**에러 (500):**
```json
{
  "success": false,
  "error": "에러 메시지"
}
```

---

## 🔧 시스템 관리 API

### 21. 사용 가능한 모델 목록
Ollama에서 사용 가능한 모델 목록을 조회합니다.

```http
GET /models
```

**응답 (200 OK):**
```json
{
  "models": [
    {
      "name": "gemma3:4b",
      "size": "4.2GB",
      "modified": "2025-11-01"
    },
    {
      "name": "gpt-oss:20b",
      "size": "20.5GB",
      "modified": "2025-10-15"
    }
  ]
}
```

---

### 22. 캐시 통계 조회
Whisper 모델 캐시 통계를 확인합니다.

```http
GET /cache/stats
```

**응답 (200 OK):**
```json
{
  "cache_dir": "/home/user/.cache/whisper",
  "total_size": "2.5GB",
  "file_count": 3,
  "models": ["base", "medium", "large-v2"]
}
```

---

### 23. 캐시 정리
오래된 캐시 파일을 정리합니다.

```http
GET /cache/cleanup
```

**응답 (200 OK):**
```json
{
  "success": true,
  "freed_space": "1.2GB",
  "deleted_files": 5
}
```

---

### 24. 서버 종료
HTTP 서버를 안전하게 종료합니다.

```http
POST /shutdown
```

**요청 본문:** 없음

**응답 (200 OK):**
```json
{
  "success": true,
  "message": "서버 종료 요청이 접수되었습니다. 잠시 후 서버가 종료됩니다."
}
```

---

## 🌐 웹 UI 관련

### 25. 메인 페이지
웹 인터페이스를 제공합니다.

```http
GET /
```

**응답:** HTML 페이지

---

### 26. 정적 리소스
CSS, JavaScript 파일을 제공합니다.

```http
GET /upload.css
GET /upload.js
```

**응답:** 정적 파일 스트림

---

## 에러 코드 요약

| 상태 코드 | 의미 | 주요 원인 |
|---------|------|----------|
| 200 | 성공 | 요청이 정상 처리됨 |
| 400 | 잘못된 요청 | 필수 파라미터 누락, JSON 파싱 오류 |
| 404 | 찾을 수 없음 | 존재하지 않는 엔드포인트 또는 파일 |
| 500 | 서버 오류 | 내부 처리 오류, Ollama 연결 실패 등 |

---

## 워크플로우 단계 (steps)

process API에서 사용 가능한 단계:

| 단계 | 설명 | 입력 | 출력 |
|-----|------|-----|-----|
| `transcribe` | Whisper 음성인식 | 오디오 파일 (.m4a, .mp3 등) | {filename}_stt.md |
| `correct` | Ollama 텍스트 교정 | {filename}_stt.md | {filename}_corrected.md |
| `summarize` | 구조화된 요약 생성 | {filename}_corrected.md | {filename}_summary.md |
| `embedding` | 벡터 임베딩 생성 | 모든 .md 파일 | embeddings/*.npy |

---

## 파일 타입 (file_type)

시스템에서 인식하는 파일 유형:

- `audio`: 오디오 파일 (m4a, mp3, wav 등)
- `stt`: STT 결과물 (*_stt.md)
- `corrected`: 교정된 텍스트 (*_corrected.md)
- `summary`: 구조화된 요약 (*_summary.md)
- `document`: 기타 문서 (pdf, txt 등)

---

## 사용 예시

### Python 예시 - 파일 업로드 및 처리

```python
import requests
import json

base_url = "http://localhost:8000"

# 1. 파일 업로드
with open("meeting.m4a", "rb") as f:
    files = {"files": ("meeting.m4a", f, "audio/x-m4a")}
    response = requests.post(f"{base_url}/upload", files=files)
    upload_result = response.json()[0]

file_path = upload_result["file_path"]
record_id = upload_result["record_id"]

# 2. 워크플로우 실행
process_payload = {
    "file_path": file_path,
    "steps": ["transcribe", "correct", "summarize", "embedding"],
    "record_id": record_id,
    "model_settings": {
        "correct_model": "gemma3:12b-it-qat",
        "summary_model": "gpt-oss:20b"
    }
}
response = requests.post(f"{base_url}/process", json=process_payload)
task_result = response.json()
task_id = task_result["task_id"]

# 3. 진행 상태 확인
import time
while True:
    response = requests.get(f"{base_url}/progress/{task_id}")
    progress = response.json()

    if progress["status"] in ["completed", "failed", "cancelled"]:
        break

    print(f"진행률: {progress['progress']*100:.1f}% - {progress['message']}")
    time.sleep(2)

# 4. 결과 검색
response = requests.get(f"{base_url}/search", params={"q": "회의 안건"})
search_results = response.json()
print(json.dumps(search_results, indent=2, ensure_ascii=False))
```

### cURL 예시 - 검색 수행

```bash
# 키워드 + 벡터 검색
curl "http://localhost:8000/search?q=프로젝트%20진행%20상황&start=2025-11-01&end=2025-11-06"

# 파일명 검색
curl "http://localhost:8000/file_search?q=meeting"

# 특정 파일과 유사한 문서 찾기
curl "http://localhost:8000/similar/abc123-def456"
```

### JavaScript (Fetch API) 예시

```javascript
// 파일 업로드
const formData = new FormData();
formData.append('files', fileInput.files[0]);

const uploadResponse = await fetch('http://localhost:8000/upload', {
  method: 'POST',
  body: formData
});
const uploadResult = await uploadResponse.json();

// 워크플로우 실행
const processResponse = await fetch('http://localhost:8000/process', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    file_path: uploadResult[0].file_path,
    steps: ['transcribe', 'correct', 'summarize'],
    record_id: uploadResult[0].record_id
  })
});
const taskResult = await processResponse.json();

// 진행 상태 폴링
const checkProgress = async (taskId) => {
  const response = await fetch(`http://localhost:8000/progress/${taskId}`);
  const progress = await response.json();
  console.log(`Status: ${progress.status}, Progress: ${progress.progress}`);

  if (progress.status !== 'completed') {
    setTimeout(() => checkProgress(taskId), 2000);
  }
};

checkProgress(taskResult.task_id);
```

---

## 참고 사항

1. **비동기 처리**: `/process` 엔드포인트는 즉시 반환되며, 실제 처리는 백그라운드에서 진행됩니다. `/progress/{task_id}` 또는 `/tasks`로 진행 상태를 확인하세요.

2. **파일 경로**: 모든 파일 경로는 `records/{uuid}/` 형식을 따릅니다. UUID는 레코드 ID와 동일합니다.

3. **모델 설정**: 플랫폼에 따라 기본 모델이 다릅니다:
   - Windows: `gemma3:4b`
   - Unix/macOS: `gemma3:12b-it-qat`, `gpt-oss:20b`

4. **벡터 검색**: `/search` API를 사용하기 전에 임베딩이 생성되어 있어야 합니다. `/incremental_embedding`으로 자동 생성할 수 있습니다.

5. **파일 삭제**: 레코드 삭제는 soft delete로 구현되어 있으며, `deleted: true` 플래그가 설정됩니다. 물리적 파일은 유지됩니다.

6. **CORS**: 프로덕션 환경에서는 적절한 CORS 설정이 필요할 수 있습니다.

---

## 추후 개선 계획

- Swagger/OpenAPI 스펙 문서 자동 생성
- API 버저닝 (v1, v2)
- 인증 및 권한 관리 (JWT)
- Rate limiting
- WebSocket을 통한 실시간 진행 상태 스트리밍
- Batch processing API
- Export API (JSON, CSV, Excel)

---

**문서 버전**: 1.0
**마지막 업데이트**: 2025-11-06
