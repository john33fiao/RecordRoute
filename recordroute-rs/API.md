# RecordRoute API 문서

RecordRoute REST API 엔드포인트 상세 문서

## Base URL

```
http://localhost:8080
```

## 인증

현재 버전은 인증을 요구하지 않습니다.

---

## 📤 파일 업로드

### POST /upload

음성 파일을 업로드합니다.

**Request**:
```bash
curl -F "file=@meeting.mp3" http://localhost:8080/upload
```

**Parameters**:
- `file` (multipart/form-data, required): 업로드할 파일

**지원 포맷**:
- MP3, WAV, M4A, MP4, OGG

**Response** (200 OK):
```json
{
  "file_uuid": "550e8400-e29b-41d4-a716-446655440000",
  "filename": "meeting.mp3",
  "path": "/uploads/550e8400-e29b-41d4-a716-446655440000.mp3"
}
```

**Error** (400 Bad Request):
```json
{
  "error": "No file provided"
}
```

---

## 🎤 워크플로우 실행

### POST /process

STT, 요약, 임베딩 워크플로우를 실행합니다.

**Request**:
```bash
curl -X POST http://localhost:8080/process \
  -H "Content-Type: application/json" \
  -d '{
    "file_uuid": "550e8400-e29b-41d4-a716-446655440000",
    "run_stt": true,
    "run_summarize": true,
    "run_embed": true,
    "stt_model": "base",
    "summary_model": "llama3.2"
  }'
```

**Request Body**:
```json
{
  "file_uuid": "string (required)",
  "run_stt": "boolean (default: false)",
  "run_summarize": "boolean (default: false)",
  "run_embed": "boolean (default: false)",
  "stt_model": "string (optional)",
  "summary_model": "string (optional)"
}
```

**Response** (200 OK):
```json
{
  "task_id": "task-1234-5678",
  "message": "Task started for stt"
}
```

**Error** (400 Bad Request):
```json
{
  "error": "No task specified"
}
```

---

## 📊 작업 관리

### GET /tasks

실행 중인 작업 목록을 조회합니다.

**Request**:
```bash
curl http://localhost:8080/tasks
```

**Response** (200 OK):
```json
{
  "tasks": [
    {
      "task_id": "task-1234-5678",
      "task_type": "stt",
      "file_uuid": "550e8400-e29b-41d4-a716-446655440000",
      "status": "Running",
      "progress": 45,
      "message": "Writing transcription results...",
      "started_at": "2025-12-30T10:30:00Z"
    }
  ]
}
```

**Task Status**:
- `Running`: 실행 중
- `Completed`: 완료
- `Failed`: 실패
- `Cancelled`: 취소됨

### POST /cancel

작업을 취소합니다.

**Request**:
```bash
curl -X POST http://localhost:8080/cancel \
  -H "Content-Type: application/json" \
  -d '{"task_id": "task-1234-5678"}'
```

**Request Body**:
```json
{
  "task_id": "string (required)"
}
```

**Response** (200 OK):
```json
{
  "message": "Task cancelled"
}
```

---

## 📝 히스토리 관리

### GET /history

처리된 파일 히스토리를 조회합니다.

**Request**:
```bash
curl http://localhost:8080/history
```

**Response** (200 OK):
```json
{
  "records": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "filename": "meeting.mp3",
      "timestamp": "2025-12-30T10:00:00Z",
      "stt_done": true,
      "summarize_done": true,
      "embed_done": true,
      "stt_path": "/data/whisper_output/550e8400-e29b-41d4-a716-446655440000.txt",
      "summary_path": "/data/whisper_output/550e8400-e29b-41d4-a716-446655440000_summary.txt",
      "one_line_summary": "프로젝트 진행 상황 및 다음 단계 논의",
      "tags": ["meeting", "project"],
      "deleted": false
    }
  ]
}
```

### POST /delete

히스토리 레코드를 삭제합니다 (소프트 삭제).

**Request**:
```bash
curl -X POST http://localhost:8080/delete \
  -H "Content-Type: application/json" \
  -d '{"ids": ["550e8400-e29b-41d4-a716-446655440000"]}'
```

**Request Body**:
```json
{
  "ids": ["string array (required)"]
}
```

**Response** (200 OK):
```json
{
  "message": "Deleted 1 records"
}
```

---

## 📥 파일 다운로드

### GET /download/{filename}

처리된 결과 파일을 다운로드합니다.

**Request**:
```bash
curl http://localhost:8080/download/550e8400-e29b-41d4-a716-446655440000.txt \
  -o transcript.txt
```

**Path Parameters**:
- `filename` (required): 다운로드할 파일명

**Response** (200 OK):
- Content-Type: text/plain 또는 application/json
- Body: 파일 내용

**Error** (404 Not Found):
```json
{
  "error": "File not found"
}
```

**파일 종류**:
- `{uuid}.txt` - 전사 텍스트
- `{uuid}_segments.json` - 세그먼트 (타임스탬프 포함)
- `{uuid}_summary.txt` - 전체 요약
- `{uuid}_oneline.txt` - 한 줄 요약

---

## 🔍 검색

### GET /search

의미 기반 벡터 검색을 수행합니다.

**Request**:
```bash
curl "http://localhost:8080/search?q=프로젝트 회의&top_k=5&start=2025-01-01&end=2025-12-31"
```

**Query Parameters**:
- `q` (string, required): 검색 쿼리
- `top_k` (integer, optional, default: 10): 반환할 결과 수
- `start` (string, optional): 시작 날짜 (ISO 8601)
- `end` (string, optional): 종료 날짜 (ISO 8601)

**Response** (200 OK):
```json
{
  "results": [
    {
      "doc_id": "550e8400-e29b-41d4-a716-446655440000",
      "score": 0.92,
      "filename": "meeting.mp3",
      "one_line_summary": "프로젝트 진행 상황 및 다음 단계 논의",
      "transcript_path": "/data/whisper_output/550e8400-e29b-41d4-a716-446655440000.txt",
      "summary_path": "/data/whisper_output/550e8400-e29b-41d4-a716-446655440000_summary.txt"
    },
    {
      "doc_id": "660e8400-e29b-41d4-a716-446655440001",
      "score": 0.85,
      "filename": "planning.mp3",
      "one_line_summary": "프로젝트 계획 수립 회의",
      "transcript_path": "/data/whisper_output/660e8400-e29b-41d4-a716-446655440001.txt",
      "summary_path": "/data/whisper_output/660e8400-e29b-41d4-a716-446655440001_summary.txt"
    }
  ],
  "query": "프로젝트 회의",
  "count": 2
}
```

**Score**:
- 0.0 ~ 1.0 범위
- 1.0에 가까울수록 높은 유사도

**Error** (400 Bad Request):
```json
{
  "error": "Query cannot be empty"
}
```

### GET /search/stats

벡터 검색 인덱스 통계를 조회합니다.

**Request**:
```bash
curl http://localhost:8080/search/stats
```

**Response** (200 OK):
```json
{
  "total_documents": 42,
  "embedding_model": "nomic-embed-text"
}
```

---

## ⚠️ 에러 응답

모든 에러는 다음 형식으로 반환됩니다:

**Error Response**:
```json
{
  "error": "에러 메시지"
}
```

**HTTP 상태 코드**:
- `400 Bad Request` - 잘못된 요청
- `404 Not Found` - 리소스를 찾을 수 없음
- `500 Internal Server Error` - 서버 내부 오류
- `503 Service Unavailable` - 외부 서비스 (Ollama 등) 연결 실패

---

## 📌 워크플로우 예시

### 전체 프로세스 (업로드 → 전사 → 요약 → 임베딩 → 검색)

```bash
# 1. 파일 업로드
UUID=$(curl -s -F "file=@meeting.mp3" http://localhost:8080/upload | jq -r '.file_uuid')

# 2. 전체 워크플로우 실행
TASK_ID=$(curl -s -X POST http://localhost:8080/process \
  -H "Content-Type: application/json" \
  -d "{\"file_uuid\":\"$UUID\",\"run_stt\":true,\"run_summarize\":true,\"run_embed\":true}" \
  | jq -r '.task_id')

# 3. 작업 완료 대기
while true; do
  STATUS=$(curl -s http://localhost:8080/tasks | jq -r ".tasks[] | select(.task_id==\"$TASK_ID\") | .status")
  if [ "$STATUS" = "Completed" ]; then
    break
  fi
  echo "Status: $STATUS"
  sleep 2
done

# 4. 결과 다운로드
curl http://localhost:8080/download/${UUID}.txt -o transcript.txt
curl http://localhost:8080/download/${UUID}_summary.txt -o summary.txt

# 5. 검색 테스트
curl "http://localhost:8080/search?q=회의 내용&top_k=3"
```

---

## 🔗 관련 문서

- [README.md](./README.md) - 프로젝트 개요 및 설치
- [CONFIGURATION.md](./CONFIGURATION.md) - 설정 가이드
- [ARCHITECTURE.md](./ARCHITECTURE.md) - 시스템 아키텍처
