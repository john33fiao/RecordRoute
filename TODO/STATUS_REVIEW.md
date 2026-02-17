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
