# CLAUDE.md

Claude 작업 가이드입니다.

- **기준 문서:** 루트 `AGENTS.md`
- **아키텍처 기준:** `docs/architecture.md`
- **API 계약 기준:** `docs/API_Doc.md`
- **OpenAPI 상세 명세:** `docs/openapi.yaml`

## 최소 운영 규칙
1. 공통 정책/규칙 변경은 반드시 `AGENTS.md`를 먼저 수정합니다.
2. 이 문서는 `AGENTS.md`의 참조용 요약만 유지합니다(중복 상세 금지).
3. 현재 코드베이스의 주요 확장 포인트는 `summary embedding`(CLI/API/인덱스)입니다.

## 빠른 참조 링크
- CLI 모드 정의: `rust/src/app/cli.rs`
- 서버 라우트 정의: `rust/src/server.rs`
- 인덱스 타입 정의: `rust/src/index/types.rs`

## 스킬

### Codex Workflow
- 트리거: 사용자 메시지에 "지피티", "코덱스" 또는 "codex" 포함, 또는 코드 작업 요청
- 파일: `.claude/agents/skills/codex-workflow.md`
- 요약: Claude는 오케스트레이션만 담당하고 실제 코드 작업은 Codex에 위임한다.
