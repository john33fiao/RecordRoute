# TODO Master (신규 백로그 시작)

- 마지막 점검일: 2026-02-15
- 점검 기준: `README.md`, `CLAUDE.md`, `GEMINI.md`, `TODO/STATUS_REVIEW.md`, 주요 구현 파일(`sttEngine/http_api/handler.py`, `frontend/src/*`, `frontend/graph-view.html`)
- 원칙: **완료 항목은 TODO에서 제거**하고, 실행이 필요한 항목만 유지

## 0) 이번 정리에서 반영한 내용

- 기존 TODO에서 `완료` 상태였던 항목(핵심 파이프라인, 검색/캐시 API, 엔트리포인트 모듈화, 중복 파일 감지 등)은 삭제했다.
- `TODO/STATUS_REVIEW.md`의 P0/P1/P2 실행 백로그를 TODO 기준으로 재편했다.
- 아래 목록은 지금부터 진행할 **신규 TODO**다.

---

## 1) P0 (즉시 착수)

- [x] Obsidian MCP 재설계 전 임시 비활성화 유지
  - 점검 결과: `_MCP_TEMPORARILY_DISABLED = True`와 `self.enabled = requested_enabled and not _MCP_TEMPORARILY_DISABLED`로 코드 레벨 전역 비활성화가 유지되고 있음 (`sttEngine/obsidian_mcp.py`)
  - 후속: 재활성화 조건(전송 트리거/파일명 정책/실패 처리 규약) 정의 전까지 현 상태 유지

- [ ] `handler.py` 책임 분리 (라우트 단위 모듈화) — **부분완료**
  - 점검 결과: `sttEngine/http_api/routes/*` 모듈로 GET/POST 일부가 분리됐지만, `UploadHandler` 내부에 다운로드/유사문서/정적 서빙/업로드 처리 로직이 여전히 대형 메서드로 남아 있음
  - 잔여 작업: `handler.py` 잔존 책임(파일 서빙/도메인별 응답 조합) 추가 분리 + 회귀 테스트 고정

- [ ] 파괴적 API 보호
  - 점검 결과: `/shutdown`, `/delete`, `/delete_records`, `/reset`, `/reset_all_tasks`, `/reset_summary_embedding` 호출 경로에서 토큰/세션 검증이 확인되지 않음
  - 목표: 토큰/세션 기반 보호 정책 도입

- [x] 그래프 렌더 엔진 확정 + 상호작용 MVP
  - 점검 결과: React `SimilarityGraphPanel`이 API 연동(`getSimilarityGraph`) + 줌/팬/노드 드래그를 구현했고, 메인 탭(`App.tsx`)에 통합되어 있음
  - 후속: 스타일/필터 고도화는 P1 항목으로 유지

- [x] 큐 진행률 UX 고도화
  - 점검 결과: 백엔드 진행률 DTO(`progress_percent`, `eta_seconds`, `error`)가 HTTP/WebSocket 모두에서 제공되고, 프론트 `JobQueue`에 퍼센트/ETA/오류 카드가 연결되어 있음
  - 후속: 실패 복구 정책 표준화는 P1에서 지속

## 2) P1 (다음 스프린트)

- [ ] 검색 고급 필터 UI + 하이라이트
  - 의존: 검색 API 파라미터/응답 계약 문서화

- [ ] 오버레이 접근성 강화
  - 범위: ESC 종료, 포커스 트랩, ARIA 보강

- [ ] 그래프 스타일/필터 + React 메인 UI 통합
  - 의존: P0 그래프 렌더 엔진 확정

- [ ] 실패 복구 정책 표준화
  - 범위: 재시도 가능 오류 코드 표준화 + UI 가이드

- [ ] API 버전 관리/OpenAPI 초안
  - 범위: 버전 전략, 주요 검색/처리 API 계약서 초안
- [ ] PYTHONPATH 기반 실행 옵션 재도입 검토 (보류)
  - 배경: 의존성/실행 환경 충돌 이슈로 현재 제거, 필요 시 설계 재검토 후 도입


## 3) P2 (중기 개선)

- [ ] 컨텍스트 리렌더 최적화
- [ ] 대용량 히스토리 가상 스크롤
- [ ] 그래프 증분 업데이트 전략
- [ ] 스토리지 모니터링/자동 정리/백업 정책
- [ ] 처리 통계 대시보드용 집계 API
- [ ] 레거시 코드 정리 (`frontend/legacy/*` 유지보수 범위 한정)

---

## 4) 의존관계 체크포인트

- 에러/진행률 UX(P0) ← 백엔드 progress/오류 DTO 표준화
- 검색 고급화(P1) ← 검색 API 계약(파라미터/응답) 명세
- 그래프 통합(P1) ← 렌더 엔진 확정(P0)
- 운영 지표(P2) ← 로깅/메트릭 스키마 정리

## 5) 메모

- React 기준 신규 작업은 `frontend/src/*` 중심으로 관리한다.
- `frontend/legacy/*`는 fallback 유지 목적의 최소 변경만 허용한다.

---

## 6) 제안 작업 순서 (프론트/백엔드 연동 + llama.cpp 마이그레이션)

아래는 요청한 4개 축을 실제 리스크/의존성 기준으로 재정렬한 권장 순서다.

1. **API 계약 확정 + 프론트-백엔드 연동 안정화**
   - 목표: 현재 Python 백엔드 API를 프론트(`frontend/src/api/client.ts`, `types.ts`)와 1:1로 고정
   - 산출물:
     - 엔드포인트/요청/응답 스키마 동기화
     - 에러 규약(`error_code`, `retryable`, `failed_step`) 일치
     - `/process`, `/search`, `/progress` 회귀 테스트 통과
   - 이유: 이후 단계(버그 수정/엔진 교체/Rust 전환)의 기준선(Baseline)을 먼저 만들기 위함

2. **프론트엔드 버그 수정 (현행 API 기준)**
   - 목표: UI/상태관리/예외처리를 현재 API 계약에 맞춰 먼저 안정화
   - 산출물:
     - 주요 사용자 플로우(업로드→처리→결과확인) E2E 시나리오 정리
     - 검색/히스토리/진행률 화면의 알려진 버그 정리 및 수정
   - 이유: 백엔드 엔진이 바뀌기 전에 UI 품질 이슈를 분리해서 해결해야 원인 분석이 쉬움

3. **Ollama 의존 기능의 llama.cpp 전환 (Python 백엔드 내부 어댑터 단계)**
   - 목표: 교정/요약 LLM 호출부를 Ollama API 종속 구조에서 추상화하고 llama.cpp 백엔드로 치환 가능하게 변경
   - 산출물:
     - LLM Provider 인터페이스(예: ollama, llama.cpp) 도입
     - 모델 옵션/프롬프트/타임아웃/오류 매핑 표준화
     - 동일 입력 대비 품질/속도/실패율 비교 리포트
   - 이유: Rust 이전에 모델 실행 레이어를 먼저 분리하면, Rust 마이그레이션 범위를 크게 줄일 수 있음

4. **백엔드 Rust(+llama.cpp) 마이그레이션**
   - 목표: HTTP API 계약을 유지한 채 고비용 경로부터 Rust로 점진 이관
   - 산출물:
     - 이관 우선순위: (1) 추론/전처리 고비용 경로 → (2) 워크플로우 오케스트레이션 → (3) 부가 API
     - Python↔Rust 공존 배포 전략(기능 플래그/카나리)
     - 장애 시 Python 경로로 즉시 롤백 가능한 운영 절차
   - 이유: API/UX/LLM 추상화가 먼저 안정된 뒤에 언어 전환을 해야 회귀 범위를 제어할 수 있음

## 7) 누락되기 쉬운 필수 항목 (추가 TODO)

- [ ] **API 계약 고정 문서(OpenAPI 또는 스키마 문서) 작성**
  - 마이그레이션 중 프론트/백엔드 드리프트 방지용

- [ ] **통합 회귀 테스트 게이트 구성**
  - 최소: `tests/http_api/test_workflow.py`, `test_search.py`, 프론트 빌드, 핵심 E2E 1~2개

- [ ] **성능/품질 기준선(Benchmark) 수립**
  - STT/교정/요약 처리시간, 실패율, 검색 응답시간, 요약 품질 체크셋

- [ ] **운영 관측성(로그/메트릭/트레이싱) 정비**
  - 단계별 latency, 모델 호출 실패 사유, 큐 체류시간 지표 추가

- [ ] **데이터/인덱스 마이그레이션 계획 수립**
  - `DB/upload_history.json`, `DB/file_registry.json`, `DB/vector_store/index.json` 호환성 검증

- [ ] **배포/롤백 전략 문서화**
  - Python-only ↔ Hybrid ↔ Rust-primary 단계별 롤아웃/롤백 체크리스트

- [ ] **보안 점검 병행**
  - `/shutdown`, `/delete*`, `/reset*` 보호 정책을 실제 운영 기본값으로 강제

- [ ] **개발 환경 재현성 확보**
  - llama.cpp 빌드 옵션(CPU/GPU), 모델 파일 경로, OS별 실행 가이드 표준화
