@echo off
setlocal

echo RecordRoute server starting on http://127.0.0.1:38080/
echo Web UI: http://127.0.0.1:38080/

cd /d "%~dp0rust"
if errorlevel 1 exit /b %errorlevel%

cargo run --release -- server
exit /b %errorlevel%
