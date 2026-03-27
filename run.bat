@echo off
setlocal EnableExtensions

set "binary_path=%~dp0rust\target\release\recordroute_rust.exe"
if not exist "%binary_path%" (
  echo RecordRoute runtime binary is missing. Run setup.bat first.
  exit /b 1
)

echo RecordRoute server starting on http://127.0.0.1:38080/
echo Web UI: http://127.0.0.1:38080/

"%binary_path%" server
exit /b %errorlevel%
