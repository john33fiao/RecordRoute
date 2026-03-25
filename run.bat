@echo off
setlocal

cd /d "%~dp0rust"
if errorlevel 1 exit /b %errorlevel%

cargo run --release -- server
exit /b %errorlevel%
