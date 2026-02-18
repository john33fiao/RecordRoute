# llama.cpp 전환 상세 로드맵 (Ollama 의존성 제거)

이 문서는 RecordRoute의 현재 Ollama 의존 구성을 llama.cpp(또는 OpenAI 호환 LLM 서버) 기반으로 전환하기 위한 실행 로드맵이다.

- 기준 문서: `README.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`
- 범위: 백엔드 워크플로우(STT 제외), 임베딩/검색, 모델 조회 API, 프론트 모델 설정 계약, 배포/문서/테스트
- 비범위(1차): Whisper STT 엔진 전환

---

## 0. 목표와 성공 기준

### 목표
1. 요약/교정/임베딩 경로에서 Ollama 직접 의존을 제거한다.
2. llama.cpp 또는 호환 백엔드를 공급자(provider)로 붙일 수 있는 추상화 구조를 만든다.
3. 기존 API 계약(`error/error_code/retryable/failed_step`)과 운영 UX를 유지한다.
4. 문서/실행 스크립트/테스트를 코드와 동기화한다.

### 성공 기준 (Definition of Done)
- [~] `sttEngine/workflow/*`, `embedding_pipeline.py`, `vector_search.py`에서 Ollama 하드코딩 호출 제거 (핵심 경로 완료, `one_line_summary.py` 잔존)
- [x] `/models`가 provider 기반으로 모델 목록을 반환
- [x] `/process`의 `model_settings` 계약이 프론트와 백엔드에서 일치 (`whisper`, `summarize`, `correct`, `embedding`, `language`, `device`)
- [x] 핵심 회귀 테스트 통과
  - `pytest tests/http_api/test_workflow.py`
  - `pytest tests/http_api/test_search.py`
  - `pytest tests/server/test_queue.py`
  - `pytest tests/test_vocab_system.py`
- [x] `README.md`, `AGENTS.md`에 신규 런타임/환경변수/실행 방법 반영

---


## 0-1. 진행 상태 스냅샷 (2026-02-18)

- 전체 판정: **조건부 완료(핵심 마이그레이션 완료)**
- 완료:
  - provider 추상화/팩토리, summarize/correct, embedding/search, `/models`, 에러코드 범용화, 문서/compose/환경변수 반영
  - 핵심 회귀 테스트(`workflow/search/queue/vocab`) 통과
- 미완료/잔여:
  1. `sttEngine/one_line_summary.py`의 direct Ollama 경로 정리 또는 legacy 명시
  2. clean 환경 `setup/run` 실측 검증
  3. provider contract test 확장(chat/embed/list_models, timeout/retry, 미설치/미응답)

---

## 1. 현재 상태 진단 요약

### 1-1. Ollama 결합 지점 (2026-02-18 기준)
- 핵심 경로(요약/교정/임베딩/검색/API)는 provider 추상화로 전환 완료
  - `sttEngine/workflow/correct.py`, `sttEngine/workflow/summarize.py`: provider 호출 사용
  - `sttEngine/embedding_pipeline.py`, `sttEngine/vector_search.py`: provider 중립 임베딩 호출 사용
  - `sttEngine/http_api/handler.py` `/models`: provider 기반 목록/상태 응답
- 잔존 Ollama 직접 결합
  - `sttEngine/one_line_summary.py`: `import ollama` + `ollama.generate` 직접 사용
- 과도기 유틸/호환
  - `sttEngine/ollama_utils.py`: Ollama provider 경로에서 사용(호환 목적)
  - `sttEngine/server/services/errors.py`: `dependency_ollama` → `dependency_llm_provider` alias 유지
- 배포/의존성
  - `docker-compose.yml`: provider profile(`ollama`, `llamacpp`) 분리
  - `requirements.txt`: Ollama SDK optional 분리 (`requirements-ollama.txt`)

### 1-2. 계약 상태(업데이트)
- `/process` 기준 `model_settings.whisper` 계약으로 정렬 완료.
- 프론트는 `whisper/summarize/embedding/language/provider/llm_provider` 키를 전송하며, 백엔드 워크플로우는 `provider` 또는 `llm_provider`를 수용.

---

## 2. 아키텍처 전환 원칙

1. **Provider Interface First**
   - 비즈니스 로직(워크플로우)은 공급자 구현을 몰라야 한다.
2. **Config-driven Provider Selection**
   - 환경변수 기반으로 LLM/Embedding 공급자 선택.
3. **Backward Compatibility**
   - 과도기에는 Ollama provider 유지 가능(플래그 기반), API 응답 계약 유지.
4. **Observability**
   - 에러 코드 범용화 + 상세 원인은 로그에 분리.
5. **Incremental Rollout**
   - Summarize/Correct → Embedding → `/models` → 배포 순으로 단계적 전환.

---

## 3. 단계별 실행 계획

## Phase 1 — Provider 추상화 골격 구축

### 작업
- [x] `sttEngine/providers/` 패키지 추가
  - [x] `base.py`: 공통 예외/타입/인터페이스
  - [x] `llm_provider.py`: `chat()`, `generate()`, `list_models()`, `healthcheck()`
  - [x] `embedding_provider.py`: `embed(text)`, `embed_batch(texts)`, `healthcheck()`
- [x] `providers/ollama_provider.py` 생성
  - 기존 `ollama_utils.py`의 기능 단계적 이동
- [x] `providers/llama_cpp_provider.py` 생성
  - OpenAI 호환 endpoint 또는 llama.cpp 서버 endpoint에 대한 어댑터
- [x] `providers/factory.py`
  - 환경변수 기반 provider 인스턴스 반환

### 산출물
- provider 인터페이스/구현/팩토리 코드
- 단위 테스트(모킹) 초안

### 리스크
- llama.cpp 배포 방식 다양성(OpenAI 호환/비호환)으로 옵션 매핑이 복잡해질 수 있음.

---

## Phase 2 — 요약/교정 워크플로우 분리

### 작업
- [x] `sttEngine/workflow/summarize.py`
  - [x] `call_ollama_with_timeout` → `call_llm_with_timeout`로 중립화
  - [x] `call_ollama_with_retry` → `call_llm_with_retry`로 중립화
  - [x] `check_ollama_model_available` 제거, provider `list_models()`/`healthcheck()`로 대체
- [x] `sttEngine/workflow/correct.py`
  - [x] `ollama.chat` 직접 호출 제거
  - [x] provider `chat()` 사용
- [x] 옵션 사상 테이블 도입
  - 공통 옵션: `temperature`, `max_tokens`, `context_window`
  - provider별 옵션 키 변환

### 산출물
- summarize/correct의 provider 중립화
- 기존 프롬프트 품질 유지

### 검증
- [x] 요약 step 성공/실패/타임아웃 테스트
- [x] 교정 step 성공/실패 테스트

---

## Phase 3 — 임베딩/검색 전환

### 작업
- [x] `sttEngine/embedding_pipeline.py`
  - [x] `embed_text_ollama`를 중립 함수(`embed_text`)로 교체
  - [x] 응답 파싱을 provider별 adapter로 분리
- [x] `sttEngine/vector_search.py`
  - [x] 쿼리 임베딩 호출을 provider 중립 함수로 전환
- [x] `sttEngine/http_api/embedding.py`
  - [x] 임베딩 생성 경로 provider 연동
- [x] 벡터 차원 검증 추가
  - 모델 교체 시 차원 mismatch 탐지 및 명시적 오류 반환

### 산출물
- provider 기반 임베딩 파이프라인
- 모델 교체 가이드(재색인 필요 조건)

### 검증
- [x] 검색 API 회귀 (`/search`)
- [ ] 임베딩 인덱스 생성/갱신 테스트

---

## Phase 4 — 모델 조회 API & 프론트 계약 정비

### 작업
- [x] `sttEngine/http_api/handler.py` `/models`
  - [x] `ollama list` subprocess 제거
  - [x] provider `list_models()` 기반 응답
- [x] `frontend/src/api/types.ts`
  - [x] `ModelSettings` 키를 백엔드 계약과 일치시킴 (`whisper` 우선)
  - [x] 과도기 호환(`transcribe` 별칭) 처리 여부 결정
- [x] `frontend/src/hooks/useTaskQueue.ts`
  - [x] `/process`에 전달하는 `model_settings` 키를 계약과 동기화
- [x] `frontend/src/components/SettingsDialog.tsx`
  - [x] provider별 모델 목록 UX 반영

### 산출물
- 모델 조회/선택 플로우 정합성 확보

### 검증
- [x] `/models` 응답 계약 테스트
- [x] 프론트 `npm run build` 성공

---

## Phase 5 — 에러 체계/운영 가시성 정리

### 작업
- [x] `sttEngine/server/services/errors.py`
  - [x] `dependency_ollama`를 범용 코드(예: `dependency_llm_provider`)로 확장
  - [x] 하위호환 alias 전략 결정
- [x] 워크플로우 에러 매핑에서 공급자 구체 메시지 의존 최소화

### 산출물
- 공급자 전환 친화적 에러 모델

### 검증
- [x] queue/error 회귀 테스트 업데이트

---

## Phase 6 — 배포/의존성/문서 동기화

### 작업
- [x] `requirements.txt`
  - [x] Ollama SDK 필수 의존 제거 또는 optional 분리
- [x] `.env.example`
  - [x] 신규 변수 추가
    - `LLM_PROVIDER`, `EMBEDDING_PROVIDER`
    - `LLM_BASE_URL`, `EMBEDDING_BASE_URL`
    - `LLM_MODEL_*`, `EMBEDDING_MODEL_*`
  - [x] Ollama 전용 변수 deprecate 명시
- [x] `docker-compose.yml`
  - [x] llama.cpp 서버 서비스 추가 또는 provider profile 분리
- [x] 문서 동기화
  - [x] `README.md` 런타임/트러블슈팅 갱신
  - [x] `AGENTS.md` 실행/구조/API 기준 갱신

### 산출물
- 실행 가능한 배포 시나리오 + 최신 문서

### 검증
- [ ] clean 환경에서 setup/run 절차 점검

---

## 4. 마이그레이션 전략

### 전략 A: 점진 전환(권장)
1. provider 골격 + Ollama provider 먼저 도입 (기능 변화 없음)
2. llama.cpp provider 추가 후 특정 step부터 전환
3. 충분한 검증 후 기본 provider를 llama.cpp로 변경

장점: 다운타임/리스크 최소화

### 전략 B: 빅뱅 전환
- 단일 릴리스로 전면 교체

단점: 장애 구간 파악 어려움, 롤백 비용 큼

---

## 5. 환경변수 제안 (초안)

```bash
# Provider selection
LLM_PROVIDER=llama_cpp         # ollama | llama_cpp
EMBEDDING_PROVIDER=llama_cpp   # ollama | llama_cpp

# Endpoints
LLM_BASE_URL=http://localhost:8081
EMBEDDING_BASE_URL=http://localhost:8081

# Models
SUMMARY_MODEL_UNIX=...
SUMMARY_MODEL_WINDOWS=...
EMBEDDING_MODEL_UNIX=...
EMBEDDING_MODEL_WINDOWS=...

# Optional compatibility
OLLAMA_BASE_URL=http://localhost:11434  # deprecated(과도기)
```

---

## 6. 테스트 계획

### 필수 회귀
- [x] `pytest tests/http_api/test_workflow.py`
- [x] `pytest tests/http_api/test_search.py`
- [x] `pytest tests/server/test_queue.py`
- [x] `pytest tests/test_vocab_system.py`

### 추가 권장
- [ ] provider별 contract test (chat/embed/list_models)
- [ ] timeout/retry/backoff 시나리오
- [ ] 모델 미설치/미응답/차원 불일치 시나리오

---

## 7. 위험요소 및 대응

1. **모델 응답 포맷 차이**
   - 대응: provider adapter에서 응답 정규화
2. **옵션 키 불일치(num_ctx 등)**
   - 대응: 공통 옵션 스키마 + provider별 매핑
3. **임베딩 차원 변화로 검색 품질/호환성 저하**
   - 대응: 모델 전환 시 전체 재색인 강제
4. **프론트-백엔드 모델키 불일치**
   - 대응: 호환 매핑 + 계약 테스트 추가
5. **운영 중 provider 장애**
   - 대응: healthcheck + 명확한 dependency 에러 코드

---

## 8. 우선순위 백로그 (실행 순서)

P0 (즉시)
- [x] Provider 인터페이스 + 팩토리
- [x] summarize/correct에서 provider 호출로 치환
- [x] model_settings 키 계약 정렬

P1
- [x] 임베딩/검색 provider 전환
- [x] `/models` provider화
- [x] 에러코드 범용화

P2
- [x] docker compose/profile 정리
- [x] 문서/운영 가이드 완전 전환
- [ ] Ollama legacy 경로 정리(삭제/선택적 유지)

---

## 9. 최종 산출물 체크리스트

- [x] 코드: provider 아키텍처 + llama.cpp adapter + 기존 흐름 통합
- [x] API: `/models`, `/process` 계약 정합성
- [~] 테스트: 핵심 회귀 + provider 계약 테스트 (핵심 회귀 통과, provider 계약 테스트는 부분 보강 필요)
- [~] 문서: `README.md`, `AGENTS.md`, `TODO/llama-migration.md` (이번 업데이트로 `TODO/llama-migration.md` 재동기화)
- [x] 운영: `.env.example`, `docker-compose.yml`, 의존성 파일 정리

