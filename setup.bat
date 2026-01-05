@echo off
REM RecordRoute Setup Script for Windows
REM ======================================

setlocal enabledelayedexpansion

REM Set directory to where the script is
cd /d "%~dp0" || exit /b 1
set SCRIPT_DIR=%cd%

echo.
echo ================================
echo    RecordRoute Setup
echo ================================
echo.

REM 1. Check Python
echo Step 0: Checking Python installation...

set PY_CMD=
where python >nul 2>&1
if !errorlevel! equ 0 (
    set PY_CMD=python
) else (
    where python3 >nul 2>&1
    if !errorlevel! equ 0 (
        set PY_CMD=python3
    )
)

if "!PY_CMD!"=="" (
    echo.
    echo [오류] python 또는 python3이 설치되어 있지 않습니다.
    echo Python 3.8 이상을 설치한 후 다시 실행하세요.
    echo 다운로드: https://www.python.org/downloads/
    echo.
    pause
    exit /b 1
)

echo Python 버전:
!PY_CMD! --version
echo.

REM 2. Virtual Environment
echo Step 1: Creating Virtual Environment...
echo.

if exist "%SCRIPT_DIR%\venv" (
    echo 기존 가상환경이 발견되었습니다.
    setlocal disableDelayedExpansion
    set /p response="기존 가상환경을 삭제하고 다시 생성하시겠습니까? (y/n): "
    setlocal enabledelayedexpansion
    
    if /i "!response!"=="y" (
        echo 기존 venv를 삭제하고 있습니다...
        rmdir /s /q "%SCRIPT_DIR%\venv"
        
        echo 새로운 venv를 생성하고 있습니다...
        !PY_CMD! -m venv venv
    ) else (
        echo 기존 venv를 사용합니다.
    )
) else (
    echo 새로운 venv를 생성하고 있습니다...
    !PY_CMD! -m venv venv
)

if not exist "%SCRIPT_DIR%\venv" (
    echo.
    echo [오류] 가상환경 생성에 실패했습니다.
    echo.
    pause
    exit /b 1
)

REM Verify venv was created successfully
if not exist "%SCRIPT_DIR%\venv\Scripts\python.exe" (
    echo.
    echo [오류] 가상환경 Python 실행 파일을 찾을 수 없습니다.
    echo.
    pause
    exit /b 1
)

set VENV_PYTHON=%SCRIPT_DIR%\venv\Scripts\python.exe

echo 가상환경이 성공적으로 생성되었습니다.
echo.

REM 3. Install Dependencies
echo Step 2: Installing Dependencies...
echo.

REM Upgrade pip first
echo pip을 업그레이드하고 있습니다...
"!VENV_PYTHON!" -m pip install --upgrade pip

if exist "%SCRIPT_DIR%\sttEngine\requirements.txt" (
    echo sttEngine 의존성을 설치하고 있습니다...
    "!VENV_PYTHON!" -m pip install -r sttEngine\requirements.txt
    if !errorlevel! neq 0 (
        echo [경고] 일부 sttEngine 의존성 설치에 실패했습니다.
    )
) else (
    echo [경고] sttEngine\requirements.txt 파일을 찾을 수 없습니다.
)

if exist "%SCRIPT_DIR%\requirements.txt" (
    echo 루트 의존성을 설치하고 있습니다...
    "!VENV_PYTHON!" -m pip install -r requirements.txt
    if !errorlevel! neq 0 (
        echo [경고] 일부 루트 의존성 설치에 실패했습니다.
    )
)

echo.

REM 3.5 PyTorch Verification
echo Step 2.5: Verifying PyTorch...

"!VENV_PYTHON!" -c "import torch; print('PyTorch: ' + torch.__version__); print('CUDA available: ' + str(torch.cuda.is_available()))" >nul 2>&1

if !errorlevel! equ 0 (
    echo PyTorch가 설치되어 있습니다.
) else (
    echo PyTorch가 설치되지 않았거나 CUDA 지원이 없습니다.
    echo CUDA 지원을 포함한 PyTorch를 설치하고 있습니다...
    "!VENV_PYTHON!" -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu124 torch torchvision torchaudio
    
    if !errorlevel! neq 0 (
        echo [경고] CUDA 빌드 설치에 실패했습니다. 기본 빌드를 설치합니다...
        "!VENV_PYTHON!" -m pip install --upgrade torch torchvision torchaudio
    )
)

"!VENV_PYTHON!" -c "import torch; print('PyTorch: ' + torch.__version__); print('CUDA available: ' + str(torch.cuda.is_available()))" 2>nul || echo [경고] PyTorch 확인에 실패했거나 설치되지 않았습니다.
echo.

REM 4. Ollama Check
echo Step 3: Checking Ollama...

where ollama >nul 2>&1
if !errorlevel! equ 0 (
    echo Ollama가 설치되어 있습니다.
    echo 모델을 확인하고 있습니다...
    
    ollama list 2>nul | findstr /i "gemma" >nul
    if !errorlevel! equ 0 (
        echo Gemma 모델이 이미 설치되어 있습니다.
    ) else (
        echo Gemma 모델을 설치하고 있습니다...
        echo (이 과정은 시간이 걸릴 수 있습니다)
        ollama pull gemma3:4b-it-qat
    )
) else (
    echo Ollama가 설치되어 있지 않습니다.
    echo 다음 링크에서 다운로드하세요: https://ollama.ai
    echo 또는 Scoop, Chocolatey 등의 패키지 관리자를 사용하세요.
    echo.
    echo 설치 후 다음 명령어를 실행하세요:
    echo   ollama pull gemma3:4b-it-qat
)
echo.

REM 5. FFmpeg Check
echo Step 4: Checking FFmpeg...

where ffmpeg >nul 2>&1
if !errorlevel! equ 0 (
    echo FFmpeg가 설치되어 있습니다.
    ffmpeg -version 2>nul | find "version" | findstr /r /c:".*"
) else (
    echo FFmpeg가 설치되어 있지 않습니다.
    echo 다음 링크에서 다운로드하세요: https://ffmpeg.org/download.html
    echo.
    echo 또는 패키지 관리자를 사용하세요:
    echo   Scoop: scoop install ffmpeg
    echo   Chocolatey: choco install ffmpeg
    echo   Windows Package Manager: winget install ffmpeg
)

echo.
echo ================================
echo     Setup Complete!
echo ================================
echo run.bat를 실행하여 서버를 시작할 수 있습니다.
echo.

endlocal
pause
