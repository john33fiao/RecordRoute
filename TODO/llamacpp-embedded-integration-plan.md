# llama.cpp 내장(서버 의존 최소화) 전환 계획

## 목표
- 현재 `LLM=llama-cli`, `Embedding=OpenAI 호환 HTTP`로 분리된 경로를 점진적으로 단일 런타임으로 수렴한다.
- 운영 환경에서 별도 `llama-server` 프로세스 의존을 선택사항으로 낮추고, 코드베이스 내장 모드(embedded mode)를 제공한다.

## 범위
- 백엔드 provider 계층(`sttEngine/providers/*`) 중심 변경
- 환경변수/설정 키 정리
- 테스트 보강 및 문서 동기화

## 작업 계획

### 1) 현행 동작 고정(회귀 안전장치)
- `llamacpp` provider 관련 기존 테스트를 점검하고, embedding HTTP 경로의 계약을 테스트로 명시한다.
- 실패/타임아웃/모델 경로 누락 시 오류 코드와 메시지 계약을 고정한다.

### 2) 내장 Embedding 경로 설계
- 옵션 A: `llama-cli` 호출 기반 embedding 추출
- 옵션 B: `llama-cpp-python` 기반 in-process embedding 추출
- 두 옵션의 성능/의존성/배포 복잡도를 비교해 기본 경로를 선택한다.

### 3) Provider 인터페이스 확장
- `LlamaCppEmbeddingProvider`에 실행 모드(`http|embedded`) 분기 추가
- `EMBEDDING_PROVIDER=llamacpp` 유지 하에, 추가 설정으로 모드 전환 가능하게 설계
- 기본값은 하위호환(기존 HTTP 모드) 유지

### 4) 설정 키 및 호환성 정책
- 신규 설정 키(예: `LLAMA_CPP_EMBEDDING_MODE`) 도입
- 기존 `LLM_BASE_URL`, `EMBEDDING_BASE_URL`, `LLAMA_CPP_TIMEOUT`과 충돌 없이 동작하도록 우선순위 정의
- `.env.example`, `README.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` 동기화

### 5) 구현 및 테스트
- provider 분기 구현 + 단위 테스트 추가
- 검색 경로(`/search`, `/similar`)에서 임베딩 차원 불일치/오류 처리 회귀 확인
- 핵심 회귀 테스트 실행:
  - `pytest tests/http_api/test_workflow.py`
  - `pytest tests/http_api/test_search.py`
  - `pytest tests/server/test_queue.py`
  - `pytest tests/test_vocab_system.py`

### 6) 단계적 롤아웃
- 1차: hidden flag(기본 off)로 내장 모드 출시
- 2차: 운영 검증 후 환경별 기본값 재검토
- 실패 시 즉시 HTTP 모드로 되돌릴 수 있는 rollback 절차 문서화

## 완료 기준(Definition of Done)
- 내장 모드 on/off 전환 시 기능 동등성 확보
- 회귀 테스트 통과
- 문서 4종(README/AGENTS/CLAUDE/GEMINI) 동기화 완료
- 운영 체크리스트(헬스체크, 성능, 롤백) 포함
