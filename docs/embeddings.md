# RecordRoute 임베딩과 요약 검색 (2026-03-31 코드 기준)

이 문서는 현재 구현된 summary embedding, backfill, similarity search 동작을 설명한다.
현행 구현은 파일 sidecar가 아니라 메타DB 저장을 사용한다.

## 1. 범위

현재 임베딩 기능의 대상은 job당 summary 문서 1개다.

- 입력 문서: 메타DB의 `summaries` 레코드
- 임베딩 단위: summary 문서 전체 1개
- 저장 방식: 메타DB의 `summary_embeddings`
- 검색 방식: brute-force cosine similarity

현재 범위 밖:

- transcript chunk 단위 검색
- ANN 인덱스
- query embedding 캐시
- 외부 vector DB

## 2. 모델 정책

summary와 embedding은 서로 다른 모델을 쓸 수 있다.

- summary 모델: `RECORDROUTE_LLAMA_MODEL`
- embedding 모델: `RECORDROUTE_LLAMA_EMBEDDING_MODEL`

기본값:

- summary: `ggml-org/gemma-3-4b-it-GGUF`
- embedding: `Qwen/Qwen3-Embedding-4B-GGUF`

환경변수 값이 실제 파일이면 local GGUF로 사용하고, 아니면 Hugging Face repo 문자열로 해석한다.

## 3. 저장 구조

임베딩 관련 저장은 다음 두 레벨로 나뉜다.

- `jobs.summary_embedding`
  - 검색 재사용 판단에 필요한 metadata
- `summary_embeddings`
  - 실제 벡터와 metadata

`SummaryEmbeddingRecord` 필드:

- `model_id`
- `text_sha256`
- `dimension`
- `normalized`
- `created_at`

실제 벡터는 `SummaryEmbeddingVectorRecord.vector`에 저장된다.
더 이상 `summary/embedding.json` 같은 sidecar 파일은 만들지 않는다.

## 4. 생성 흐름

1. summary와 embedding은 분리된 stage다.
2. summary 성공만으로 embedding이 자동 enqueue되지는 않는다.
3. embedding 트리거는 `POST /jobs/{job_id}/summary/embedding`, `POST /jobs/batch-process`의 `embedding|all`, CLI `embed-summaries`다.
4. 현재 summary 본문과 embedding metadata가 일치하면 `Reused`
5. queued/running task가 있으면 `Deduplicated`
6. 아니면 새 embedding task를 실행한다.

stale 판단 기준:

- embedding metadata 없음
- summary 없음
- 현재 summary 본문의 sha256과 `text_sha256` 불일치
- 현재 embedding 모델 id와 저장된 `model_id` 불일치
- DB의 metadata/vector 불일치

주의:

- `batch-process all`은 계획 시점에 summary가 이미 있는 job만 embedding 대상으로 잡는다.
- 같은 요청 안에서 미래 summary 결과를 예측해 embedding까지 예약하지는 않는다.

## 5. 검색 동작

`search_summaries()`와 `POST /summary/search`는 query 문자열을 같은 embedding 모델로 즉시 임베딩한다.

후보 선정 조건:

- completed job
- summary 존재
- embedding vector 존재
- 현재 embedding 모델과 `model_id` 일치
- `is_embedding_stale(...) == false`

후보 점수는 cosine similarity로 계산한다.
현재 summary와 어긋난 stale embedding은 검색 결과에서 제외된다.
