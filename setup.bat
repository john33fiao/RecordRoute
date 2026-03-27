@echo off
setlocal EnableExtensions

for %%I in ("%~dp0.") do set "repo_root=%%~fI"
set "rust_manifest=%repo_root%\rust\Cargo.toml"
set "frontend_dir=%repo_root%\frontend"
set "frontend_package_json=%frontend_dir%\package.json"
set "frontend_lockfile=%frontend_dir%\package-lock.json"

if exist "%frontend_package_json%" (
  where npm >nul 2>nul
  if errorlevel 1 (
    echo frontend\package.json found, but npm is not installed or not on PATH.
    exit /b 1
  )

  if exist "%frontend_lockfile%" (
    echo Installing frontend dependencies with npm ci...
    npm ci --prefix "%frontend_dir%"
  ) else (
    echo Installing frontend dependencies with npm install...
    npm install --prefix "%frontend_dir%"
  )
  if errorlevel 1 exit /b %errorlevel%
) else (
  echo No frontend\package.json found, skipping frontend dependency install.
)

echo Building ffmpeg...
call "%repo_root%\scripts\build_ffmpeg.bat"
if errorlevel 1 exit /b %errorlevel%

echo Building whisper...
call "%repo_root%\scripts\build_whisper.bat"
if errorlevel 1 exit /b %errorlevel%

echo Building llama...
call "%repo_root%\scripts\build_llama.bat"
if errorlevel 1 exit /b %errorlevel%

echo Building rust (release)...
cargo build --manifest-path "%rust_manifest%" --release
if errorlevel 1 exit /b %errorlevel%

echo Preparing llama model...
cargo run --manifest-path "%rust_manifest%" --release -- prepare-llama-model
if errorlevel 1 exit /b %errorlevel%

exit /b 0
