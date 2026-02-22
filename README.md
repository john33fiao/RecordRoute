# RecordRoute

RecordRoute는 음성/문서 처리 파이프라인을 **Rust 중심 아키텍처**로 전환 중인 프로젝트입니다.

현재 저장소는 아래 두 영역으로 나뉩니다.

- `deprecated/`: 기존 Python 기반 백엔드/워크플로우(레거시 기준선)
- `frontend/`: 현재 유지 중인 웹 프론트엔드

Rust 백엔드는 아직 본 저장소에 구현되지 않았고, 설계 및 전환 계획을 기준으로 단계적으로 이전합니다.

오디오 전처리/변환은 기존 `ffmpeg` 실행 방식 대신 Rust `symphonia` 크레이트 기반 구현을 목표 기준으로 문서화합니다.

## 문서 우선순위

1. 아키텍처/작업 규칙: `AGENTS.md`
2. 사용자/운영 개요: `README.md` (이 문서)
3. 전환 설계: `docs/rust-cpp-backend-rewrite-plan.md`
4. 전환 실행 WBS: `docs/rust-migration-wbs.md`
5. 레거시 코드 참고: `deprecated/current-codebase-overview.md`
6. 에이전트 요약: `CLAUDE.md`, `GEMINI.md`

## 현재 상태 (2026-02 기준)

- Python 기반 구 구현은 `deprecated/`로 이동되어 유지보수 기준선으로 사용합니다.
- Rust 오케스트레이터 + C++(llama.cpp/whisper.cpp) 엔진 분리 아키텍처를 목표로 합니다.
- 프론트엔드는 유지하되, 향후 Rust API 계약에 맞춰 점진적으로 연결합니다.

## 개발 시작

### 프론트엔드 실행

```bash
cd frontend
npm install
npm run dev
```

### 프론트엔드 빌드

```bash
cd frontend
npm run build
```

## Rust 전환 가이드

- 상세 목표/포트/큐/타임아웃/슈퍼비전 정책은 `docs/rust-cpp-backend-rewrite-plan.md`를 단일 기준으로 따릅니다.
- 실행 단위 일정/의존성 관리는 `docs/rust-migration-wbs.md`를 기준으로 추적합니다.
- 전환 우선순위는 **read-heavy API 및 검색 경로 최적화**를 먼저 수행하고, `/process` 전체 전환은 Go/No-Go 판단 이후 진행합니다.
- 기존 Python 동작과의 계약 호환(응답 필드/에러 규약/정렬/페이징)은 반드시 테스트로 고정합니다.

## 레거시 코드 다룰 때

- `deprecated/` 내부 문서(`deprecated/AGENTS.md`, `deprecated/CLAUDE.md`, `deprecated/GEMINI.md`)는 레거시 코드 수정 시에만 적용합니다.
- 루트 문서와 레거시 문서가 충돌하면, 수정 대상 디렉터리의 문서 스코프를 우선합니다.

## 주의

- 이 저장소 루트에는 아직 Rust 실행 코드(`Cargo.toml`)가 없습니다.
- 따라서 현재 CI/로컬 검증은 프론트엔드 중심이며, 백엔드 검증은 레거시(`deprecated/`) 또는 별도 Rust 저장소/브랜치에서 수행해야 합니다.
