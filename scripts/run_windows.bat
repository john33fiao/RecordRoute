@echo off
setlocal EnableExtensions

set "ROOT_DIR=%~dp0.."
pushd "%ROOT_DIR%" >nul

echo [RecordRoute] Starting Rust API server...
start "RecordRoute Rust API" cmd /k "cd /d %ROOT_DIR% && cargo run"
if errorlevel 1 goto :fail

echo [RecordRoute] Starting frontend dev server...
start "RecordRoute Frontend" cmd /k "cd /d %ROOT_DIR%\frontend && npm run dev"
if errorlevel 1 goto :fail

echo [RecordRoute] Servers started in separate windows.
popd >nul
exit /b 0

:fail
echo [RecordRoute] Failed to start one or more services.
popd >nul
exit /b 1
