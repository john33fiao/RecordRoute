# Graph 백로그 (문서 유사도 네트워크)

- 마지막 점검일: 2026-02-17
- 점검 방법: 코드 기준 재검토(`similarity_matrix`, `similarity_routes`, `SimilarityGraphPanel`, 벤치 스크립트/산출물)
- 대조 기준 파일: `sttEngine/similarity_matrix.py`, `sttEngine/http_api/routes/similarity_routes.py`, `frontend/src/components/SimilarityGraphPanel.tsx`, `frontend/scripts/benchmark-similarity-graph.mjs`, `docs/perf/similarity-graph-baseline.*`

## 1) 구현 현황 재평가

| 항목 | 상태 | 우선순위 | 의존관계 | 근거 |
|---|---|---|---|---|
| 유사도 그래프 계산 모듈(`similarity_matrix.py`) | 완료 | - | - | 코사인 유사도, threshold/max_neighbors, 캐시, 서브그래프 제공 |
| 그래프 API(`/api/similarity-graph`, `/api/documents/metadata`) | 완료 | - | - | GET 엔드포인트 동작 + `sampling`/`max_nodes` 파라미터 처리 |
| React 그래프 패널 메인 탭 통합 | 완료 | - | - | `App.tsx` `graph` 탭 + `SimilarityGraphPanel` 연동 |
| 인터랙션(드래그/줌/팬) | 완료 | - | - | 패널에서 휠 줌/배경 팬/노드 드래그 지원 |
| 기본 필터(min_similarity/max_nodes) | 완료 | - | - | 패널 제어값이 API 요청 파라미터로 연결 |
| 노드 스타일(타입/중심성) + 엣지 가중치 시각화 | 완료 | - | - | 타입별 컬러, 중심성 기반 반지름, edge opacity/두께 + 범례 UI 반영 |
| 성능 계측 자동화(100/500/1000 문서) | 완료 | - | - | `benchmark-similarity-graph.mjs` + `docs/perf/*` 산출물 유지 |
| 증분 유사도 재사용(변경 없는 문서쌍 재계산 생략) | 완료 | - | - | `_INCREMENTAL_STATE` + fingerprint 기반 `reused_pairs/computed_pairs` 계산 |
| 고급 필터(문서타입/기간/키워드) | 미착수 | P1 | metadata 확장 + UI 컨트롤 | 현재 UI/API는 similarity/max_nodes(+sampling) 중심 |
| 대용량 성능 전략(O(n²) 근본 대응: ANN/근사 최근접) | 미착수 | P0 | 인덱스/정확도 기준/벤치 시나리오 | 샘플링/증분은 있으나 유사도 계산 자체는 여전히 pairwise |

## 2) 유효 백로그 (다음 스프린트)

| 항목 | 상태 | 우선순위 | 의존관계 |
|---|---|---|---|
| 필터 UI(threshold + 날짜/타입/키워드) | 미착수 | P1 | API 파라미터 + metadata 필드 확장 |
| sampling 전략 선택 UI(recent/random/hybrid) 노출 | 미착수 | P1 | 프론트 컨트롤 + API 파라미터 동기화 |
| 증분 업데이트 고도화(메모리/TTL/캐시 정책) | 완료 | P2 | - |
| 근사 최근접(ANN) 또는 후보 축소 전략 PoC | 미착수 | P0 | 정확도 허용오차 + 성능 목표 합의 |
| 벤치마크 확장(실서버 + 대용량 구간 2k/5k) | 부분완료 | P2 | 실서버 계측 환경 |

## 3) 최근 완료 항목

| 항목 | 완료 근거 |
|---|---|
| 성능 계측(문서 100/500/1000 응답시간) 자동화 | `frontend/scripts/benchmark-similarity-graph.mjs`, `docs/perf/similarity-graph-baseline.json`, `docs/perf/similarity-graph-baseline.md` |
| 엣지 범례/스케일 표준화 + 노드 스타일 개선 | `frontend/src/components/SimilarityGraphPanel.tsx` |
| 증분 유사도 행렬 재사용(변경 없는 문서 쌍 score 재계산 생략) | `sttEngine/similarity_matrix.py` (`_build_similarity_matrix`, `_INCREMENTAL_STATE`) |
| 그래프 응답 메타 진단 정보 확장(`sampling`, `incremental`) | `sttEngine/similarity_matrix.py`, `sttEngine/http_api/routes/similarity_routes.py` |
| 증분 상태/그래프 캐시 정책(TTL + max entries) 및 진단 meta.cache 추가 | `sttEngine/similarity_matrix.py`, `.env.example`, `README.md` |
| 벤치마크 스크립트 대용량 옵션(`--dataset-sizes=100,500,1000,2000,5000`) 추가 | `frontend/scripts/benchmark-similarity-graph.mjs` |

## 4) 실행 순서

1. **P0** O(n²) 병목 완화를 위한 ANN/후보축소 PoC 및 정확도/성능 기준 정의
2. **P1** 사용자 필터(날짜/타입/키워드) + sampling 선택 UI/API 동기화
3. **P2** 증분 캐시 정책(메모리/TTL/무효화) 및 벤치마크 시나리오 확장
