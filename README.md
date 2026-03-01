# RecordRoute

RecordRoute는 **로컬 환경에서 음성/문서 처리 파이프라인을 실행**하는 제품입니다.
현재 저장소는 Rust 오케스트레이터 전환을 진행 중이며, 사용자는 웹 GUI를 통해 업로드·처리·조회·검색·설정을 수행할 수 있습니다.

> 상태 안내: 이 저장소는 전환 진행 중이며, 설치/실행 경로는 `scripts/install_*`, `scripts/run_*`를 기준으로 관리됩니다.

## 1) 이 README의 목적

이 문서는 **신규 사용자용 운영 매뉴얼**입니다.
아래 순서를 따르면:

1. 저장소 clone
2. 필수 도구 설치
3. 로컬 모델 배치
4. 설치 게이트 점검/빌드
5. GUI 실행
6. 실제 작업(업로드→처리→결과 확인)

까지 한 번에 수행할 수 있습니다.

---

## 2) 사전 준비

### 지원 OS

- Linux / macOS: `scripts/install_unix.sh`, `scripts/run_unix.sh`
- Windows: `scripts/install_windows.bat`, `scripts/run_windows.bat`

### 필수 도구

- Git
- Node.js + npm
- Rust (`cargo`)
- (선택) `huggingface-cli` — 모델 자동 다운로드를 사용할 때 필요

### 필수 환경변수(설치 게이트)

설치 스크립트는 아래 3개 모델 환경변수가 없으면 실패합니다.

- `RECORDROUTE_DEFAULT_STT_MODEL`
- `RECORDROUTE_DEFAULT_SUMMARIZE_MODEL`
- `RECORDROUTE_DEFAULT_EMBED_MODEL`

기본 예시는 `.env.example`에 포함되어 있습니다.

---

## 3) 설치: clone부터 빌드까지

## 3-1) 저장소 clone

```bash
git clone <YOUR_REPOSITORY_URL>
cd RecordRoute
```

> 사내/개인 포크 URL을 사용하세요.

## 3-2) 환경변수 파일 준비

루트의 `.env.example`를 기준으로 값을 준비합니다.

```bash
cp .env.example .env
```

운영 쉘에서 `.env`를 로드하거나(예: `set -a; source .env; set +a`) CI/쉘 프로파일로 주입합니다.

## 3-3) 모델 파일 준비

모델 파일은 아래 경로에 배치합니다.

- STT: `models/stt/`
- 요약(LLM): `models/text/`
- 임베딩: `models/embed/`

환경변수에는 **절대경로 또는 파일명**을 넣을 수 있습니다.

예)

- `RECORDROUTE_DEFAULT_STT_MODEL=ggml-large-v3-turbo.bin`
- `RECORDROUTE_DEFAULT_SUMMARIZE_MODEL=gemma-3-4b-it-Q5_K_M.gguf`
- `RECORDROUTE_DEFAULT_EMBED_MODEL=bge-m3-Q8_0.gguf`

### (선택) 모델 자동 pull 사용

설치 시 모델이 없으면 자동 다운로드를 사용할 수 있습니다.

- Unix: `scripts/install_unix.sh --yes-pull`
- Windows: `scripts\install_windows.bat --yes-pull`

이때 아래 repo 변수도 지정해야 합니다.

- `RECORDROUTE_STT_MODEL_REPO`
- `RECORDROUTE_SUMMARIZE_MODEL_REPO`
- `RECORDROUTE_EMBED_MODEL_REPO`

## 3-4) 설치/빌드

### Linux/macOS

```bash
bash scripts/install_unix.sh
```

선행 점검만 하려면:

```bash
bash scripts/install_unix.sh --check
```

### Windows (PowerShell/CMD)

```bat
scripts\install_windows.bat
```

선행 점검만 하려면:

```bat
scripts\install_windows.bat --check
```

---

## 4) 실행: GUI 열기

## 4-1) 개발 모드(권장 시작점)

### Linux/macOS

```bash
bash scripts/run_unix.sh
```

### Windows

```bat
scripts\run_windows.bat
```

기본적으로:

- Rust API 서버 실행
- 프론트 dev 서버 실행

이 동시에 시작됩니다.

## 4-2) 릴리스 바이너리 모드

설치 단계에서 `cargo build --release`가 완료된 뒤 사용합니다.

### Linux/macOS

```bash
bash scripts/run_unix.sh --release
```

### Windows

```bat
scripts\run_windows.bat --release
```

## 4-3) 브라우저 접속

프론트 개발 서버 기본 포트는 `3000`입니다.

- `http://localhost:3000`

> 참고: 프론트 dev proxy 기본 타깃은 `http://localhost:8080`입니다. 오케스트레이터 기본 포트(18000)를 사용할 경우 `VITE_API_BASE_URL=http://127.0.0.1:18000` 또는 `VITE_TAURI_BACKEND_URL=http://127.0.0.1:18000`를 지정해 맞춰주세요.

---

## 5) 실제 GUI 사용 절차 (사용자 매뉴얼)

아래는 운영자가 실제로 따라야 하는 최소 사용자 플로우입니다.

### Step A. 모델 인식 확인

1. GUI 접속 후 **SettingsDialog**를 엽니다.
2. STT/요약/임베딩 모델 목록에 준비한 모델이 노출되는지 확인합니다.
3. 각 항목을 저장합니다.

성공 기준:

- 저장 후 설정이 유지되고, 이후 작업 생성이 가능해야 합니다.

### Step B. 파일 업로드 및 작업 생성

1. 메인 화면 **UploadSection**으로 이동합니다.
2. 오디오 또는 텍스트 파일을 선택합니다.
3. **작업 생성** 버튼을 누릅니다.
4. **JobQueue**에서 신규 작업이 `queued` → `running`으로 전이되는지 확인합니다.

성공 기준:

- 작업 항목 생성 + 상태 전이 시작.

### Step C. 처리 결과 확인

1. **HistoryPanel**에서 완료(`completed`) 작업을 선택합니다.
2. STT 텍스트/요약 결과를 확인합니다.
3. 필요 시 동일 파일을 다시 올려 재현성(지연·결과 품질)을 점검합니다.

성공 기준:

- `completed` 상태 도달 + 결과 데이터 표시.

### Step D. 검색/탐색 검증

1. **SearchPanel**에서 키워드 검색을 실행합니다.
2. **SimilarityGraphPanel**에서 연관 노드/문서 연결을 확인합니다.
3. 필요한 결과를 다시 열어 원문/요약을 대조합니다.

성공 기준:

- 검색 결과 노출 + 그래프 탐색 가능.

### Step E. 운영 제어(선택)

- 실패(`failed`/`rejected`/`canceled`) 작업은 원인 확인 후 재시도합니다.
- 종료 전 작업 큐 상태를 확인하고, 실행 터미널에서 정상 종료(Ctrl+C)합니다.

---

## 6) 자주 발생하는 문제와 해결

### 6-1) 설치 단계에서 모델 누락 오류

- 원인: 필수 모델 env 누락 또는 파일 경로 불일치
- 해결:
  1. `.env`의 3개 필수 모델 변수 재확인
  2. `models/stt`, `models/text`, `models/embed` 경로와 파일명 대조
  3. 필요 시 `--yes-pull` + repo env 설정 후 재시도

### 6-2) Windows에서 `rustc.exe ... not applicable` 오류

아래 순서로 rustup toolchain을 복구합니다.

```powershell
rustup show
rustup toolchain uninstall stable-x86_64-pc-windows-msvc
rustup toolchain install stable-x86_64-pc-windows-msvc --profile default
rustup default stable-x86_64-pc-windows-msvc
rustup component add rustc cargo clippy rustfmt --toolchain stable-x86_64-pc-windows-msvc
rustup update
cargo build
```

### 6-3) 프론트는 켜졌는데 API 호출 실패

- `RECORDROUTE_API_PORT`와 프론트의 API base 해석(`VITE_API_BASE_URL`, `VITE_TAURI_BACKEND_URL`)이 일치하는지 확인합니다.
- 서버 로그에서 `/healthz`, `/readyz` 응답을 먼저 확인한 뒤 작업 요청을 재시도합니다.

---

## 7) 운영/기술 문서

- 사용자 상세 조작 매뉴얼: `docs/user-operation-manual.md`
- 기술 아키텍처/운영 통합 문서: `docs/technical-architecture-and-operations.md`
- 아키텍처 기준선: `docs/architecture.md`
- Rust 전환 계획: `docs/rust-cpp-backend-rewrite-plan.md`
- 배포/자산 정책: `docs/deployment-asset-policy.md`
- Tauri 패키징/보안 기준: `docs/tauri-packaging-security-baseline.md`
- Tauri 설치/배포 통합 플로우: `docs/tauri-install-deploy-unified-flow.md`
- 운영 점검 정례화: `docs/operations/weekly-drill/README.md`

---

## 8) 유지보수 메모

- 이 README는 사용자 관점 문서이며, 상세 작업 규칙의 원본은 `AGENTS.md`입니다.
- 문서 정책상 `README.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`는 함께 동기화합니다.
