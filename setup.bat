@echo off
setlocal EnableExtensions

for %%I in ("%~dp0.") do set "repo_root=%%~fI"
set "rust_manifest=%repo_root%\rust\Cargo.toml"
set "frontend_dir=%repo_root%\frontend"
set "package_dir=%repo_root%\package"
set "staging_dir=%repo_root%\.package-staging"
set "next_dir=%repo_root%\.package-next"

call :detect_arch
if errorlevel 1 exit /b 1
set "target_dir=windows-%platform_arch%"

call :ensure_bundled_sources
if errorlevel 1 exit /b 1

call :require_file "%frontend_dir%\package.json" "frontend package manifest"
if errorlevel 1 exit /b 1

pushd "%frontend_dir%"
call npm install --no-package-lock
if errorlevel 1 (
  popd
  exit /b 1
)
call npm run build
if errorlevel 1 (
  popd
  exit /b 1
)
popd

call :require_file "%frontend_dir%\build\index.html" "frontend build index"
if errorlevel 1 exit /b 1

call "%repo_root%\scripts\build_ffmpeg.bat"
if errorlevel 1 exit /b 1
call "%repo_root%\scripts\build_whisper.bat"
if errorlevel 1 exit /b 1
call "%repo_root%\scripts\build_llama.bat"
if errorlevel 1 exit /b 1

call :require_file "%repo_root%\.build\ffmpeg\%target_dir%\install\bin\ffmpeg.exe" "ffmpeg binary"
if errorlevel 1 exit /b 1
call :require_file "%repo_root%\.build\ffmpeg\%target_dir%\install\bin\ffprobe.exe" "ffprobe binary"
if errorlevel 1 exit /b 1
call :require_file "%repo_root%\.build\whisper\%target_dir%\bin\whisper-cli.exe" "whisper-cli binary"
if errorlevel 1 exit /b 1
call :require_file "%repo_root%\.build\llama\%target_dir%\bin\llama-cli.exe" "llama-cli binary"
if errorlevel 1 exit /b 1
call :require_file "%repo_root%\.build\llama\%target_dir%\bin\llama-embedding.exe" "llama-embedding binary"
if errorlevel 1 exit /b 1

cargo build --manifest-path "%rust_manifest%" --release --bin recordroute --bin recordroute_server --bin recordroute_rust
if errorlevel 1 exit /b 1

if exist "%staging_dir%" rmdir /s /q "%staging_dir%"
if exist "%next_dir%" rmdir /s /q "%next_dir%"
mkdir "%staging_dir%"

copy /Y "%repo_root%\rust\target\release\recordroute.exe" "%staging_dir%\RecordRoute.exe" >nul
copy /Y "%repo_root%\rust\target\release\recordroute_server.exe" "%staging_dir%\RecordRouteServer.exe" >nul
if exist "%repo_root%\.build" xcopy "%repo_root%\.build" "%staging_dir%\.build" /E /I /Y >nul
if exist "%frontend_dir%\build" xcopy "%frontend_dir%\build" "%staging_dir%\frontend\build" /E /I /Y >nul
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

if exist "%repo_root%\.env" (
  copy /Y "%repo_root%\.env" "%next_dir%\.env" >nul
) else if exist "%package_dir%\.env" (
  copy /Y "%package_dir%\.env" "%next_dir%\.env" >nul
) else if exist "%repo_root%\.env.example" (
  copy /Y "%repo_root%\.env.example" "%next_dir%\.env" >nul
)

if not exist "%next_dir%\db" mkdir "%next_dir%\db"
if not exist "%next_dir%\logs" mkdir "%next_dir%\logs"

if exist "%package_dir%" rmdir /s /q "%package_dir%"
move "%next_dir%" "%package_dir%" >nul

set "RECORDROUTE_RUNTIME_ROOT=%package_dir%"
echo Starting runtime model preparation...
"%repo_root%\rust\target\release\recordroute_rust.exe" prepare-models
if errorlevel 1 exit /b 1
call :touch_packaged_outputs "%package_dir%" "%target_dir%"
if errorlevel 1 exit /b 1
if exist "%staging_dir%" rmdir /s /q "%staging_dir%"
echo Runtime model preparation finished.

echo Package ready: %package_dir%\RecordRoute.exe
exit /b 0

:detect_arch
set "machine=%PROCESSOR_ARCHITECTURE%"
if defined PROCESSOR_ARCHITEW6432 set "machine=%PROCESSOR_ARCHITEW6432%"
set "platform_arch="
if /I "%machine%"=="AMD64" set "platform_arch=x86_64"
if /I "%machine%"=="ARM64" set "platform_arch=aarch64"
if not defined platform_arch set "platform_arch=%machine%"
exit /b 0

:ensure_bundled_sources
if exist "%repo_root%\modules\ffmpeg" if exist "%repo_root%\modules\whisper.cpp" if exist "%repo_root%\modules\llama.cpp" exit /b 0
git -C "%repo_root%" submodule update --init --recursive
exit /b %errorlevel%

:require_file
set "required_path=%~1"
set "required_label=%~2"
if exist "%required_path%" exit /b 0
>&2 echo missing %required_label%: %required_path%
exit /b 1

:touch_packaged_outputs
set "touch_root=%~1"
set "touch_target=%~2"
set "RECORDROUTE_TOUCH_ROOT=%touch_root%"
set "RECORDROUTE_TOUCH_TARGET=%touch_target%"
powershell -NoProfile -ExecutionPolicy Bypass -Command "$root = [System.Environment]::GetEnvironmentVariable('RECORDROUTE_TOUCH_ROOT'); $target = [System.Environment]::GetEnvironmentVariable('RECORDROUTE_TOUCH_TARGET'); if (-not $root -or -not $target) { exit 1 }; $paths = @([System.IO.Path]::Combine($root, 'RecordRoute.exe'), [System.IO.Path]::Combine($root, 'RecordRouteServer.exe'), [System.IO.Path]::Combine($root, '.build', 'ffmpeg', $target, 'install', 'bin', 'ffmpeg.exe'), [System.IO.Path]::Combine($root, '.build', 'ffmpeg', $target, 'install', 'bin', 'ffprobe.exe'), [System.IO.Path]::Combine($root, '.build', 'whisper', $target, 'bin', 'whisper-cli.exe'), [System.IO.Path]::Combine($root, '.build', 'llama', $target, 'bin', 'llama-cli.exe'), [System.IO.Path]::Combine($root, '.build', 'llama', $target, 'bin', 'llama-embedding.exe')); foreach ($path in $paths) { if (Test-Path -LiteralPath $path -PathType Leaf) { (Get-Item -LiteralPath $path).LastWriteTime = Get-Date } }; $frontendRoot = [System.IO.Path]::Combine($root, 'frontend', 'build'); if (Test-Path -LiteralPath $frontendRoot -PathType Container) { Get-ChildItem -LiteralPath $frontendRoot -File -Recurse | ForEach-Object { $_.LastWriteTime = Get-Date } }" >nul
set "RECORDROUTE_TOUCH_ROOT="
set "RECORDROUTE_TOUCH_TARGET="
if errorlevel 1 exit /b 1
exit /b 0
