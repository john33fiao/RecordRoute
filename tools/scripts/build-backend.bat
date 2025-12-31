@echo off
REM RecordRoute Backend Build Script for Windows
REM Phase 3: Build Python backend with PyInstaller

setlocal enabledelayedexpansion

echo ======================================
echo RecordRoute Backend Build Script
echo ======================================
echo.

REM Check if virtual environment is activated
if "%VIRTUAL_ENV%"=="" (
    echo Warning: Virtual environment not detected
    echo Attempting to activate virtual environment...

    if exist "venv\Scripts\activate.bat" (
        call venv\Scripts\activate.bat
        echo [OK] Virtual environment activated
    ) else (
        echo [ERROR] Virtual environment not found
        echo Please create one with: python -m venv venv
        exit /b 1
    )
)

REM Check if PyInstaller is installed
python -c "import PyInstaller" 2>nul
if errorlevel 1 (
    echo PyInstaller not found. Installing...
    pip install pyinstaller
)

echo.
echo [Step 1/4] Cleaning previous build...
if exist build rmdir /s /q build
if exist dist rmdir /s /q dist
if exist bin\RecordRouteAPI rmdir /s /q bin\RecordRouteAPI
echo [OK] Cleaned

echo.
echo [Step 2/4] Building Python backend with PyInstaller...
pyinstaller RecordRouteAPI.spec --clean

if errorlevel 1 (
    echo [ERROR] PyInstaller build failed
    exit /b 1
)
echo [OK] PyInstaller build complete

echo.
echo [Step 3/4] Copying executable to bin directory...
if not exist bin mkdir bin
xcopy /E /I /Y dist\RecordRouteAPI bin\RecordRouteAPI
echo [OK] Copied RecordRouteAPI to bin\

echo.
echo [Step 4/5] Copying FFmpeg binary...

set FFMPEG_SRC=bin\ffmpeg\win32\ffmpeg.exe
set FFMPEG_DST=bin\ffmpeg.exe

if exist "%FFMPEG_SRC%" (
    copy /Y "%FFMPEG_SRC%" "%FFMPEG_DST%" >nul
    echo [OK] Copied FFmpeg to bin\
) else (
    echo [WARNING] FFmpeg binary not found at %FFMPEG_SRC%
    echo   The build will continue, but you'll need to install FFmpeg separately.
    echo   See bin\ffmpeg\README.md for instructions.
)

echo.
echo [Step 5/5] Build summary...
echo ------------------------------
echo Output directory: dist\RecordRouteAPI
echo Installed to: bin\RecordRouteAPI

if exist "bin\RecordRouteAPI\RecordRouteAPI.exe" (
    echo [OK] Build successful!
) else (
    echo [ERROR] Executable not found
    exit /b 1
)

echo.
echo ======================================
echo Backend build complete!
echo ======================================
echo.
echo Next steps:
echo   1. Test the backend: bin\RecordRouteAPI\RecordRouteAPI.exe
echo   2. Build Electron app: npm run build
echo.

endlocal
