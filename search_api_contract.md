# Search API Contract (Frontend 연동용)

## 엔드포인트
- `GET /search?q=<query>&start=<iso>&end=<iso>&sort_by=<similarity|date>&sort_order=<asc|desc>&min_score=<float>&page=<int>&page_size=<int>`

## 응답 스키마
기존 하위호환 필드(`keywordMatches`, `similarDocuments`)는 유지합니다.

```ts
interface SearchResponse {
  keywordMatches: KeywordMatch[];            // 기존 필드 (호환)
  similarDocuments: SimilarDocument[];       // 기존 필드 (호환)

  sort?: {
    by: string;      // similarity | date
    order: string;   // asc | desc
  };

  scoreBreakdown?: {
    keywordWeight: number; // 현재 0.0
    vectorWeight: number;  // 현재 1.0
  };

  pagination?: {
    page: number;
    pageSize: number;
    returned: number;
    hasNext: boolean;
  };

  timing?: Record<string, number>; // ms 단위
  performanceTargetMs?: {
    keyword_only_p95: number;
    vector_only_p95: number;
    hybrid_with_date_filter_p95: number;
  };

  cache?: {
    hit: boolean;
  };
}

interface SimilarDocument {
  file_uuid?: string;
  file: string;
  display_name?: string;
  score: number;
  uploaded_at?: string;
  source_filename?: string;
  link: string;
  record_id?: string;
  score_breakdown?: {
    vector_similarity: number;
    keyword_overlap: number;
    composite: number;
  };
}
```

## 호출 경로 및 병목 포인트
`/search` 요청은 다음 순서로 처리됩니다.
1. `sttEngine/http_api/handler.py`에서 쿼리 파라미터 파싱.
2. 키워드 검색: `collect_searchable_documents` + `collect_keyword_matches`.
3. 벡터 검색: `sttEngine/vector_search.py::search`.
4. 캐시 확인/저장: `sttEngine/search_cache.py`.

관측된 핵심 병목:
- 키워드 검색의 파일 본문 반복 읽기(I/O)
- 벡터 검색의 임베딩 생성(`embed_text_ollama`)과 벡터 파일 스캔(`np.load` 반복)

## 성능 목표 (P95)
- 키워드만 사용: `<= 120ms`
- 유사도 검색만 사용: `<= 450ms`
- 날짜 필터 + 유사도 조합: `<= 650ms`

## 캐시 키 정책
다음 요소를 모두 포함해 캐시 오염을 방지합니다.
- query, top_k
- date filter(start/end)
- min_score
- sort(by/order)
- pagination(page/page_size)
