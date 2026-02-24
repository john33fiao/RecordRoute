# AGENTS.md - RecordRoute 루트 작업 기준 (Rust 전환)

이 문서는 저장소 루트(`./`) 기준 에이전트 작업 표준입니다.

> 문서 동기화 메모: 2026-03-30 기준 운영 안정화 + 운영 점검 정례화(7.4.1~7.4.5) 완료 상태를 README/CLAUDE/GEMINI/TODO와 정렬.
> 동기화 포인트: runbook 저장 경로를 `docs/operations/weekly-drill/`로 통일, 정례화 정책/템플릿 신규 문서 반영.

## 1) 프로젝트 구조 인식

- `frontend/`: 운영 중인 프론트엔드 코드
- `docs/`: Rust 전환/설계 문서
- (현재 없음) `deprecated/`: 과거 Python 레거시 코드베이스 경로였으며, 현 저장소에는 포함되지 않습니다.

원칙:
- 신규 구현/문서화는 Rust 전환 목표를 기준으로 작성합니다.
- 현재 저장소에는 `deprecated/` 디렉터리가 없으므로 레거시 실행 경로를 기본 타깃으로 가정하지 않습니다.

## 2) Rust 전환 기본 방향

- 목표 아키텍처: **Rust 오케스트레이터 + C++ 엔진(독립 프로세스)**
- 엔진 연동: FFI 대신 내부 HTTP 계약 우선
- 큐: 단일 큐 금지, 엔진별 큐/동시성 분리
- 운영: 헬스체크/재시작/타임아웃/배압을 Rust 계층에서 명시적으로 관리
- 오디오 전처리: `ffmpeg` 외부 프로세스 대신 Rust `symphonia` 크레이트 기반 변환을 기본값으로 사용
- 자산 추적: `vendor/` 소스는 버전관리, `models/`는 raw 데이터 제외 후 manifest(`manifest.yml|yaml|json`)만 추적

세부 정책은 `docs/rust-cpp-backend-rewrite-plan.md`를 단일 기준으로 따르며, 아키텍처 기준선은 `docs/architecture.md`를 참조합니다.
배포/자산 세부 운영 기준은 `docs/deployment-asset-policy.md`를 참조합니다.
`main.rs` 분할/모듈화 실무 기준은 `docs/main-rs-modularization-guide.md`를 참조합니다.
운영 점검 정례화 정책은 `docs/operations/weekly-drill/README.md`를 참조합니다.
7.4.5 완료 조건 누적 추적은 `docs/operations/weekly-drill/STATUS.md`를 기준으로 갱신합니다.
실행 단위/의존성 추적은 `TODO/TODO.md`를 함께 참조합니다.

## 3) 문서 동기화 규칙

아래 파일은 항상 함께 최신화합니다.

- `AGENTS.md` (원본 기준)
- `CLAUDE.md` (요약)
- `GEMINI.md` (요약)
- `README.md` (사용자 관점 개요)

문서 간 충돌 방지 원칙:
- 상세 규칙은 `AGENTS.md`에만 둡니다.
- `CLAUDE.md`, `GEMINI.md`는 링크/체크리스트 중심으로 유지합니다.

## 4) 변경 우선순위


정렬 상태 메모:
- WBS `1.2 OpenAPI/API 계약 재정렬`은 구현 라우트/파라미터와 OpenAPI path/param 1:1 매핑 재검증 완료 전까지 재검토 상태로 관리합니다.
- 운영 점검 정례화(7.4.1~7.4.4)에는 계약 드리프트 주간 점검(구현↔OpenAPI path/param 대조 + CI 정적 계약 점검 확인 + 경로 파라미터 명칭 일치 검증 + 기준 엔드포인트 세트 `/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}` 고정)을 포함합니다.
- OpenAPI 계약(`docs/openapi.yaml`, `docs/swagger/openapi.yaml`)은 Rust 목표 엔드포인트 기준으로 유지합니다.
- OpenAPI 잡 상태 enum은 `queued|running|completed|failed|timeout|canceled|rejected`를 단일 기준으로 유지합니다.
1. 계약 안정성(API 응답 필드/에러 규약)
2. 운영 안전성(타임아웃, 큐 포화, 헬스체크)
3. 성능 최적화(검색/벡터/read-heavy 경로)
4. 전체 워크플로우 전환(`/process`)은 Go/No-Go 이후

## 5) 검증 규칙

문서 변경만 있을 때 최소 검증:
- Markdown 링크/구조 점검
- 저장소 상태 점검(`git status`)

코드 변경이 포함되면 해당 스택 검증 필수:
- 프론트: `cd frontend && npm run build`
- 레거시 Python(외부/별도 저장소에서만): 해당 저장소 테스트 규칙에 따라 `pytest` 또는 영향 범위 테스트
- Rust 코드 도입 시: `cargo test`, `cargo clippy`(도입 이후 필수)

## 5-1) RTD(배포준비) 스킬 적용 규칙

- 첫 코드 변경 전에 RTD Step 1~4(계획/검토/과도성 제거)를 수행하고, 각 단계 PASS/FAIL과 핵심 근거(최대 5줄)를 남깁니다.
- 구현 완료 후 배포(커밋) 전에 RTD Step 5~18을 순서대로 수행하고, 각 단계 PASS/FAIL과 핵심 근거(최대 5줄)를 남깁니다.
- FAIL이 발생하면 원인을 해결한 뒤 실패 단계부터 재수행합니다.
- 보안 리스크(권한/인증/인가/인젝션 등) 또는 롤백 경로가 불명확하면 PASS 판정을 금지합니다.
- Step 1~18 전체 PASS(READY) 상태에서만 커밋/PR 초안을 진행합니다.

## 6) 레거시 스코프 주의

- 현재 저장소 기준 `deprecated/`는 비어 있거나 존재하지 않을 수 있습니다.
- 향후 `deprecated/`가 재도입되어 하위 파일을 수정할 경우, 해당 경로의 `AGENTS.md`가 있으면 우선 준수합니다.

## 7) 금지/권장

금지:
- 전환 미완료 상태를 완료된 것처럼 문서화
- 존재하지 않는 실행 경로를 기본 사용법으로 제시

권장:
- “현재 상태”와 “목표 상태”를 문서에서 명확히 분리
- 레거시 참조 경로를 명시해 온보딩 혼선을 줄이기
