@echo off
setlocal EnableExtensions EnableDelayedExpansion

set "ROOT_DIR=%~dp0.."
pushd "%ROOT_DIR%" >nul

set "CHECK_ONLY=0"
set "MODEL_POLICY=prompt"

:parse_args
if "%~1"=="" goto :args_done
if /I "%~1"=="--check" (
    set "CHECK_ONLY=1"
    shift
    goto :parse_args
)
if /I "%~1"=="--yes-pull" (
    set "MODEL_POLICY=pull"
    shift
    goto :parse_args
)
if /I "%~1"=="--no-pull" (
    set "MODEL_POLICY=cancel"
    shift
    goto :parse_args
)
if /I "%~1"=="-h" goto :help
if /I "%~1"=="--help" goto :help

echo [ERROR] Unknown option: %~1
goto :help

:help
echo Usage: scripts\install_windows.bat [options]
echo.
echo Options:
echo   --check      Validate env/model/prerequisites only (no install/build)
echo   --yes-pull   Auto-pull missing models
echo   --no-pull    Fail immediately when model is missing
echo   -h, --help   Show help
popd >nul
exit /b 1

:args_done
echo [RecordRoute] Windows install started.

call :require_command npm
if errorlevel 1 goto :fail
call :require_command cargo
if errorlevel 1 goto :fail

call :require_env RECORDROUTE_DEFAULT_STT_MODEL "STT default model"
if errorlevel 1 goto :fail
call :require_env RECORDROUTE_DEFAULT_SUMMARIZE_MODEL "Summarize default model"
if errorlevel 1 goto :fail
call :require_env RECORDROUTE_DEFAULT_EMBED_MODEL "Embed default model"
if errorlevel 1 goto :fail

call :ensure_model RECORDROUTE_DEFAULT_STT_MODEL "models\stt" RECORDROUTE_STT_MODEL_REPO
if errorlevel 1 goto :fail
call :ensure_model RECORDROUTE_DEFAULT_SUMMARIZE_MODEL "models\text" RECORDROUTE_SUMMARIZE_MODEL_REPO
if errorlevel 1 goto :fail
call :ensure_model RECORDROUTE_DEFAULT_EMBED_MODEL "models\embed" RECORDROUTE_EMBED_MODEL_REPO
if errorlevel 1 goto :fail

if "%CHECK_ONLY%"=="1" (
  echo [RecordRoute] Check mode passed ^(no install/build executed^).
  popd >nul
  exit /b 0
)

echo [1/3] Installing frontend dependencies...
call npm --prefix frontend install
if errorlevel 1 goto :fail

echo [2/3] Building frontend...
call npm --prefix frontend run build
if errorlevel 1 goto :fail

echo [3/3] Building Rust orchestrator...
call cargo build --release
if errorlevel 1 goto :fail

echo [RecordRoute] Install completed successfully.
popd >nul
exit /b 0

:require_command
set "CMD=%~1"
where %CMD% >nul 2>nul
if errorlevel 1 (
    echo [ERROR] Required command not found: %CMD%
    exit /b 1
)
exit /b 0

:require_env
set "VAR_NAME=%~1"
set "LABEL=%~2"
call set "VAR_VALUE=%%%VAR_NAME%%%"
if not defined VAR_VALUE (
    echo [ERROR] %LABEL% env ^(%VAR_NAME%^) is not set.
    exit /b 1
)
exit /b 0

:ensure_model
set "MODEL_VAR=%~1"
set "MODEL_DIR=%~2"
set "MODEL_REPO_VAR=%~3"
call set "MODEL_VALUE=%%%MODEL_VAR%%%"
set "MODEL_PATH=!MODEL_VALUE!"

if not exist "!MODEL_PATH!" (
    set "MODEL_PATH=%ROOT_DIR%\%MODEL_DIR%\!MODEL_VALUE!"
)

if exist "!MODEL_PATH!" (
    echo [OK] !MODEL_VAR! -^> !MODEL_PATH!
    exit /b 0
)

echo [WARN] Model not found for !MODEL_VAR! ^(!MODEL_VALUE!^).
echo        Expected path: !MODEL_PATH!

set "ACTION=%MODEL_POLICY%"
if /I "!ACTION!"=="prompt" (
    set /p CHOICE="Choose action: [C]ancel install / [P]ull model: "
    if /I "!CHOICE!"=="P" (
        set "ACTION=pull"
    ) else (
        set "ACTION=cancel"
    )
)

if /I "!ACTION!"=="pull" (
    call :pull_model "%MODEL_VAR%" "%MODEL_DIR%" "%MODEL_REPO_VAR%"
    if errorlevel 1 exit /b 1

    call set "MODEL_VALUE=%%%MODEL_VAR%%%"
    set "MODEL_PATH=!MODEL_VALUE!"
    if not exist "!MODEL_PATH!" set "MODEL_PATH=%ROOT_DIR%\%MODEL_DIR%\!MODEL_VALUE!"
    if exist "!MODEL_PATH!" (
        echo [OK] Pulled !MODEL_VAR! -^> !MODEL_PATH!
        exit /b 0
    )

    echo [ERROR] Model still missing after pull attempt: !MODEL_PATH!
    exit /b 1
)

echo [ERROR] Install canceled because model file is missing.
exit /b 1

:pull_model
set "MODEL_VAR=%~1"
set "MODEL_DIR=%~2"
set "MODEL_REPO_VAR=%~3"
call set "MODEL_FILE=%%%MODEL_VAR%%%"
call set "MODEL_REPO=%%%MODEL_REPO_VAR%%%"

if not defined MODEL_REPO (
    echo [ERROR] Pull requested but %MODEL_REPO_VAR% is not set.
    echo        Set repo id ^(e.g. org/repo^) and rerun.
    exit /b 1
)

where huggingface-cli >nul 2>nul
if errorlevel 1 (
    echo [ERROR] huggingface-cli is required to pull models.
    echo        Install: pip install -U huggingface_hub
    exit /b 1
)

echo [INFO] Pulling %MODEL_FILE% from %MODEL_REPO% ...
call huggingface-cli download "%MODEL_REPO%" "%MODEL_FILE%" --local-dir "%ROOT_DIR%\%MODEL_DIR%" --local-dir-use-symlinks False
if errorlevel 1 (
    echo [ERROR] huggingface-cli download failed.
    exit /b 1
)
exit /b 0

:fail
echo [RecordRoute] Install failed.
popd >nul
exit /b 1
