# 코드베이스 상태 점검 (2026-02-13)

## 1) 한눈에 보는 현황
- 백엔드(STT/교정/요약/임베딩/검색) 핵심 워크플로우는 구현되어 있으며, `server.py`는 신규 `sttEngine.http_api` 진입점으로 단순화되어 있음.
- 프론트엔드는 레거시 정적 파일(`frontend/legacy/*`)에서 React+TypeScript(`frontend/src/*`)로 이전된 상태.
- TODO는 **백엔드 문서(TODO.md)는 일부 완료**, **Graph/GUI 문서는 대부분 미착수**로 유지되고 있어, 실제 코드 진행도 대비 TODO 정합성 업데이트가 필요함.

## 2) TODO 진행률 스냅샷
- `TODO/TODO.md`: 36 / 65 완료 (약 55%)
- `TODO/Graph.md`: 0 / 54 완료 (0%)
- `TODO/GUI.md`: 0 / 82 완료 (0%)

## 3) 코드/문서 정합성 이슈
1. `GUI.md`에는 "React 전환 완료"라고 명시되어 있으나, 하위 항목에 `upload.js` 모듈화 같은 레거시 기준 항목이 남아 있음.
2. `TODO.md`의 리팩토링 과제(`server.py 모듈화`)는 일부 선행됨. 현재 실제 엔트리는 `sttEngine/http_api/app.py`, `handler.py`, `routes/*`로 분리됨.
3. Graph 기능은 별도 TODO가 크지만, 코드에는 `sttEngine/similarity_matrix.py`, `frontend/graph-view.html` 등 선행 파일이 존재해 "완전 미착수"보다는 "초기 구조 존재" 상태로 보는 편이 정확함.

## 4) 지금 당장 진행 권장 작업 (우선순위)

### P0 (이번 스프린트 즉시)
1. **TODO 정합성 정리**
   - TODO/GUI/Graph 문서에서 현재 코드 기준으로 체크 상태 재평가
   - 이미 완료된 항목과 미완료 항목을 분리해 가시성 확보
2. **검색 API 확장 우선 착수**
   - `TODO.md` 미완료 항목 중 사용자 체감이 큰 "검색 API 확장/성능 최적화"를 1순위로 진행
3. **UI 실시간 상태 표시 최소 기능(MVP)**
   - 작업 단계별 진행률 + 에러 재시도 버튼부터 구현

### P1 (단기)
1. **Graph 기능 MVP**
   - `/api/similarity-graph` 단일 엔드포인트 + `graph-view.html` 기본 렌더링
2. **텍스트 오버레이 접근성 개선**
   - 키보드 닫기(ESC), 포커스 트랩, ARIA 라벨
3. **테스트 구조 정비 시작**
   - 현재 존재하는 단일 테스트(`test_vocab_system.py`)에서 검색/워크플로우 API 테스트로 확장

### P2 (중기)
1. **프론트 상태관리 최적화**
   - `AppContext` 리렌더 범위 측정 후 domain hook 중심으로 재조정
2. **대시보드 레이아웃/반응형**
   - 정보 구조(업로드/큐/기록/검색) 재배치
3. **운영성 보강**
   - 스토리지 사용량, 자동 정리, 백업 등 데이터 관리 기능 추가

## 5) 추천 실행 순서 (2주 기준)
- 1~2일차: TODO 문서 정합성 정리 + 우선순위 재확정
- 3~6일차: 검색 API 확장 + 검색 UI 고급화(필터/하이라이트)
- 7~9일차: 실시간 상태 표시 MVP + 에러 재시도 UX
- 10~12일차: Graph API MVP + 그래프 뷰 기본 연동
- 13~14일차: 회귀 테스트/빌드 안정화, 다음 스프린트 백로그 확정

## 6) 검증 로그
- `python -m pytest -q test_vocab_system.py` 통과
- `npm run build` (frontend) 통과
