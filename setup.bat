@echo off
chcp 65001 >nul
REM RecordRoute Setup Script for Windows
REM ======================================

setlocal EnableExtensions
setlocal EnableDelayedExpansion

REM Set directory to where the script is
set "SCRIPT_DIR=%~dp0"
if "%SCRIPT_DIR:~-1%"=="\" set "SCRIPT_DIR=%SCRIPT_DIR:~0,-1%"

REM Load .env and apply provider values
if exist "%SCRIPT_DIR%\.env" (
    for /f "usebackq eol=# delims=" %%L in ("%SCRIPT_DIR%\.env") do (
        set "line=%%L"
        if /i "!line:~0,7!"=="export " set "line=!line:~7!"
        if not "!line!"=="" set "!line!"
    )
)

set "LLM_PROVIDER_VALUE=!LLM_PROVIDER!"
if "!LLM_PROVIDER_VALUE!"=="" set "LLM_PROVIDER_VALUE=ollama"
set "EMBEDDING_PROVIDER_VALUE=!EMBEDDING_PROVIDER!"
if "!EMBEDDING_PROVIDER_VALUE!"=="" set "EMBEDDING_PROVIDER_VALUE=!LLM_PROVIDER_VALUE!"
if /i "!LLM_PROVIDER_VALUE!"=="ollama" (
    set "NEED_OLLAMA=true"
) else if /i "!EMBEDDING_PROVIDER_VALUE!"=="ollama" (
    set "NEED_OLLAMA=true"
) else (
    set "NEED_OLLAMA=false"
)

echo.
echo ================================
echo    RecordRoute Setup
echo ================================
echo.

REM 1. Check Python
echo Step 0: Checking Python installation...

set PY_CMD=
set PY_VER=
set CURRENT_PY_VER=

set CURRENT_PY_VER=
for /f "delims=" %%V in ('python3 --version 2^>^&1') do set "CURRENT_PY_VER=%%V"
if not "!CURRENT_PY_VER!"=="" (
    echo !CURRENT_PY_VER! | findstr /r /c:"^Python [0-9]" >nul 2>&1
    if !errorlevel! equ 0 (
        set "PY_CMD=python3"
        set "PY_VER=!CURRENT_PY_VER!"
    )
)

if not defined PY_CMD (
    set CURRENT_PY_VER=
    for /f "delims=" %%V in ('python --version 2^>^&1') do set "CURRENT_PY_VER=%%V"
    if not "!CURRENT_PY_VER!"=="" (
        echo !CURRENT_PY_VER! | findstr /r /c:"^Python [0-9]" >nul 2>&1
        if !errorlevel! equ 0 (
            set "PY_CMD=python"
            set "PY_VER=!CURRENT_PY_VER!"
        )
    )
)

if not defined PY_CMD (
    set CURRENT_PY_VER=
    for /f "delims=" %%V in ('py -3 --version 2^>^&1') do set "CURRENT_PY_VER=%%V"
    if not "!CURRENT_PY_VER!"=="" (
        echo !CURRENT_PY_VER! | findstr /r /c:"^Python [0-9]" >nul 2>&1
        if !errorlevel! equ 0 (
            set "PY_CMD=py -3"
            set "PY_VER=!CURRENT_PY_VER!"
        )
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
if defined PY_VER (
    echo !PY_VER!
) else (
    !PY_CMD! --version
)
echo.

REM 2. Virtual Environment
echo Step 1: Creating Virtual Environment...
echo.

if exist "%SCRIPT_DIR%\venv" (
    echo 기존 가상환경이 발견되었습니다.
    set /p response="기존 가상환경을 삭제하고 다시 생성하시겠습니까? (y/n): "
    
    if /i "!response!"=="y" (
        echo 기존 venv를 삭제하고 있습니다...
        rmdir /s /q "%SCRIPT_DIR%\venv"

        if exist "%SCRIPT_DIR%\venv" (
        echo venv 사용 중인 Python 프로세스를 종료한 뒤 삭제를 재시도합니다...
            powershell -NoProfile -ExecutionPolicy Bypass -Command "$venv=(Resolve-Path '%SCRIPT_DIR%\venv').Path; Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($venv, [System.StringComparison]::OrdinalIgnoreCase) } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }" >nul 2>&1
            timeout /t 1 /nobreak >nul
            rmdir /s /q "%SCRIPT_DIR%\venv"
        )

        if exist "%SCRIPT_DIR%\venv" (
            echo.
            echo [오류] 기존 venv 삭제에 실패했습니다. 아래 명령을 관리자 CMD에서 실행한 뒤 다시 시도하세요.
            echo   taskkill /f /im python.exe
            echo   rmdir /s /q "%SCRIPT_DIR%\venv"
            echo.
            pause
            exit /b 1
        )
        
        echo 새로운 venv를 생성하고 있습니다...
        !PY_CMD! -m venv venv

        if !errorlevel! neq 0 (
            echo.
            echo [오류] 가상환경 생성 명령이 실패했습니다.
            echo.
            pause
            exit /b 1
        )
    ) else (
        echo 기존 venv를 사용합니다.
    )
) else (
    echo 새로운 venv를 생성하고 있습니다...
    !PY_CMD! -m venv venv

    if !errorlevel! neq 0 (
        echo.
        echo [오류] 가상환경 생성 명령이 실패했습니다.
        echo.
        pause
        exit /b 1
    )
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

if /i "!NEED_OLLAMA!"=="true" if exist "%SCRIPT_DIR%\requirements-ollama.txt" (
    echo Ollama 선택 의존성을 설치하고 있습니다...
    "!VENV_PYTHON!" -m pip install -r requirements-ollama.txt
    if !errorlevel! neq 0 (
        echo [경고] 일부 Ollama 선택 의존성 설치에 실패했습니다.
    )
)

REM 3.5 PyTorch Verification
echo Step 2.5: Verifying PyTorch...

if "%OS%"=="Windows_NT" (
    echo PyTorch는 현재 Windows 환경에서 자동 설치를 수행하지 않습니다.
    "!VENV_PYTHON!" -c "import torch; print('PyTorch: ' + torch.__version__); print('CUDA available: ' + str(torch.cuda.is_available() if hasattr(torch, 'cuda') else False))" 2>nul
    if !errorlevel! neq 0 echo [경고] PyTorch 확인에 실패했거나 설치되지 않았습니다.
) else (
    set "TORCH_CUDA_AVAILABLE="
    "!VENV_PYTHON!" -c "import torch; print(f'PyTorch: {torch.__version__}'); torch.cuda.is_available() and print('CUDA available')" >"%TEMP%\recordroute_torch_check.txt" 2>nul
    if !errorlevel! neq 0 (
        echo CUDA available not detected. Attempting to install PyTorch with CUDA support...
        "!VENV_PYTHON!" -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu118 torch torchvision torchaudio
    ) else (
        findstr /r "CUDA available" "%TEMP%\recordroute_torch_check.txt" >nul
        if !errorlevel! neq 0 (
            echo CUDA not available or PyTorch not installed with CUDA support.
            echo Attempting to install PyTorch with CUDA support...
            "!VENV_PYTHON!" -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu118 torch torchvision torchaudio
        )
    )
    del /q "%TEMP%\recordroute_torch_check.txt" 2>nul
    "!VENV_PYTHON!" -c "import torch; print('PyTorch: ' + torch.__version__); print('CUDA available: ' + str(torch.cuda.is_available() if hasattr(torch, 'cuda') else False))" 2>nul
    if !errorlevel! neq 0 echo [경고] PyTorch 확인에 실패했거나 설치되지 않았습니다.
)
echo.

REM 4. Ollama Check
echo Step 3: Checking Ollama...

if /i "!NEED_OLLAMA!"=="true" (
    where ollama >nul 2>&1
    if !errorlevel! equ 0 (
        echo Ollama가 설치되어 있습니다.
        echo 모델을 확인하고 있습니다...
        
        set OLLAMA_MODEL=gemma3:4b-it-qat
        ollama list 2>nul | more +1 | findstr /r /c:"." >nul
        if !errorlevel! equ 0 (
            echo 기존 Ollama 모델이 감지되었습니다. pull 단계를 건너뜁니다.
        ) else (
            echo 설치된 Ollama 모델이 없습니다. 기본 모델 '!OLLAMA_MODEL!'을^(를^) 설치합니다...
            echo ^(이 과정은 시간이 걸릴 수 있습니다^)
            ollama pull !OLLAMA_MODEL!
        )
    ) else (
        echo Ollama가 설치되어 있지 않습니다.
        echo 다음 링크에서 다운로드하세요: https://ollama.ai
        echo 또는 Scoop, Chocolatey 등의 패키지 관리자를 사용하세요.
        echo.
        echo 설치 후 다음 명령어를 실행하세요:
        echo   ollama pull gemma3:4b-it-qat
    )
) else (
    echo LLM/Embedding provider가 ollama가 아니므로 Ollama 점검을 건너뜁니다.
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

REM 6. Node.js & Frontend Build
echo Step 5: Building Frontend...

where node >nul 2>&1
if !errorlevel! equ 0 (
    echo Node.js가 설치되어 있습니다.
    node --version

    where npm >nul 2>&1
    if !errorlevel! equ 0 (
        echo 프론트엔드 의존성을 설치합니다...
        cd /d "%SCRIPT_DIR%\frontend"
        call npm install

        echo 프론트엔드를 빌드합니다...
        call npm run build
        if !errorlevel! equ 0 (
            echo 프론트엔드 빌드 성공.
        ) else (
            echo [경고] 프론트엔드 빌드에 실패했습니다. 수동으로 빌드하세요: cd frontend ^&^& npm run build
        )
        cd /d "%SCRIPT_DIR%"
    ) else (
        echo [경고] npm이 설치되어 있지 않습니다. Node.js 18 이상을 설치하세요.
    )
) else (
    echo [경고] Node.js가 설치되어 있지 않습니다.
    echo https://nodejs.org 에서 Node.js 18 이상을 다운로드하세요.
    echo 설치 후 다음 명령어를 실행하세요: cd frontend ^&^& npm install ^&^& npm run build
)
echo.

echo.
echo ================================
echo     Setup Complete!
echo ================================
echo run.bat를 실행하여 서버를 시작할 수 있습니다.
echo.

endlocal
