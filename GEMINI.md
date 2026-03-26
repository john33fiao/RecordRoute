# GEMINI.md

Gemini 작업 가이드입니다.

- 기본 작업 규칙/코드베이스 기준 문서는 루트 `AGENTS.md`입니다.
- 아키텍처 최신 설명은 `docs/architecture.md`를 우선 참조합니다.
- API 스펙이 필요하면 `docs/openapi.yaml`을 확인합니다.

## 우선 참조 순서
1. `AGENTS.md` (공통 기준)
2. `docs/architecture.md` (현재 코드 구조)
3. 변경 대상 코드/문서 (`rust/src/*`, `docs/openapi.yaml`, `docs/API_TODO.md`)

## 빠른 체크리스트
- 작업 전후로 재사용/중복제거 규칙(FFmpeg/STT/Summary/Model preparation)이 유지되는지 확인합니다.
- API 변경 시 `/jobs/upload`, `/jobs/by-source`, `/jobs/{job_id}/files/*`까지 영향 범위를 점검합니다.
- 문서 갱신 시 공통 정책은 `AGENTS.md`를 먼저 갱신하고, 이 파일은 참조/요약만 유지합니다.

## 메모
- 공통 규칙 변경은 이 파일보다 `AGENTS.md`를 먼저 수정하세요.
