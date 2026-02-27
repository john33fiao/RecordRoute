# Tauri 설치/배포 자동화 통합 가이드 (WBS 9.4)

## 1) 목적과 범위

이 문서는 기존 설치/실행 스크립트(`scripts/install_*.sh|bat`, `scripts/run_*.sh|bat`)와
Tauri 배포 흐름을 **단일 사용자 플로우**로 연결하기 위한 운영 기준입니다.

- 목표: 사용자가 플랫폼별로 동일한 단계(`사전 점검 → 빌드/패키징 → 실행/검증 → 업데이트`)를 따른다.
- 비범위: Tauri 릴리스 채널/자동 업데이트 서버 구현 자체(9.5 게이트에서 최종 품질 판정).

## 2) 단일 사용자 플로우

### Step A. 사전 점검(설치 게이트)

1. Unix/macOS/Linux
   - `scripts/install_unix.sh --check`
2. Windows
   - `scripts\install_windows.bat --check`

검증 항목:
- 필수 도구(`npm`, `cargo`) 존재
- 필수 모델 env(`RECORDROUTE_DEFAULT_STT_MODEL`, `RECORDROUTE_DEFAULT_SUMMARIZE_MODEL`, `RECORDROUTE_DEFAULT_EMBED_MODEL`) 설정
- 모델 파일 존재(또는 pull 가능성 확인)

> 정책: `--check`가 실패하면 Tauri installer/업데이트 단계로 진행하지 않습니다.

### Step B. 빌드/패키징

1. 기존 Rust+Frontend 빌드
   - `scripts/install_unix.sh` 또는 `scripts\install_windows.bat`
2. Tauri 패키징 기준 적용
   - 권한/보안/환경변수 정책은 `docs/tauri-packaging-security-baseline.md`를 따릅니다.
   - 런타임 endpoint 계약은 `docs/tauri-frontend-backend-contract-alignment.md`를 따릅니다.

### Step C. 실행/검증

- 개발/운영 점검 실행
  - Unix/macOS/Linux: `scripts/run_unix.sh [--release]`
  - Windows: `scripts\run_windows.bat [--release]`
- Tauri lifecycle 회귀는 `src/bin/tauri_lifecycle_probe.rs` + `.github/workflows/tauri-lifecycle-poc.yml`로 검증합니다.

### Step D. 업데이트(업그레이드 재진입)

- 업데이트 전 동일한 사전 점검(`--check`)을 재실행합니다.
- 모델/의존성 누락이 감지되면 업데이트를 중단하고 복구 후 재시도합니다.
- 종료/재시작 정합성은 `engine_manager` 상태 전이(`Booting/Ready/Degraded/Stopping/Stopped`) 기준을 유지합니다.

## 3) 빌드 산출물 포함/제외 정책

### 포함(배포 후보)

- Frontend 정적 빌드 산출물(`frontend/dist/**`)
- Rust 릴리스 바이너리(`target/release/recordroute-orchestrator[.exe]`)
- Swagger 분리 실행 스크립트(`scripts/run-swagger.sh`)
- 운영/배포 문서(`README.md`, `docs/deployment-asset-policy.md`, 본 문서)

### 제외(버전관리/배포 패키지 공통)

- Rust 중간 산출물 및 캐시(`target/**` 전반)
- 앱 패키징 임시 산출물(로컬 빌드 디렉터리, 서명 전 임시 파일)
- 모델 raw 데이터(`models/**`), 단 `manifest.yml|yaml|json`은 추적

세부 ignore 원칙은 `.gitignore` 및 `docs/deployment-asset-policy.md`를 기준으로 유지합니다.

## 4) 운영 체크리스트(9.4 완료 기준)

- [x] 기존 설치/실행 스크립트와 Tauri 배포 흐름의 단계 연결(사전 점검→빌드→검증→업데이트) 문서화
- [x] 빌드 산출물 포함/제외 정책을 README/배포 문서와 동기화
- [x] 설치 게이트(`--check`)를 Tauri installer/업데이트 진입 조건으로 고정

## 5) 리스크/롤백

- 리스크: 모델 env 누락 또는 모델 파일 부재 시 설치/업데이트 실패.
- 완화: `--check` 선실행, 누락 시 pull 또는 중단을 명시적으로 선택.
- 롤백: 릴리스 바이너리/프론트 산출물 교체 배포 시 이전 버전 아티팩트로 즉시 복원.
