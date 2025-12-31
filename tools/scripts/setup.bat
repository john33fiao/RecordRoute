@echo off
REM RecordRoute Initial Setup Script (Windows)
REM Run this script after cloning the repository

echo ==========================================
echo RecordRoute Setup Script
echo ==========================================
echo.

REM Check if running from project root
if not exist "electron\package.json" (
    echo Error: Please run this script from the RecordRoute project root directory
    exit /b 1
)

echo This script will:
echo   1. Install Node.js dependencies
echo   2. Install electron-builder dependencies
echo   3. Download Whisper models (optional)
echo.

REM Install Node.js dependencies
echo ==========================================
echo Step 1: Installing Node.js Dependencies
echo ==========================================
echo.

echo Installing root dependencies...
call npm install

if errorlevel 1 (
    echo Error: npm install failed
    exit /b 1
)

echo.
echo Installing electron workspace dependencies...
call npm install -w electron

if errorlevel 1 (
    echo Error: npm install -w electron failed
    exit /b 1
)

echo.
echo Installing frontend workspace dependencies...
call npm install -w frontend

if errorlevel 1 (
    echo Error: npm install -w frontend failed
    exit /b 1
)

echo.
echo Installing electron-builder dependencies...
call npm run install-deps

if errorlevel 1 (
    echo Error: electron-builder install-app-deps failed
    exit /b 1
)

echo [✓] Node.js dependencies installed

REM Ask about Whisper model
echo.
echo ==========================================
echo Step 2: Whisper Model Setup (Optional)
echo ==========================================
echo.
echo Would you like to download the Whisper model now?
echo This will download whisper.cpp and the base model (~200MB)
echo.
set /p DOWNLOAD_WHISPER="Download Whisper model? (y/n): "

if /i "%DOWNLOAD_WHISPER%"=="y" (
    if exist "tools\scripts\download-whisper-model.bat" (
        call tools\scripts\download-whisper-model.bat
    ) else (
        echo Warning: download-whisper-model.bat not found
    )
) else (
    echo Skipping Whisper model download
    echo You can download it later by running: tools\scripts\download-whisper-model.bat
)

echo.
echo ==========================================
echo Setup Complete!
echo ==========================================
echo.
echo Next steps:
echo.
echo   1. Development mode:
echo      npm start
echo.
echo   2. Build for production:
echo      tools\scripts\build-all.bat
echo.
echo   3. Download Whisper model (if not done):
echo      tools\scripts\download-whisper-model.bat
echo.
echo Happy coding!
echo.

pause
