# Deployment & Asset Policy (WBS 1.2.x ~ 2.2.x)

이 문서는 `docs/rust-cpp-backend-rewrite-plan.md`의 WBS 1.2.x(구성요소 분리), 2.2.x(폴더/자산 정책)를 실행 가능한 기준으로 구체화한 산출물입니다.

## 1) 배포 단위 분리 기준 (WBS 1.2.2)

### API 서버 (Rust orchestrator)
- 포트: `:18000`
- 책임:
  - 외부 요청 진입점 (`/jobs`, `/healthz`, `/readyz`)
  - 엔진별 큐 라우팅 및 backpressure
  - 엔진 헬스체크/재시작/타임아웃 감독
- 비책임:
  - Swagger UI 정적 자산 제공

### Swagger 서버 (문서 전용 프로세스)
- 포트: `:14000`
- 책임:
  - Swagger UI 제공
  - OpenAPI 원본은 `http://localhost:18000/openapi.json` 참조
- 비책임:
  - API 트래픽 처리
  - 엔진 상태 판단

### 운영 분리 원칙
- API와 Swagger는 **서로 다른 프로세스/서비스 단위**로 배포한다.
- Swagger 장애가 API readiness에 영향을 주지 않아야 한다.
- API 배포 롤백 시 Swagger 배포를 강제하지 않는다(역도 동일).

## 2) 엔진 역할 분리 기준 (WBS 1.2.1)

- `llama-text` (`127.0.0.1:18101`)
  - `/models/text/*.gguf`
  - 요약/교정 계열 요청 전용
- `llama-embed` (`127.0.0.1:18102`)
  - `/models/embed/*.gguf`
  - 임베딩 요청 전용
  - pooling 정책 명시 (`mean` 또는 `cls`)
- `whisper-server` (`127.0.0.1:18103`)
  - `/models/stt/*`
  - STT 추론 전용

요약/임베딩을 동일 엔진 프로세스에서 혼합 처리하지 않는다.

## 3) 저장소 구조 기준 (WBS 2.2.1)

```text
/
├── Cargo.toml
├── /src
├── /vendor
│   ├── /llama.cpp
│   └── /whisper.cpp
├── /models
│   ├── /text
│   ├── /embed
│   └── /stt
└── /target 또는 /build (gitignore)
```

규칙:
- `/vendor/*`는 소스만 관리하고 빌드 산출물/실행파일은 커밋하지 않는다.
- `/models/**`의 원본 모델 파일은 커밋하지 않는다.
- 모델 추적은 `manifest.yml` 또는 `manifest.json`으로 대체한다.

## 4) Git 추적 정책 (WBS 2.2.2)

### 필수 ignore 항목
- `/build`
- `/target`
- `/models/**` (원본 모델 데이터 전역 제외)
- 엔진 바이너리/오브젝트/캐시

### 버전관리 허용 항목
- `/vendor/llama.cpp/**` 소스
- `/vendor/whisper.cpp/**` 소스
- `/models/**/manifest.yml` 또는 `manifest.yaml` 또는 `manifest.json`
- 배포/실행 스크립트, 설정 템플릿

## 5) WBS 완료 판정 체크포인트

- [ ] API(`:18000`)와 Swagger(`:14000`)를 독립 프로세스로 실행 가능한 배포 정의가 준비되었는가?
- [ ] `llama-text`/`llama-embed`가 모델 경로와 요청 타입 기준으로 완전히 분리되었는가?
- [x] 저장소에 `/vendor`, `/models`, 빌드 산출물 정책이 `.gitignore`와 함께 반영되었는가?
- [x] 모델 원본 없이 manifest만으로 버전/체크섬 추적이 가능한가?

## 6) Tauri 설치/배포 통합 정책 (WBS 9.4)

- 단일 사용자 플로우(사전 점검→빌드/패키징→실행 검증→업데이트)는
  `docs/tauri-install-deploy-unified-flow.md`를 기준으로 유지합니다.
- 설치 게이트는 `scripts/install_*.sh|bat --check` 결과를 단일 기준으로 사용합니다.
- `--check` 실패(필수 모델/의존성 누락) 상태에서는 Tauri installer/업데이트 단계를 시작하지 않습니다.
- 배포 후보에는 `frontend/dist/**`, `target/release/recordroute-orchestrator[.exe]`, 운영 문서를 포함합니다.
- 배포/버전관리 공통 제외 대상은 `target/**` 중간 산출물, 패키징 임시 산출물, 모델 raw 데이터입니다.

## 7) 관련 문서

- 기준 계획: `docs/rust-cpp-backend-rewrite-plan.md`
- 아키텍처 기준선: `docs/architecture.md`
- 실행 단위 추적: `TODO/TODO.md`
- Tauri 통합 가이드: `docs/tauri-install-deploy-unified-flow.md`
