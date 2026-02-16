# Similarity Graph Benchmark Baseline

- Generated at: 2026-02-16T02:55:59.014Z
- Response source: mock delay (8 ms)
- Iterations per scenario: 10
- Dataset sizes: 100, 500, 1000

## Summary

| Documents | Edges | Response Avg/P95 (ms) | Render Avg/P95 (ms) |
| --- | --- | --- | --- |
| 100 | 294 | 8 / 8 | 4.32 / 11.61 |
| 500 | 1494 | 8 / 8 | 26.3 / 42.45 |
| 1000 | 2994 | 8 / 8 | 84.98 / 129.74 |

## Measurement definition

- **Response**: `/api/similarity-graph` request/response round trip time.
- **Render**: SimilarityGraphPanel의 핵심 연산(초기 좌표 시드 + force tick 1회 + SVG element 구성 루프)을 Node 런타임에서 실행한 시간.
- 현재 렌더 측정은 브라우저 페인트 시간 전체가 아니라, 컴포넌트의 자료구조/루프 비용에 대한 CPU 기준선입니다.
