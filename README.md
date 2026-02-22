# RecordRoute

RecordRoute는 음성/문서 처리 파이프라인을 **Rust 중심 아키텍처**로 전환 중인 프로젝트입니다.

현재 저장소는 아래 두 영역으로 나뉩니다.

- `frontend/`: 현재 유지 중인 웹 프론트엔드
- `docs/`: Rust 전환/설계 문서

Rust 백엔드는 스캐폴딩(실행 진입점/로깅) 단계까지 반영되어 있으며, API/엔진 연동은 설계 문서를 기준으로 단계적으로 이전합니다.


> 문서 동기화: 2026-02-22 기준 `TODO/TODO.md`, `docs/rust-cpp-backend-rewrite-plan.md` 상태와 정렬됨.

오디오 전처리/변환은 기존 `ffmpeg` 실행 방식 대신 Rust `symphonia` 크레이트 기반 구현을 목표 기준으로 문서화합니다.

## 문서 우선순위

1. 아키텍처/작업 규칙: `AGENTS.md`
2. 사용자/운영 개요: `README.md` (이 문서)
3. 아키텍처 기준선: `docs/architecture.md`
4. 전환 설계: `docs/rust-cpp-backend-rewrite-plan.md`
5. 배포/자산 정책: `docs/deployment-asset-policy.md`
6. 전환 실행 WBS: `TODO/TODO.md`
7. 에이전트 요약: `CLAUDE.md`, `GEMINI.md`

## 현재 상태 (2026-02 기준)

- Python 기반 구 구현은 현재 저장소에 포함되어 있지 않으며, 필요 시 별도 레거시 보관소를 참조합니다.
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

- 단일 진입점/엔진 경계 기준은 `docs/architecture.md`를 먼저 확인합니다.
- 상세 목표/포트/큐/타임아웃/슈퍼비전 정책은 `docs/rust-cpp-backend-rewrite-plan.md`를 단일 기준으로 따릅니다.
- 실행 단위 일정/의존성 관리는 `TODO/TODO.md`를 기준으로 추적합니다.
- 전환 우선순위는 **read-heavy API 및 검색 경로 최적화**를 먼저 수행하고, `/process` 전체 전환은 Go/No-Go 판단 이후 진행합니다.
- 기존 Python 동작과의 계약 호환(응답 필드/에러 규약/정렬/페이징)은 반드시 테스트로 고정합니다.

## 레거시 코드 다룰 때

- 현재 저장소에는 `deprecated/` 디렉터리가 없으므로 레거시 코드는 기본 작업 범위가 아닙니다.
- 레거시 이슈 재현이 필요하면 별도 레거시 보관소/브랜치에서 수행하고, 결과만 본 저장소 문서에 반영합니다.

## 주의

- 이 저장소 루트에는 Rust 실행 코드(`Cargo.toml`)가 존재합니다.
- 따라서 Rust 코드 변경 시 `cargo test`, `cargo clippy`를 포함한 검증을 수행해야 합니다.
