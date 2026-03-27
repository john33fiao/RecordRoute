@echo off
setlocal EnableExtensions

for %%I in ("%~dp0.") do set "repo_root=%%~fI"
set "rust_manifest=%repo_root%\rust\Cargo.toml"
set "package_dir=%repo_root%\package"
set "staging_dir=%package_dir%\.staging"
set "next_dir=%package_dir%\.next"

git -C "%repo_root%" submodule update --init --recursive
if errorlevel 1 exit /b 1

call "%repo_root%\scripts\build_ffmpeg.bat"
if errorlevel 1 exit /b 1
call "%repo_root%\scripts\build_whisper.bat"
if errorlevel 1 exit /b 1
call "%repo_root%\scripts\build_llama.bat"
if errorlevel 1 exit /b 1

call :require_artifact "%repo_root%\.build\ffmpeg\windows-*\install\bin\ffmpeg.exe" "ffmpeg binary"
if errorlevel 1 exit /b 1
call :require_artifact "%repo_root%\.build\ffmpeg\windows-*\install\bin\ffprobe.exe" "ffprobe binary"
if errorlevel 1 exit /b 1
call :require_artifact "%repo_root%\.build\whisper\windows-*\bin\whisper-cli.exe" "whisper-cli binary"
if errorlevel 1 exit /b 1
call :require_artifact "%repo_root%\.build\llama\windows-*\bin\llama-cli.exe" "llama-cli binary"
if errorlevel 1 exit /b 1
call :require_artifact "%repo_root%\.build\llama\windows-*\bin\llama-embedding.exe" "llama-embedding binary"
if errorlevel 1 exit /b 1

cargo build --manifest-path "%rust_manifest%" --release --bin recordroute --bin recordroute_server --bin recordroute_rust
if errorlevel 1 exit /b 1

if exist "%staging_dir%" rmdir /s /q "%staging_dir%"
if exist "%next_dir%" rmdir /s /q "%next_dir%"
mkdir "%staging_dir%"

copy /Y "%repo_root%\rust\target\release\recordroute.exe" "%staging_dir%\RecordRoute.exe" >nul
copy /Y "%repo_root%\rust\target\release\recordroute_server.exe" "%staging_dir%\RecordRouteServer.exe" >nul
if exist "%repo_root%\.build" xcopy "%repo_root%\.build" "%staging_dir%\.build" /E /I /Y >nul
if exist "%repo_root%\models" xcopy "%repo_root%\models" "%staging_dir%\models" /E /I /Y >nul
> "%staging_dir%\.recordroute-runtime-root" echo.

mkdir "%next_dir%"
xcopy "%staging_dir%" "%next_dir%" /E /I /Y >nul

if exist "%package_dir%\db" (
  if exist "%next_dir%\db" rmdir /s /q "%next_dir%\db"
  xcopy "%package_dir%\db" "%next_dir%\db" /E /I /Y >nul
)
if exist "%package_dir%\models" (
  if exist "%next_dir%\models" rmdir /s /q "%next_dir%\models"
  xcopy "%package_dir%\models" "%next_dir%\models" /E /I /Y >nul
)
if exist "%package_dir%\logs" (
  if exist "%next_dir%\logs" rmdir /s /q "%next_dir%\logs"
  xcopy "%package_dir%\logs" "%next_dir%\logs" /E /I /Y >nul
)

if exist "%package_dir%\.env" (
  copy /Y "%package_dir%\.env" "%next_dir%\.env" >nul
) else if exist "%repo_root%\.env" (
  copy /Y "%repo_root%\.env" "%next_dir%\.env" >nul
) else if exist "%repo_root%\.env.example" (
  copy /Y "%repo_root%\.env.example" "%next_dir%\.env" >nul
)

if not exist "%next_dir%\db" mkdir "%next_dir%\db"
if not exist "%next_dir%\logs" mkdir "%next_dir%\logs"

if exist "%package_dir%" rmdir /s /q "%package_dir%"
move "%next_dir%" "%package_dir%" >nul

set "RECORDROUTE_RUNTIME_ROOT=%package_dir%"
"%repo_root%\rust\target\release\recordroute_rust.exe" prepare-models
if errorlevel 1 exit /b 1

echo Package ready: %package_dir%\RecordRoute.exe
exit /b 0

:require_artifact
set "pattern=%~1"
set "label=%~2"
set "found_path="
for /f "delims=" %%P in ('dir /b /s "%pattern%" 2^>nul') do (
  if not defined found_path set "found_path=%%~fP"
)
if defined found_path exit /b 0
>&2 echo missing %label% ^(pattern: %pattern%^)
exit /b 1
