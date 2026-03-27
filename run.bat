@echo off
setlocal EnableExtensions

set "launcher_path=%~dp0package\RecordRoute.exe"
if not exist "%launcher_path%" (
  echo RecordRoute package launcher is missing. Run setup.bat first.
  exit /b 1
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\check_package_stale.ps1" -RepoRoot "%~dp0"
if errorlevel 1 exit /b 1

"%launcher_path%"
exit /b %errorlevel%
