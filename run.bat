@echo off
REM Windows용 RecordRoute 웹서버 실행 스크립트

REM 지연 확장을 활성화하여 특수문자가 포함된 환경변수도 안전하게 처리
setlocal EnableDelayedExpansion

REM 이 스크립트가 있는 디렉토리 기준으로 경로 설정
set "SCRIPT_DIR=%~dp0"
REM 경로 끝의 백슬래시 제거
if "%SCRIPT_DIR:~-1%"=="\" set "SCRIPT_DIR=%SCRIPT_DIR:~0,-1%"

REM .env 파일이 있으면 환경변수 로드
if exist "%SCRIPT_DIR%\.env" (
    echo ".env 파일에서 환경변수를 로드합니다."
    for /f "usebackq tokens=1* delims== eol=#" %%a in ("%SCRIPT_DIR%\.env") do (
        if not "%%b"=="" (
            set "%%a=%%b"
        )
    )
    if defined PYANNOTE_TOKEN (
        echo [DEBUG] PYANNOTE_TOKEN이 로드되었습니다.
    ) else (
        echo [DEBUG] PYANNOTE_TOKEN이 설정되지 않았습니다.
    )
)

REM 가상환경의 Python 실행 파일 경로
set "VENV_PYTHON=%SCRIPT_DIR%\venv\Scripts\python.exe"

REM 웹서버 스크립트 경로
set "WEB_SERVER=%SCRIPT_DIR%\sttEngine\server.py"

REM 가상환경 존재 확인
if not exist "%VENV_PYTHON%" (
    echo "오류: 가상환경(venv)을 찾을 수 없습니다."
    echo "경로: %VENV_PYTHON%"
    echo "먼저 setup.bat 스크립트를 실행하여 가상환경을 설정하세요."
    echo.
    echo "계속하려면 아무 키나 누르세요..."
    pause
    exit /b 1
)

REM 웹서버 스크립트 존재 확인
if not exist "%WEB_SERVER%" (
    echo "오류: 웹서버 스크립트(server.py)를 찾을 수 없습니다."
    echo "경로: %WEB_SERVER%"
    echo.
    echo 계속하려면 아무 키나 누르세요...
    pause
    exit /b 1
)

REM 의존성 확인 및 설치 (requirements.txt가 변경되었거나 새 패키지가 필요한 경우)
set "REQUIREMENTS_FILE=%SCRIPT_DIR%\sttEngine\requirements.txt"
set "REQUIREMENTS_STATE_FILE=%SCRIPT_DIR%\venv\.requirements_hash"

if exist "%REQUIREMENTS_FILE%" (
    echo "필요한 파이썬 패키지를 확인합니다..."
    set "REQ_HASH="
    for /f "delims=" %%H in ('call "%VENV_PYTHON%" -c "import hashlib, pathlib, sys; print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())" "%REQUIREMENTS_FILE%"') do set "REQ_HASH=%%H"

    set "INSTALLED_HASH="
    if exist "%REQUIREMENTS_STATE_FILE%" (
        set /p INSTALLED_HASH=<"%REQUIREMENTS_STATE_FILE%"
    )

    if not "!REQ_HASH!"=="!INSTALLED_HASH!" (
        echo "의존성을 설치/업데이트합니다..."
        "%VENV_PYTHON%" -m pip install -r "%REQUIREMENTS_FILE%"
        if errorlevel 1 (
            echo "경고: 의존성 설치에 실패했습니다. 설치 로그를 확인하세요."
        ) else (
            >"%REQUIREMENTS_STATE_FILE%" echo !REQ_HASH!
            echo "필요한 패키지가 준비되었습니다."
        )
    ) else (
        echo "필요한 파이썬 패키지가 이미 설치되어 있습니다."
    )
) else (
    echo "경고: requirements.txt 파일을 찾을 수 없습니다."
)

REM Ensure CUDA-enabled PyTorch is installed (avoid CPU-only wheels)
set "TORCH_VER_FILE=%SCRIPT_DIR%\venv\Lib\site-packages\torch\version.py"
if not exist "%TORCH_VER_FILE%" (
    echo "PyTorch가 설치되지 않았습니다. CUDA 빌드를 설치합니다 (cu124)..."
    "%VENV_PYTHON%" -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu124 torch
) else (
    findstr /C:"+cpu'" "%TORCH_VER_FILE%" >nul 2>&1
    if not errorlevel 1 (
        echo "CPU 빌드 PyTorch 감지 → CUDA 빌드로 교체합니다 (cu124)..."
        "%VENV_PYTHON%" -m pip uninstall -y torch >nul 2>&1
        "%VENV_PYTHON%" -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu124 torch
    ) else (
        echo "PyTorch CUDA 빌드 또는 호환 빌드가 감지되었습니다."
    )
)

echo "PyTorch 상태 확인:"
"%VENV_PYTHON%" -c "import torch; print('Torch:', torch.__version__); print('CUDA runtime:', getattr(getattr(torch,'version',None),'cuda',None)); print('cuda.is_available:', bool(getattr(torch,'cuda',None) and torch.cuda.is_available()))"

REM Ollama 서버 상태 확인 및 시작
echo "Ollama 서버 상태를 확인합니다..."
curl -s http://localhost:11434/api/version >nul 2>&1
if !ERRORLEVEL! neq 0 (
    echo "Ollama 서버가 실행되지 않았습니다. 자동으로 시작합니다..."
    where ollama >nul 2>&1
    if !ERRORLEVEL! equ 0 (
        REM 새 창에서 ollama serve 실행
        start "Ollama Server" ollama serve
        echo "Ollama 서버를 시작했습니다."
        
        REM 서버 시작을 위해 잠시 대기
        echo "서버 시작을 기다리는 중..."
        timeout /t 3 /nobreak >nul
        
        REM 서버 시작 확인
        curl -s http://localhost:11434/api/version >nul 2>&1
        if !ERRORLEVEL! equ 0 (
            echo "Ollama 서버가 성공적으로 시작되었습니다."
        ) else (
            echo "경고: Ollama 서버 시작을 확인할 수 없습니다. 수동으로 'ollama serve'를 실행해주세요."
        )
    ) else (
        echo "경고: ollama 명령어를 찾을 수 없습니다. Ollama가 설치되어 있는지 확인하세요."
        echo "수동으로 'ollama serve' 명령어를 실행한 후 이 스크립트를 다시 실행하세요."
    )
) else (
    echo "Ollama 서버가 이미 실행 중입니다."
)

REM 웹서버 실행
echo "가상환경의 파이썬으로 웹서버를 실행합니다..."
echo "서버 URL: http://localhost:8080"
echo "(웹브라우저에서 http://localhost:8080 에 접속하세요)"
echo.
echo "서버를 종료하려면 Ctrl+C를 누르세요."
echo.

cd /d "%SCRIPT_DIR%"
"%VENV_PYTHON%" -m sttEngine.server
set "EXIT_CODE=%ERRORLEVEL%"

REM 서버 종료 후 일시 정지
echo.
if "%EXIT_CODE%"=="0" (
    echo "서버가 정상적으로 종료되었습니다."
) else (
    echo "서버가 오류와 함께 종료되었습니다. (오류코드: %EXIT_CODE%)"
    echo "오류 상세 내용을 확인하세요."
)
echo.
echo "계속하려면 아무 키나 누르세요..."
pause
