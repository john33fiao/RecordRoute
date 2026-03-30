@echo off
setlocal EnableExtensions

for %%I in ("%~dp0.") do set "repo_root=%%~fI"
set "launcher_path=%repo_root%\package\RecordRoute.exe"

if not exist "%launcher_path%" (
  echo RecordRoute package launcher is missing. Run setup.bat first.
  exit /b 1
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%repo_root%\scripts\check_package_stale.ps1" -RepoRoot "%repo_root%"
if errorlevel 1 exit /b 1

"%launcher_path%"
exit /b %errorlevel%
