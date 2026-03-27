@echo off
setlocal EnableExtensions

set "launcher_path=%~dp0package\RecordRoute.exe"
if not exist "%launcher_path%" (
  echo RecordRoute package launcher is missing. Run setup.bat first.
  exit /b 1
)

"%launcher_path%"
exit /b %errorlevel%
