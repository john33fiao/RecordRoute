@echo off
setlocal EnableExtensions

set "ROOT_DIR=%~dp0.."
set "RUN_MODE=dev"

:parse_args
if "%~1"=="" goto :args_done
if /I "%~1"=="--release" (
    set "RUN_MODE=release"
    shift
    goto :parse_args
)
if /I "%~1"=="-h" goto :help
if /I "%~1"=="--help" goto :help

echo [ERROR] Unknown option: %~1
goto :help

:help
echo Usage: scripts\run_windows.bat [options]
echo.
echo Options:
echo   --release    Run compiled release binary instead of cargo run
echo   -h, --help   Show help
exit /b 1

:args_done
pushd "%ROOT_DIR%" >nul

call :require_command npm
if errorlevel 1 goto :fail
if /I "%RUN_MODE%"=="dev" (
    call :require_command cargo
    if errorlevel 1 goto :fail
)

echo [RecordRoute] Starting Rust API server...
if /I "%RUN_MODE%"=="release" (
    if not exist "%ROOT_DIR%\target\release\recordroute-orchestrator.exe" (
        echo [ERROR] Release binary not found: %ROOT_DIR%\target\release\recordroute-orchestrator.exe
        echo        Run scripts\install_windows.bat first or cargo build --release.
        goto :fail
    )
    start "RecordRoute Rust API" cmd /k "cd /d %ROOT_DIR% && target\release\recordroute-orchestrator.exe"
) else (
    start "RecordRoute Rust API" cmd /k "cd /d %ROOT_DIR% && cargo run --bin recordroute-orchestrator"
)
if errorlevel 1 goto :fail

echo [RecordRoute] Starting frontend dev server...
start "RecordRoute Frontend" cmd /k "cd /d %ROOT_DIR%\frontend && npm run dev"
if errorlevel 1 goto :fail

echo [RecordRoute] Servers started in separate windows.
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

:fail
echo [RecordRoute] Failed to start one or more services.
popd >nul
exit /b 1
