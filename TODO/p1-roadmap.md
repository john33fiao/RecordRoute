# P1 Roadmap (에이전트 코딩 실행 계획)

- 작성일: 2026-02-15
- 기준 문서: `README.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `TODO/TODO.md`, `TODO/STATUS_REVIEW.md`
- 목적: TODO의 P1 항목을 실제 개발 작업(백엔드/프론트/문서/테스트)으로 분해해 에이전트가 바로 실행할 수 있도록 표준 절차를 정의한다.

---

## 1. P1 범위 (TODO 기준)

다음 항목을 이번 P1 범위로 고정한다.

1. 검색 고급 필터 UI + 하이라이트
2. 오버레이 접근성 강화 (ESC 종료, 포커스 트랩, ARIA)
3. 그래프 스타일/필터 + React 메인 UI 통합
4. 실패 복구 정책 표준화 (재시도 가능 오류 코드 + UI 가이드)
5. API 버전 관리/OpenAPI 초안
6. PYTHONPATH 기반 실행 옵션 재도입 검토 (보류)

의존관계:
- 검색 고급화는 검색 API 계약 명세가 선행되어야 한다.
- 그래프 통합은 P0 그래프 렌더 엔진 확정 결과를 전제로 한다.

---

## 2. 작업 원칙 (문서 합의사항 반영)

### 2.1 문서/기준 우선순위
- 사용자/운영 관점은 `README.md`를 기준으로 유지한다.
- 에이전트 개발 규칙은 `AGENTS.md`를 기준으로 유지한다.
- `CLAUDE.md`, `GEMINI.md`는 `AGENTS.md` 요약본으로 취급한다.

### 2.2 아키텍처/경로 원칙
- 백엔드 엔트리는 `sttEngine/http_api/app.py`, 서버 실행은 `python -m sttEngine.server` 기준으로 유지한다.
- 프론트 작업은 `frontend/src/*`를 기준으로 하고 `frontend/legacy/*`는 fallback 유지 목적의 최소 변경만 허용한다.
- 라우트/스키마 변경 시 `frontend/src/api/client.ts`, `frontend/src/api/types.ts`를 함께 동기화한다.

### 2.3 API/에러/경로 규약
- `/process` 계약에서 `summary` step 키, `whisper` 모델 키를 유지한다.
- 실패 응답 필드는 `error`, `error_code`, `retryable`, `failed_step`를 유지한다.
- 진행률 응답은 `progress_percent`, `eta_seconds`, `error` 구조를 유지한다.
- 경로 처리 시 문자열 조합 대신 `normalize_record_path`, `resolve_record_path`, `to_record_path` 유틸 사용을 우선한다.

---

## 3. 단계별 실행 계획 (권장 4주 + 버퍼 1주)

## Phase 0. 착수 준비 (2~3일)

### 목표
- P1 구현 전 기준선(Baseline)과 작업 단위를 확정한다.

### 작업
- [ ] P1 항목별 Scope/Out-of-scope 문서화
- [ ] 기존 `/search`, `/progress`, 그래프 UI 동작 캡처
- [ ] 티켓 분할: Backend / Frontend / Test / Docs
- [ ] 리스크 등록: API 드리프트, 접근성 회귀, 성능 저하

### 산출물
- `TODO/p1-roadmap.md` 최신화
- P1 작업 보드(이슈 트래커)

---

## Phase 1. 검색 고급 필터 UI + 하이라이트

### 목표
- 검색 API 계약을 고정하고, UI에 고급 필터/하이라이트를 일관되게 연결한다.

### 백엔드
- [x] `GET /search` 파라미터 유효성/기본값 규칙 명시
- [x] 정렬/페이징/최소점수 처리 규칙 문서화
- [x] 응답 `contract_version`, `pagination`, `filters`, `cache`, `timing` 필드 검증

### 프론트
- [x] 고급 필터 UI: 날짜(start/end), 파일타입, 상태(status/status_task), 정렬, min_score
- [x] 결과 텍스트 하이라이트 렌더링(키워드 일치 중심)
- [x] empty/error/loading 상태를 표준 컴포넌트로 정리

### 테스트
- [x] `tests/http_api/test_search.py`에 파라미터/정렬/페이징 회귀 강화
- [ ] 프론트 필터 상태→API 쿼리 매핑 테스트 (남은 작업)
- [x] 프론트 빌드 확인

### 완료 기준
- [x] README 검색 계약 요약과 실제 응답 필드가 일치
- [ ] UI 필터 조합이 기대한 검색 결과를 재현 (실데이터 기반 수동 점검 필요)

---

## Phase 2. 오버레이 접근성 강화

### 목표
- 키보드/스크린리더 사용성을 확보해 접근성 결함을 줄인다.

### 작업
- [ ] ESC 키 종료 동작 표준화
- [ ] 포커스 트랩/포커스 리턴 구현
- [ ] ARIA role/label/description 보강
- [ ] 탭 순서, 포커스 가시성(visual indicator) 점검

### 테스트
- [ ] 접근성 체크리스트 기반 수동 시나리오(키보드-only)
- [ ] 오버레이 오픈/클로즈 회귀 테스트
- [x] 프론트 빌드 확인

### 완료 기준
- [ ] 오버레이 진입/이탈 시 포커스 손실 없음
- [ ] ESC 종료가 주요 오버레이 전부에서 일관 동작

---

## Phase 3. 그래프 스타일/필터 + 메인 UI 통합

### 목표
- P0 MVP 그래프를 메인 UX 수준으로 고도화한다.

### 작업
- [ ] 그래프 스타일 옵션(노드/엣지 시각 규칙) 추가
- [ ] 필터 옵션(노드 타입, score threshold, 링크 강도 등) 추가
- [ ] 메인 탭 상태와 그래프 필터 상태 동기화
- [ ] 대규모 데이터에서 렌더 성능 저하 여부 측정

### 테스트
- [ ] 그래프 필터 적용 시 데이터/뷰 일관성 테스트
- [ ] 줌/팬/드래그 기존 동작 회귀 확인
- [x] 프론트 빌드 확인

### 완료 기준
- [ ] 기본 그래프 상호작용 + 신규 필터가 충돌 없이 동작
- [ ] 주요 브라우저 환경에서 그래프가 usable 상태 유지

---

## Phase 4. 실패 복구 정책 표준화

### 목표
- 백엔드 오류 분류와 프론트 재시도 UX를 하나의 규약으로 통일한다.

### 백엔드
- [ ] `map_workflow_exception()` 규약 기준 오류 코드 표준표 작성
- [ ] `retryable` 판정 기준 확정(모델/네트워크/입력오류 구분)
- [ ] `failed_step` 누락 케이스 점검

### 프론트
- [ ] 오류 카드 UI를 `error_code/retryable/failed_step` 기반으로 단일화
- [ ] 재시도 가능 오류의 UX(버튼/문구/동작) 표준화
- [ ] 치명 오류와 사용자 교정 가능 오류 분리 안내

### 테스트
- [ ] `tests/http_api/test_workflow.py` 오류 매핑 회귀 강화
- [ ] 큐/진행률 오류 노출 회귀 (`tests/server/test_queue.py` 포함)

### 완료 기준
- [ ] 동일 오류가 API/WS/UI에서 같은 의미로 표시
- [ ] 재시도 UX가 오류 타입에 맞게 동작

---

## Phase 5. API 버전 관리/OpenAPI 초안

### 목표
- 마이그레이션 중 프론트-백엔드 드리프트를 줄일 최소 계약 문서를 확보한다.

### 작업
- [ ] 버전 전략 초안 (예: `v1` 경로 vs 헤더 버전)
- [ ] 우선 엔드포인트 문서화: `/process`, `/progress/{task_id}`, `/search`
- [ ] 요청/응답 예시와 오류 스키마 포함
- [ ] `contract_version` 필드 운영 규칙 정의

### 완료 기준
- [ ] OpenAPI 초안 또는 동등 스키마 문서가 리뷰 가능한 상태
- [ ] 프론트 타입 정의와 문서 간 불일치 항목 0건

---

## 4. 보류 항목: PYTHONPATH 실행 옵션 재도입 검토

현재는 보류 유지한다. 구현이 아니라 설계 검토만 수행한다.

### 검토 질문
- [ ] 기존 제거 사유(의존성/실행환경 충돌)가 현재도 유효한가?
- [ ] OS별 실행 경로 충돌 없이 안전한가?
- [ ] 표준 실행(`python -m sttEngine.server`) 대비 운영상 이점이 충분한가?

### 의사결정
- [ ] 재도입 보류 유지 / 제한 도입 / 전면 도입 중 하나를 근거와 함께 결정

---

## 5. 품질 게이트 (P1 공통)

P1 각 Phase 머지 전 아래 검증을 기본 게이트로 사용한다.

- `pytest tests/http_api/test_workflow.py`
- `pytest tests/http_api/test_search.py`
- `pytest tests/server/test_queue.py`
- `pytest tests/test_vocab_system.py`
- (프론트 변경 시) `cd frontend && npm run build`

추가 권장:
- [ ] 핵심 사용자 플로우 E2E 1~2개(업로드→처리→결과확인, 검색 필터→결과확인)

---

## 6. 변경 시 체크리스트 (에이전트용)

각 작업 PR에서 아래를 확인한다.

- [ ] 라우트/스키마 변경 시 백엔드 + 프론트 API 클라이언트/타입 + 테스트 동시 수정
- [ ] 오류 규약(`error`, `error_code`, `retryable`, `failed_step`) 유지
- [ ] 진행률 규약(`progress_percent`, `eta_seconds`, `error`) 유지
- [ ] 경로 유틸(`normalize_record_path`, `resolve_record_path`, `to_record_path`) 우선 사용
- [ ] 런타임 영향 변경 시 문서(`README.md`, `AGENTS.md`) 동기화
- [ ] TODO 상태 업데이트(완료 항목 제거 또는 상태 갱신)

---

## 7. 권장 작업 순서 (요약)

1. 검색 API 계약 고정 + 검색 필터 UI
2. 오버레이 접근성 강화
3. 그래프 스타일/필터 통합
4. 실패 복구 정책 표준화
5. API 버전 관리/OpenAPI 초안
6. PYTHONPATH 재도입 검토(보류)

이 순서는 TODO 의존관계와 현재 구조를 기준으로 리스크가 낮은 순서다.
