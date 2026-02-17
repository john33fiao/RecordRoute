# Graph 백로그 (문서 유사도 네트워크)

- 마지막 점검일: 2026-02-17
- 대조 기준 파일: `sttEngine/similarity_matrix.py`, `sttEngine/http_api/routes/similarity_routes.py`, `frontend/src/components/SimilarityGraphPanel.tsx`

## 1) 구현 현황 재평가

| 항목 | 상태 | 우선순위 | 의존관계 | 근거 |
|---|---|---|---|---|
| 유사도 그래프 계산 모듈(`similarity_matrix.py`) | 완료 | - | - | 코사인 유사도, threshold/max_neighbors, 캐시, 서브그래프 제공 |
| 그래프 API(`/api/similarity-graph`, `/api/documents/metadata`) | 완료 | - | - | GET 엔드포인트 동작 |
| React 그래프 패널 메인 탭 통합 | 완료 | - | - | `App.tsx` `graph` 탭 + `SimilarityGraphPanel` 연동 |
| 인터랙션(드래그/줌/팬) | 완료 | - | - | 패널에서 휠 줌/배경 팬/노드 드래그 지원 |
| 엣지 가중치 시각화 | 부분완료 | P1 | 범례/스케일 표준화 | opacity/두께 반영, 범례 미구현 |
| 필터(문서타입/기간/키워드) | 미착수 | P1 | metadata 확장 + UI 컨트롤 | 현재 similarity/max_nodes 중심 |
| 대용량 성능 전략(O(n²) 대응) | 부분완료 | P0 | 샘플링/근사 최근접/증분 계산 설계 | `max_nodes` 샘플링(`recent/random/hybrid`) + 벤치 기준선은 존재하나, 근사 최근접/증분 계산은 미구현 |

## 2) 유효 백로그 (다음 스프린트)

| 항목 | 상태 | 우선순위 | 의존관계 |
|---|---|---|---|
| 엣지 범례 + 두께 스케일 표준화 | 미착수 | P1 | 스코어 스케일 규칙 |
| 노드 스타일(문서 타입/중심성 반영) | 미착수 | P1 | metadata 확장 |
| 필터 UI(threshold + 날짜/타입) | 미착수 | P1 | API 파라미터 정리 |
| 증분 업데이트 전략(신규 문서 추가 시 부분 갱신) | 미착수 | P2 | 인덱스/캐시 구조 변경 |

## 3) 최근 완료 항목

| 항목 | 완료 근거 |
|---|---|
| 성능 계측(문서 100/500/1000 응답시간) 자동화 | `frontend/scripts/benchmark-similarity-graph.mjs`, `docs/perf/similarity-graph-baseline.json`, `docs/perf/similarity-graph-baseline.md` |

## 4) 실행 순서

1. **P1** 스타일/범례/필터 고도화
2. **P2** 증분 계산 고도화
