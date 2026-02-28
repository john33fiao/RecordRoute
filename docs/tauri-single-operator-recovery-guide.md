# Tauri 단일 실행자 실패 복구 가이드 (WBS 9.5)

이 문서는 단일 실행자(1인) 기준으로 설치/실행 실패를 빠르게 복구하기 위한 표준 절차를 정의합니다.

## 1) 설치 게이트 실패 복구 (`scripts/install_*.sh|bat --check`)

### 1-1. Unix(macOS/Linux)

```bash
scripts/install_unix.sh --check
```

실패 시 순서:
1. 누락된 필수 명령(`npm`, `cargo`) 설치
2. 기본 모델 env 재설정
   - `RECORDROUTE_DEFAULT_STT_MODEL`
   - `RECORDROUTE_DEFAULT_SUMMARIZE_MODEL`
   - `RECORDROUTE_DEFAULT_EMBED_MODEL`
3. 모델 파일 경로 확인(상대 경로는 `<repo>/models/...` 기준)
4. 모델이 없으면 `--yes-pull`로 재시도

```bash
scripts/install_unix.sh --yes-pull
```

### 1-2. Windows

```bat
scripts\install_windows.bat --check
```

실패 시 순서:
1. 누락된 필수 명령(`npm`, `cargo`) 설치
2. 기본 모델 env 재설정
   - `RECORDROUTE_DEFAULT_STT_MODEL`
   - `RECORDROUTE_DEFAULT_SUMMARIZE_MODEL`
   - `RECORDROUTE_DEFAULT_EMBED_MODEL`
3. 모델 파일 경로 확인(상대 경로는 `<repo>\models\...` 기준)
4. 모델이 없으면 `--yes-pull`로 재시도

```bat
scripts\install_windows.bat --yes-pull
```

## 2) 실행 실패 복구 (`scripts/run_*.sh|bat`)

### 2-1. Unix(macOS/Linux)

기본 실행:

```bash
scripts/run_unix.sh
```

복구 순서:
1. 실행 중 프로세스 정리 후 재실행
2. release 바이너리 누락 시 `scripts/install_unix.sh` 또는 `cargo build --release` 후 `--release`로 재실행

```bash
scripts/run_unix.sh --release
```

### 2-2. Windows

기본 실행:

```bat
scripts\run_windows.bat
```

복구 순서:
1. 기존 Rust API/프론트 창 종료
2. release 바이너리 누락 시 `scripts\install_windows.bat` 또는 `cargo build --release` 수행
3. `--release` 모드로 재실행

```bat
scripts\run_windows.bat --release
```

## 3) 로그 조회/수집 위치

- Tauri lifecycle smoke 로그: `artifacts/tauri-lifecycle/<ts-os-pid>/`
  - `orchestrator.log`
  - `swagger.log`
  - `orchestrator-port-conflict.log`
  - `summary.json`
- 계약 점검 리포트: `artifacts/contracts/<run-id>/contract-drift-report.json`

CI 실패 분석 시에는 위 경로의 산출물을 우선 확인하고, 재실행 전 env/포트 점유/모델 파일 상태를 점검합니다.
