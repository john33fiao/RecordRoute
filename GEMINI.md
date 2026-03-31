# GEMINI.md

Gemini 작업 가이드입니다.

- **기준 문서:** 루트 `AGENTS.md`
- **아키텍처 기준:** `docs/architecture.md`
- **API 계약 기준:** `docs/API_Doc.md`
- **OpenAPI 상세 명세:** `docs/openapi.yaml`
- **임베딩/검색 기준:** `docs/embeddings.md`
- **외부 툴체인 감사:** `docs/API_Audit.md`

## 최소 운영 규칙
1. 공통 정책/규칙 변경은 반드시 `AGENTS.md`를 먼저 수정합니다.
2. 이 문서는 `AGENTS.md`의 참조용 요약만 유지합니다(중복 상세 금지).
3. 현재 코드베이스의 주요 확장 포인트는 `storage split(metadata DB + audio store)`, `queue/dictionary`, `summary embedding search`입니다.

## 빠른 참조 링크
- CLI 모드 정의: `rust/src/app/cli.rs`
- 서버 라우트 정의: `rust/src/server.rs`
- 인덱스 타입 정의: `rust/src/index/types.rs`
- 런타임/런처: `rust/src/launcher.rs`, `rust/src/runtime_root.rs`
- 저장/오디오: `rust/src/storage.rs`, `rust/src/audio_store.rs`
