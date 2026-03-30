@echo off
setlocal EnableExtensions

for %%I in ("%~dp0.") do set "repo_root=%%~fI"
set "launcher_path=%repo_root%\package\RecordRoute.exe"
set "frontend_dir=%repo_root%\frontend"
set "should_start_frontend_dev=0"

if not exist "%launcher_path%" (
  echo RecordRoute package launcher is missing. Run setup.bat first.
  exit /b 1
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%repo_root%\scripts\check_package_stale.ps1" -RepoRoot "%repo_root%"
if errorlevel 1 exit /b 1

if exist "%frontend_dir%\package.json" (
  where npm >nul 2>nul
  if errorlevel 1 (
    >&2 echo npm is unavailable. Starting backend only.
  ) else (
    set "should_start_frontend_dev=1"
  )
)

if "%should_start_frontend_dev%"=="0" (
  "%launcher_path%"
  exit /b %errorlevel%
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%repo_root%\scripts\run_with_frontend.ps1" -LauncherPath "%launcher_path%" -FrontendDir "%frontend_dir%"
exit /b %errorlevel%
