# AGENTS.md - RecordRoute 코딩 에이전트 가이드

이 문서는 RecordRoute 코드베이스를 수정하는 에이전트를 위한 최신 기준 문서입니다.

## 0. 문서 우선순위
- 사용자/운영 안내는 `README.md` 기준
- 에이전트 개발 규칙은 `AGENTS.md` 기준
- `CLAUDE.md`, `GEMINI.md`는 `AGENTS.md` 요약본

문서 동기화 규칙:
- API/경로/실행 방식 변경 시 `README.md`와 `AGENTS.md` 동시 갱신
- 에이전트 문서(`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`) 수정 시 **세 파일을 같은 커밋에서 동기화**

작업 시작 전 필수 확인:
- `README.md`
- `AGENTS.md`
- `CLAUDE.md`, `GEMINI.md`
- `TODO/TODO.md`, `TODO/GUI.md`, `TODO/llama-migration.md`, `TODO/rust-migration.md`, `TODO/DOCKER_OPENAPI_DEPLOY_PLAN.md`

---

## 1. 현재 개발 정책 (중요)

- 기존 Python 백엔드(`sttEngine/*`)는 **deprecated 유지 구역**으로 간주합니다.
- 백엔드 신규 구현은 **Rust 신규 백엔드(그린필드)** 방향으로 진행합니다.
- 레거시 Python 코드는 아래 경우에만 수정합니다.
  1) 장애/보안 핫픽스
  2) 데이터 호환성 보정
  3) 전환(마이그레이션)용 최소 브리지
- 기능 개발 요청은 원칙적으로 Rust 기준 설계/문서/테스트를 우선 작성합니다.

---

## 2. 레포 구조 요약

- 레거시 백엔드(Deprecated): `sttEngine/*`
- 프론트엔드(Active): `frontend/src/*`
- 레거시 프론트 fallback: `frontend/legacy/*`
- 문서/계획:
  - Rust 전환 계획: `TODO/rust-migration.md`
  - Rust 전환 WBS: `docs/rust-migration-wbs.md`

---

## 3. 작업 규칙

- 라우트/스키마 변경 시 프론트 API 클라이언트/타입/테스트 동기화
  - `frontend/src/api/client.ts`
  - `frontend/src/api/types.ts`
- 경로는 `normalize_record_path`, `resolve_record_path`, `to_record_path` 유틸 우선 사용
- 워크플로우 오류 규약 유지: `error`, `error_code`, `retryable`, `failed_step`
- 런타임 영향 변경 시 문서 즉시 반영

---

## 4. 테스트

기본:
- `pytest`

핵심 회귀:
- `pytest tests/http_api/test_workflow.py`
- `pytest tests/http_api/test_search.py`
- `pytest tests/server/test_queue.py`
- `pytest tests/test_vocab_system.py`

프론트:
- `cd frontend && npm run build`

---

## 5. 문서 최신화 체크리스트

- `TODO/STATUS_REVIEW.md`는 현재 저장소 기준 파일이 아니므로 참조하지 않음
- 백로그 기준은 `TODO/TODO.md`
- 트랙 상세는 각 TODO 문서에서 확인
- 중복 기술 최소화, 상세 기준은 `AGENTS.md`에 일원화
