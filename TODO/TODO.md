# TODO Master (신규 백로그)

- 마지막 점검일: 2026-02-18
- 점검 기준: `README.md`, `CLAUDE.md`, `GEMINI.md`, `TODO/STATUS_REVIEW.md`, 주요 구현 파일(`sttEngine/http_api/handler.py`, `sttEngine/http_api/routes/search_routes.py`, `frontend/scripts/benchmark-similarity-graph.mjs`, `frontend/src/*`)
- 원칙: **완료 항목은 TODO에서 제거**하고, 실행이 필요한 항목만 유지

---

## 1) P0 (즉시 착수)

- [ ] `handler.py` 책임 분리 (라우트 단위 모듈화 마무리)
  - 점검 결과: `sttEngine/http_api/routes/*` 분리는 진행됐지만, `UploadHandler` 내부에 업로드 파싱/저장/응답 조합 로직이 크게 남아 있음
  - 잔여 작업: 업로드 처리 책임 분리 + 회귀 테스트 고정

- [ ] 그래프 대용량 성능 전략 고도화 (ANN/근사 최근접)
  - 점검 결과: LSH 기반 후보 축소(`neighbor_strategy=lsh/auto`)는 도입되었으나, 정확도 기준/전용 인덱스/대용량 벤치 기준선은 미완료
  - 잔여 작업: 정확도 허용오차 정의 + 성능 목표 확정 + 인덱스 확장 여부 결정

## 2) P1 (다음 스프린트)

- [ ] 검색 고급 필터 UX 정리
  - 범위: 현재 노출된 필터(기간/정렬/최소점수/상태/파일타입)의 초기값, 검증 메시지, 페이지네이션 UX 다듬기

- [ ] 오버레이 접근성 강화
  - 범위: 오버레이별 ARIA 라벨 점검, 키보드 포커스 흐름 회귀 테스트

- [ ] 그래프 `neighbor_strategy` 사용자 선택 UI 제공
  - 범위: `auto/exact/lsh` 선택 컨트롤, 요청 파라미터 연동, 응답 meta 노출 정리

- [ ] 실패 복구 정책 표준화
  - 범위: 재시도 가능 오류 코드 표준화 + UI 가이드

- [ ] API 버전 관리/OpenAPI 초안
  - 범위: 버전 전략, `/process`/`/search`/`/progress` 계약서 초안

- [ ] 화자 분리(Speaker Diarization) 도입 설계/구현
  - 범위: 실행 로드맵(`TODO/화자분리_로드맵.md`) 기준으로 Phase 0~4 순차 적용

- [ ] 훅/컴포넌트 테스트 보강
  - 범위: `useTaskQueue`, `SearchPanel`, `TextOverlay`, `SimilarityGraphPanel`

## 3) P2 (중기 개선)

- [ ] 컨텍스트 리렌더 최적화
- [ ] 대용량 히스토리 가상 스크롤
- [ ] 그래프 증분 업데이트 전략
  - 범위: 증분 캐시 정책 실측(메모리/TTL/무효화) 및 벤치마크 시나리오 고도화
- [ ] 스토리지 모니터링/자동 정리/백업 정책
- [ ] 처리 통계 대시보드용 집계 API
- [ ] 레거시 코드 정리 (`frontend/legacy/*` 유지보수 범위 한정)

---

## 4) 의존관계 체크포인트

- 검색 고급화(P1) ← 검색 API 계약(파라미터/응답) 명세
- 운영 지표(P2) ← 로깅/메트릭 스키마 정리

## 5) 메모

- React 기준 신규 작업은 `frontend/src/*` 중심으로 관리한다.
- `frontend/legacy/*`는 fallback 유지 목적의 최소 변경만 허용한다.
