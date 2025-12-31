@echo off
REM RecordRoute Full Build Script (Windows)
REM Phase 3: Build both Python backend and Electron frontend

setlocal enabledelayedexpansion

echo ==========================================
echo RecordRoute Full Build Script
echo ==========================================
echo.

REM Parse arguments
set SKIP_BACKEND=false
set SKIP_FRONTEND=false
set TARGET=

:parse_args
if "%~1"=="" goto end_parse
if "%~1"=="--skip-backend" (
    set SKIP_BACKEND=true
    shift
    goto parse_args
)
if "%~1"=="--skip-frontend" (
    set SKIP_FRONTEND=true
    shift
    goto parse_args
)
if "%~1"=="--target" (
    set TARGET=%~2
    shift
    shift
    goto parse_args
)
echo Unknown option: %~1
echo Usage: %~nx0 [--skip-backend] [--skip-frontend] [--target win^|mac^|linux]
exit /b 1

:end_parse

REM Build backend
if "%SKIP_BACKEND%"=="false" (
    echo.
    echo ==========================================
    echo Step 1: Building Python Backend
    echo ==========================================
    echo.

    if exist "tools\scripts\build-backend.bat" (
        call tools\scripts\build-backend.bat
    ) else (
        echo Warning: tools\scripts\build-backend.bat not found, skipping backend build
    )
) else (
    echo.
    echo [Skipped] Python backend build
)

REM Install Node.js dependencies if needed
if "%SKIP_FRONTEND%"=="false" (
    echo.
    echo ==========================================
    echo Step 2: Installing Node.js Dependencies
    echo ==========================================
    echo.

    if not exist "node_modules" (
        echo Installing root dependencies...
        call npm install
        if errorlevel 1 (
            echo Error: npm install failed
            exit /b 1
        )
        echo.
        echo Installing workspace dependencies...
        call npm install --workspaces
        if errorlevel 1 (
            echo Error: npm install --workspaces failed
            exit /b 1
        )
        echo.
        echo Installing electron-builder dependencies...
        call npm run install-deps
        if errorlevel 1 (
            echo Error: electron-builder install-app-deps failed
            exit /b 1
        )
        echo [✓] Dependencies installed
    ) else (
        echo Node modules already installed
    )
)

REM Build Electron app
if "%SKIP_FRONTEND%"=="false" (
    echo.
    echo ==========================================
    echo Step 3: Building Electron Application
    echo ==========================================
    echo.

    if "%TARGET%"=="" (
        set TARGET=win
    )

    echo Building for platform: %TARGET%

    if "%TARGET%"=="win" (
        call npm run build:win
    ) else if "%TARGET%"=="windows" (
        call npm run build:win
    ) else if "%TARGET%"=="mac" (
        call npm run build:mac
    ) else if "%TARGET%"=="macos" (
        call npm run build:mac
    ) else if "%TARGET%"=="darwin" (
        call npm run build:mac
    ) else if "%TARGET%"=="linux" (
        call npm run build:linux
    ) else (
        echo Building for all platforms...
        call npm run build
    )

    if errorlevel 1 (
        echo Error: Build failed
        exit /b 1
    )

    echo [✓] Electron build complete
) else (
    echo.
    echo [Skipped] Electron application build
)

echo.
echo ==========================================
echo Build Complete!
echo ==========================================
echo.
echo Output locations:
echo   - Python backend: bin\RecordRouteAPI\
echo   - Electron app: dist\
echo.
echo To run in development mode: npm start
echo.

endlocal
