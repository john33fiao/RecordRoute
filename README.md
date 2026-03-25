# RecordRoute

RecordRoute는 **음성/문서를 업로드하고, 텍스트 결과를 확인하고, 검색/재활용**하는 데 집중한 기록 정리 도구입니다.

이 README는 아키텍처 설명 대신, 실제 사용자 관점에서 필요한 내용(기능/설정/사용/문제 해결)에 집중합니다.

---

## 무엇을 할 수 있나요? (UX 기준)

### 1) 파일 업로드 후 자동 처리
- 음성 파일 또는 문서를 업로드하고,
- 처리 단계(STT, 교정, 요약 등)를 선택해 실행할 수 있습니다.
- 작업은 백그라운드에서 진행되며 진행률과 예상 시간을 확인할 수 있습니다.

### 2) 진행 상황 실시간 확인
- 작업별 진행률(`progress_percent`)과 ETA(`eta_seconds`)를 확인할 수 있습니다.
- 실패 시 표준화된 오류 정보(`error_code`, `failed_step`)를 받아 원인 파악이 쉽습니다.

### 3) 결과 탐색과 재사용
- 업로드/처리 히스토리를 조회할 수 있습니다.
- 키워드/필터 기반 검색으로 원하는 기록을 빠르게 찾을 수 있습니다.
- 유사 문서 탐색으로 관련 기록을 이어서 확인할 수 있습니다.
- 원본/결과 파일 다운로드를 지원합니다.

### 4) 안전한 관리 작업
- 삭제/초기화처럼 파괴적인 API는 기본적으로 안전 모드가 켜져 있어,
  토큰/세션 없이 실수로 실행되지 않도록 보호됩니다.

---

## 빠른 시작

### macOS / Linux
```bash
./setup.sh
./run.sh
```

### Windows
```bat
setup.bat
run.bat
```

> 참고: Windows `run.bat`는 `8080` 포트 사용이 불가능하면 자동으로 `18080`을 시도합니다.

실행 후 기본 접속:
- API: `http://localhost:8080`
- WebSocket: `ws://localhost:8765`
- (Docker 분리 배포 시) 프론트: `http://localhost:3000`

---

## 기본 설정

### 1) `.env` 준비
- `.env.example`을 참고해 `.env`를 설정합니다.
- 핵심은 **어떤 LLM/Embedding provider를 쓸지** 먼저 정하는 것입니다.

주요 값:
- `LLM_PROVIDER` : 교정/요약 모델 provider
- `EMBEDDING_PROVIDER` : 임베딩 provider (미지정 시 LLM provider 상속)

### 2) 모델 준비
- Ollama 사용 시: 모델 pull + 서버 실행 필요
- llama.cpp 사용 시: GGUF 경로 지정(또는 Hugging Face 자동 다운로드 설정)

### 3) Ollama 체크 자동 스킵 조건
- `.env`에서 `LLM_PROVIDER`/`EMBEDDING_PROVIDER`가 `ollama`가 아니면
  setup/run의 Ollama 점검 단계는 자동으로 건너뜁니다.

---

## 사용 방법 (실무 흐름)

### 1) 업로드
- UI 또는 `/upload` API로 파일 업로드

### 2) 처리 실행
- `/process`로 단계 실행 (예: `stt → correct → summary`)
- `model_settings`로 모델, 언어, provider 등을 지정

### 3) 진행 확인
- `GET /progress/<task_id>`
- WebSocket 이벤트(`ws://localhost:8765`)

### 4) 결과 확인/활용
- `GET /history` : 처리 히스토리
- `GET /search` : 통합 검색
- `GET /similar/<uuid_or_path>` 또는 `POST /similar` : 유사 문서
- `GET /download/<uuid_or_path>` : 결과 다운로드

---

## Docker로 실행

기본(backend + frontend):
```bash
docker compose up -d --build
```

옵션 프로필:
```bash
# Ollama 포함
docker compose --profile ollama up -d --build

# llama.cpp 포함
docker compose --profile llamacpp up -d --build
```

---

## 트러블슈팅

### 1) 서버는 켜졌는데 요청이 실패할 때
- `GET /health`로 서버 상태 확인 (`{"status":"ok"}` 기대)
- 포트 충돌 여부 확인 (`8080`, `8765`, `3000`)

### 2) 모델 호출이 실패할 때
- `.env`의 provider 설정(`LLM_PROVIDER`, `EMBEDDING_PROVIDER`) 확인
- provider 서버 상태 확인 (예: Ollama 실행 여부)
- 모델 이름 오타/미설치 여부 확인

### 3) Ollama를 안 쓰는데 관련 점검이 걸릴 때
- `.env`에서 provider가 정말 `ollama`가 아닌지 다시 확인

### 4) 작업이 중간 실패할 때
- `/progress/<task_id>`의 `error_code`, `failed_step`, `retryable` 확인
- 입력 파일 형식(특히 음성 파일/세그먼트 생성 가능 여부) 점검

### 5) 삭제/초기화 API가 거부될 때
- 안전 모드 보호 동작일 가능성이 큽니다.
- 파괴적 API는 토큰/세션 환경변수 설정이 필요합니다.

### 6) 프론트가 API와 연결되지 않을 때 (Docker)
- 프론트 컨테이너의 `/api`, `/ws` 프록시 구성과 백엔드 컨테이너 상태를 함께 점검

---

## 자주 사용하는 명령

### 테스트
```bash
pytest
```

핵심 회귀 테스트:
```bash
pytest tests/http_api/test_workflow.py tests/http_api/test_search.py tests/server/test_queue.py tests/test_vocab_system.py
```

### 프론트 빌드
```bash
cd frontend
npm install
npm run build
```

---

## 문서 안내
- 사용자/운영 중심 문서: `README.md`
- 에이전트 개발 규칙 원문: `AGENTS.md`
- 모델별 요약 문서: `CLAUDE.md`, `GEMINI.md`
- 백로그: `TODO/TODO.md` 및 `TODO/*`
