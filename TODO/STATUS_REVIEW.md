# 코드베이스 상태 점검 (2026-02-17, TODO 재동기화)

## 1) 이번 점검 범위

다음 기준 문서와 실제 구현 파일을 대조해 상태를 갱신했다.
- 기준 문서: `TODO/TODO.md`, `README.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`
- 구현 근거: `sttEngine/http_api/handler.py`, `sttEngine/http_api/routes/search_routes.py`, `sttEngine/http_api/destructive_guard.py`, `frontend/scripts/benchmark-similarity-graph.mjs`, `frontend/src/components/SearchPanel.tsx`, `frontend/src/components/SimilarityGraphPanel.tsx`, `frontend/src/contexts/ThemeContext.tsx`

## 2) 핵심 결론

1. **기존 P0는 `handler.py` 책임 분리 1건만 유지**가 맞다.
   - 파괴적 API 보호는 이미 라우트 레벨 보호 + 테스트가 존재한다.
   - 그래프 성능 계측 스크립트는 이미 제공되어 P0로 재기재할 필요가 없다.
2. **검색 기능은 기본/고급 파라미터 노출과 하이라이트가 구현**되어 있다.
   - TODO는 “하이라이트 신규 구현”이 아니라 UX 정리/검증 강화로 조정해야 한다.
3. **그래프는 기본 파라미터 필터(min_similarity/max_nodes)와 인터랙션은 구현**되어 있다.
   - 남은 과제는 범례/스케일 명시, 타입/기간 필터 확장, 증분 업데이트 전략이다.
4. **테마는 아직 메모리 상태 중심**이다.
   - `ThemeContext`는 `localStorage` 영속화 및 시스템 테마 감지가 아직 없다.

## 3) 현재 유효 백로그 요약

### P0
1. `handler.py` 책임 분리(라우트 단위 모듈화 마무리)

### P1
1. 검색 고급 필터 UX 정리
2. 오버레이 접근성 회귀 테스트/보강
3. 그래프 스타일·범례·필터 고도화
4. 실패 복구 정책 표준화
5. API 버전 관리/OpenAPI 초안
6. 프론트 훅/컴포넌트 테스트 보강

### P2
1. 컨텍스트 리렌더 최적화
2. 대용량 히스토리 가상 스크롤
3. 그래프 증분 업데이트 전략
4. 스토리지 모니터링/자동 정리/백업 정책
5. 처리 통계 대시보드용 집계 API
6. 레거시 코드 정리

## 4) 의존관계 맵 (실행 전 체크)

- 검색 고급화(P1) ← 검색 API 계약 명세/OpenAPI 초안
- 운영 지표(P2) ← 로깅/메트릭 스키마 정리

## 5) 상태값 정리 결과

- 구현 완료 항목(파괴적 API 보호, 그래프 벤치 스크립트, 검색 하이라이트)은 TODO에서 재등록하지 않았다.
- TODO 문서는 실행 관점(P0/P1/P2) 기준으로 최신화했다.
- README/AGENTS/CLAUDE/GEMINI와 충돌하지 않도록 현재 아키텍처/용어 기준을 유지했다.

---

# llama 마이그레이션 Phase 1 상태 점검 (2026-02-18)

대상: `TODO/llama-migration.md`의 **"Phase 1 — Provider 추상화 골격 구축"**

## [Step 1] 계획 수립 — PASS
- 목표: Phase 1 체크리스트(인터페이스/구현/팩토리/테스트 초안) 실제 코드 반영 여부 확인.
- 범위(In): `sttEngine/providers/*`, `tests/providers/*`.
- 범위(Out): Phase 2+ 워크플로우/임베딩 전환 완료 판정.
- 검증: 파일 대조 + 관련 단위 테스트 실행.

## [Step 2] 계획 검토 — PASS
- 누락 가능성이 큰 항목(팩토리 alias, healthcheck/list_models 계약)을 체크 항목에 포함.
- 배포/운영 문서는 참고만 하고 구현 판정은 코드/테스트 기준으로 제한.

## [Step 3] 검토 재검토 — PASS
- DoD(골격 구축) 기준으로만 판정하고, 확장 단계(Phase 2~6)는 별도 리스크로 분리.
- 가정: 테스트가 provider 골격 계약을 대표한다.

## [Step 4] 과도성 검토 — PASS
- 코드 변경 없이 상태 점검 중심으로 최소 범위 유지.

## [Step 5] 구현(점검 수행) — PASS
- `sttEngine/providers/base.py`에 provider 공통 예외/정규화 유틸 존재 확인.
- `llm_provider.py`, `embedding_provider.py`에 추상 인터페이스(`chat/generate/list_models/healthcheck`, `embed/embed_batch/healthcheck`) 확인.
- `ollama_provider.py`, `llama_cpp_provider.py` 구현체 존재 및 factory 연결 확인.

## [Step 6] 목적 적합성 검토 — PASS
- `TODO/llama-migration.md` Phase 1 체크리스트의 [x] 항목과 실제 코드 구조가 일치.

## [Step 7] 잠재 이슈/보안 검토 — PASS
- 고위험 보안 이슈는 미발견.
- 단, `llama_cpp` CLI 호출/endpoint 다양성으로 옵션 매핑 오차 가능성은 잔존(문서 리스크와 동일).

## [Step 8] 회귀 검토 — PASS
- Provider 관련 단위 테스트 통과(아래 테스트 증거 참조).

## [Step 9] 대형 함수/파일 분리 검토 — PASS
- Phase 1 범위 내 추가 분리 필요 수준의 거대 함수는 미발견.

## [Step 10] 재사용/통합성 검토 — PASS
- 워크플로우/임베딩 계층에서 provider factory를 통해 주입 가능한 구조로 통합되어 있음.

## [Step 11] 사이드 이펙트 검토 — PASS
- 본 점검은 코드 실행 경로 변경 없이 문서 점검이므로 런타임 부작용 없음.

## [Step 12] 전체 변경사항 리뷰 — PASS
- 이번 변경은 상태 점검 기록 문서 추가만 포함.

## [Step 13] 불필요 코드 정리 — PASS
- 임시 코드/스크립트 생성 없음.

## [Step 14] 품질 기준 검토 — PASS
- 점검 대상 단위 테스트 통과.

## [Step 15] 사용자 흐름 검토 — PASS
- 사용자 관점에서는 provider 선택(`ollama`/`llamacpp`) 가능한 구조가 유지됨.

## [Step 16] 이슈 추적 재검토 — PASS
- 발견 이슈: 없음(Phase 1 범위).

## [Step 17] 배포 준비 판정 — PASS(조건부 READY)
- **Phase 1 자체는 READY**.
- 단, 전체 llama 마이그레이션 READY 아님(Phase 3~6 미완료 항목 존재).

## [Step 18] 커밋/PR
- 이 점검 기록을 커밋하고 PR에 반영.

## 테스트 증거
- `pytest tests/providers/test_provider_factory.py`
- `pytest tests/providers/test_llama_cpp_embedding_provider.py`

---

# llama 마이그레이션 진행 점검 (2026-02-18, RTD 재검토)

요청: "llama 마이그레이션 작업이 잘 진행됐는지"를 현재 코드/문서/테스트 기준으로 재검토.

## [Step 1] 계획 수립 — PASS
- 범위(In): provider 추상화, `/models` 계약, `/process model_settings` 정합성, 핵심 회귀 테스트.
- 범위(Out): 실제 clean 환경 배포(run/setup) 실측, 대규모 성능 벤치.
- 검증 방식: 코드 근거 확인 + 핵심/공급자 테스트 실행.

## [Step 2] 계획 검토 — PASS
- 문서 체크리스트와 실제 코드 간 드리프트 가능성을 핵심 위험으로 포함.

## [Step 3] 검토 재검토 — PASS
- DoD를 "코드에서 llama provider 경로가 실제 동작 가능한가"로 한정.
- 가정: 테스트가 현재 계약의 최소 보증선 역할을 한다.

## [Step 4] 과도성 검토 — PASS
- 기능 변경 없이 상태 진단만 수행.

## [Step 5] 구현(점검 수행) — PASS
- `sttEngine/providers/factory.py`에서 `ollama|llamacpp` 선택 + alias(`llama.cpp`, `llama_cpp`, `llama-cpp`) 처리 확인.
- `sttEngine/http_api/handler.py`의 `_serve_available_models()`가 provider 단위 상태/모델 목록(`models_by_provider`, `provider_status`)을 반환함을 확인.
- `sttEngine/http_api/workflow.py`에서 `model_settings.provider` 또는 `model_settings.llm_provider`를 교정/요약 경로로 전달함을 확인.
- `frontend/src/hooks/useTaskQueue.ts`, `frontend/src/api/types.ts`에서 `whisper/summarize/correct/embedding/llm_provider` 계약이 반영됨을 확인.

## [Step 6] 목적 적합성 검토 — PASS
- llama provider 전환의 핵심 골격(백엔드 provider 팩토리 + API 계약 + 프론트 전달)은 동작 가능한 상태.

## [Step 7] 잠재 이슈/보안 검토 — PASS(주의사항 있음)
- 고위험 보안 이슈는 미발견.
- 단, `sttEngine/one_line_summary.py`는 여전히 `ollama` 직접 import/call을 사용해 provider 중립화 범위 밖 잔존 경로로 확인.

## [Step 8] 회귀 검토 — PASS
- 아래 테스트 통과:
  - `pytest -q tests/providers/test_provider_factory.py tests/providers/test_llama_cpp_embedding_provider.py tests/http_api/test_workflow.py tests/http_api/test_search.py tests/server/test_queue.py tests/test_vocab_system.py`

## [Step 9] 대형 함수/파일 분리 검토 — PASS
- 이번 점검에서는 구조 변경 없이 상태 확인만 수행.

## [Step 10] 재사용/통합성 검토 — PASS
- 공통 provider 경로가 `sttEngine/llm_provider.py` + `sttEngine/providers/*`로 통합되어 있음.

## [Step 11] 사이드 이펙트 검토 — PASS
- 코드 실행 경로 변경 없음(문서 점검 기록만 갱신).

## [Step 12] 전체 변경사항 리뷰 — PASS
- 변경은 상태 점검 기록 추가 1건.

## [Step 13] 불필요 코드 정리 — PASS
- 임시 코드/스크립트 없음.

## [Step 14] 품질 기준 검토 — PASS
- 관련 회귀 테스트 통과.

## [Step 15] 사용자 흐름 검토 — PASS
- 사용자 흐름 관점에서 provider 선택 + 모델 목록 조회 + 워크플로우 전달이 이어지는 경로 확인.

## [Step 16] 이슈 추적 재검토 — PASS(개선 필요)
- 문서 드리프트: `TODO/llama-migration.md` 상단/하단 체크리스트에는 미완료([ ])가 다수 남아 있으나, 실제 구현은 상당수 완료([x]) 상태와 혼재.
- 후속 조치: 체크리스트를 현재 코드 기준으로 재정렬 필요.

## [Step 17] 배포 준비 판정 — 조건부 PASS
- **결론:** "llama 마이그레이션은 핵심 경로 기준으로 잘 진행됨".
- 단, 아래 2건이 남아 **완전 종료 READY는 아님**:
  1. `one_line_summary.py`의 provider 중립화 여부 결정.
  2. clean 환경 `setup/run` 실측 검증.

## [Step 18] 커밋/PR
- 본 점검 결과를 커밋/PR로 기록.

---

# 화자 분리 Phase 0 진행 점검 (2026-02-18)

대상: (삭제된) 화자분리 로드맵 문서의 **"Phase 0. 계약/지표 고정 (1주)"** 점검 기록

## 점검 요약
- **판정: 완료(PASS)**
- 근거:
  1. KPI 기준 문서가 존재하며 `DER`, 전환점 오차(F1), 요약 품질 A/B 영향 기준이 정의되어 있음.
  2. 평가셋 매니페스트가 존재하며 1:1 / 3~5인 / 잡음환경 셋의 보관 위치 및 버전 식별자가 고정되어 있음.
  3. 실패 정의(모델 불가/타임아웃/오디오 무효)와 diarization 에러코드 초안이 코드/테스트/README 계약에 반영되어 있음.

## 근거 파일
- `docs/diarization/kpi.md`
- `docs/diarization/dataset-manifest.md`
- `sttEngine/server/services/errors.py`
- `tests/server/test_error_mapping.py`
- `tests/http_api/test_workflow.py`
- `README.md`

## 동기화 조치
- 당시 화자분리 로드맵 문서의 Phase 0 TODO/DoD 체크박스를 실제 상태에 맞게 `[x]`로 갱신. *(해당 로드맵 문서는 이후 삭제됨)*
