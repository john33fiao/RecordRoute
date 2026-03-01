# RecordRoute

RecordRoute는 **로컬 AI 모델 기반 음성/문서 처리 GUI**입니다.  
이 README는 처음 사용하는 사용자가 `git clone`부터 실제 GUI 사용까지 따라갈 수 있도록 작성된 **사용자용 가이드**입니다.

> 현재 상태: 저장소는 Rust 전환 진행 중이며, 기술 기준/아키텍처 상세는 `docs/technical-architecture-and-operations.md`를 참고하세요.

---

## 1) 빠른 개요

- GUI 실행 방식
  - **권장(개발/운영 점검 공통):** 웹 GUI(`frontend`) + Rust API(`recordroute-orchestrator`) 동시 실행
  - **데스크톱 앱 실험 경로:** `src-tauri/` (전환 게이트 미완료 상태)
- 기본 API 포트: `18000`
- 필수 모델 환경변수 3개
  - `RECORDROUTE_DEFAULT_STT_MODEL`
  - `RECORDROUTE_DEFAULT_SUMMARIZE_MODEL`
  - `RECORDROUTE_DEFAULT_EMBED_MODEL`

---

## 2) 설치 전 준비

### 공통 필수 도구

- Git
- Node.js + npm
- Rust (`cargo`)

### 모델 파일 준비

RecordRoute는 로컬 모델 파일이 필요합니다.

- STT: `models/stt/`
- 요약(LLM): `models/text/`
- 임베딩: `models/embed/`

모델 준비 상세 절차는 `docs/user-operation-manual.md`의 “사전 준비” 섹션을 따르세요.

---

## 3) git clone부터 실행까지 (Linux/macOS)

### 3-1. 저장소 클론

```bash
git clone <YOUR_REPO_URL>
cd RecordRoute
```

### 3-2. 기본 모델 환경변수 설정

아래 예시는 파일명을 직접 지정하는 방식입니다.

```bash
export RECORDROUTE_DEFAULT_STT_MODEL="whisper-base.bin"
export RECORDROUTE_DEFAULT_SUMMARIZE_MODEL="summarize-model.gguf"
export RECORDROUTE_DEFAULT_EMBED_MODEL="embed-model.gguf"
```

> 절대경로를 넣어도 됩니다. 파일이 없으면 설치 스크립트가 실패합니다.

### 3-3. 사전 점검(설치 없이 검증)

```bash
scripts/install_unix.sh --check
```

검증 항목:
- `npm`, `cargo` 존재 여부
- 필수 환경변수 3개
- 모델 파일 존재 여부

### 3-4. 설치/빌드

```bash
scripts/install_unix.sh
```

실행 내용:
1. 프론트 의존성 설치 (`npm --prefix frontend install`)
2. 프론트 빌드 (`npm --prefix frontend run build`)
3. Rust release 빌드 (`cargo build --release`)

### 3-5. GUI + API 동시 실행

개발 모드:

```bash
scripts/run_unix.sh
```

릴리스 바이너리 모드:

```bash
scripts/run_unix.sh --release
```

실행 후:
- 프론트 dev 서버: 일반적으로 `http://localhost:5173`
- Rust API: `http://127.0.0.1:18000`

브라우저에서 프론트 주소를 열어 GUI를 사용합니다.

---

## 4) git clone부터 실행까지 (Windows)

### 4-1. 저장소 클론

```powershell
git clone <YOUR_REPO_URL>
cd RecordRoute
```

### 4-2. 기본 모델 환경변수 설정

```powershell
setx RECORDROUTE_DEFAULT_STT_MODEL "whisper-base.bin"
setx RECORDROUTE_DEFAULT_SUMMARIZE_MODEL "summarize-model.gguf"
setx RECORDROUTE_DEFAULT_EMBED_MODEL "embed-model.gguf"
```

> `setx` 적용 후 새 터미널을 열어야 반영됩니다.

### 4-3. 사전 점검(설치 없이 검증)

```powershell
scripts\install_windows.bat --check
```

### 4-4. 설치/빌드

```powershell
scripts\install_windows.bat
```

### 4-5. GUI + API 동시 실행

개발 모드:

```powershell
scripts\run_windows.bat
```

릴리스 바이너리 모드:

```powershell
scripts\run_windows.bat --release
```

실행 시 API/프론트가 별도 창으로 시작됩니다.

---

## 5) 실제 GUI 사용 절차 (사용자 매뉴얼 요약)

아래는 가장 일반적인 사용자 흐름입니다.

1. GUI 접속 후 **UploadSection**에서 파일 업로드
2. 작업 생성 후 Queue 상태(`queued → running → completed|failed|canceled|rejected`) 확인
3. History/Search/Similarity Graph에서 결과 검토
4. 필요 시 Settings에서 모델/동작 설정 조정
5. 실패 작업은 재시도 또는 파라미터 수정 후 재실행

상세 조작(화면 단위, 예외 처리, 운영 체크포인트)은 **`docs/user-operation-manual.md`**를 기준으로 사용하세요.

---

## 6) 상태 확인/헬스체크

API 서버 상태 확인:

```bash
curl http://127.0.0.1:18000/healthz
curl http://127.0.0.1:18000/readyz
curl http://127.0.0.1:18000/metrics
```

---

## 7) 문제 해결

### 모델 파일 관련 오류

- 환경변수 값(파일명/경로)과 실제 파일 위치가 일치하는지 확인
- 기본 디렉터리 규칙:
  - `models/stt/`
  - `models/text/`
  - `models/embed/`

### Windows에서 `cargo build` toolchain 오류

`rustc.exe ... is not applicable ...` 오류 시:

```powershell
rustup show
rustup toolchain uninstall stable-x86_64-pc-windows-msvc
rustup toolchain install stable-x86_64-pc-windows-msvc --profile default
rustup default stable-x86_64-pc-windows-msvc
rustup component add rustc cargo clippy rustfmt --toolchain stable-x86_64-pc-windows-msvc
rustup update
cargo build
```

---

## 8) 문서 맵

- 사용자 GUI 조작 상세: `docs/user-operation-manual.md`
- 기술/아키텍처/운영 기준: `docs/technical-architecture-and-operations.md`
- 주간 운영 점검: `docs/operations/weekly-drill/README.md`
- 오픈API 계약: `docs/openapi.yaml`, `docs/swagger/openapi.yaml`
- 전환 작업 추적: `TODO/TODO.md`

