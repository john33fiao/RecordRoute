@echo off
setlocal EnableExtensions

for %%I in ("%~dp0.") do set "repo_root=%%~fI"
set "rust_manifest=%repo_root%\rust\Cargo.toml"

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
