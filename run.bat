@echo off
chcp 65001 > nul
setlocal enabledelayedexpansion

REM RecordRoute 웹서버 실행 스크립트 (Windows)

REM 스크립트 디렉토리 설정
set "SCRIPT_DIR=%~dp0"
set "SCRIPT_DIR=%SCRIPT_DIR:~0,-1%"

REM .env 파일에서 환경변수 로드
if exist "%SCRIPT_DIR%\.env" (
    echo .env 파일에서 환경변수를 로드합니다.
    for /f "usebackq tokens=*" %%a in ("%SCRIPT_DIR%\.env") do (
        set "line=%%a"
        REM 주석 라인 제외
        if not "!line:~0,1!"=="#" (
            REM 빈 라인 제외
            if not "!line!"=="" (
                set "%%a"
            )
        )
    )
    echo [DEBUG] PYANNOTE_TOKEN이 로드되었습니다.
)

REM 가상환경의 Python 실행 파일 경로
set "VENV_PYTHON=%SCRIPT_DIR%\venv\Scripts\python.exe"

REM 웹서버 스크립트 경로
set "WEB_SERVER=%SCRIPT_DIR%\sttEngine\server.py"

REM 가상환경 존재 확인
if not exist "%VENV_PYTHON%" (
    echo 오류: 가상환경^(venv^)을 찾을 수 없습니다.
    echo 먼저 setup.bat 스크립트를 실행하여 가상환경을 설정하세요.
    pause
    exit /b 1
)

REM 웹서버 스크립트 존재 확인
if not exist "%WEB_SERVER%" (
    echo 오류: 웹서버 스크립트^(server.py^)를 찾을 수 없습니다.
    pause
    exit /b 1
)

REM 의존성 확인 및 설치
set "REQUIREMENTS_FILE=%SCRIPT_DIR%\sttEngine\requirements.txt"
set "REQUIREMENTS_STATE_FILE=%SCRIPT_DIR%\venv\.requirements_hash"

if exist "%REQUIREMENTS_FILE%" (
    echo 필요한 파이썬 패키지를 확인합니다...
    
    REM 현재 requirements.txt 해시 계산
    "%VENV_PYTHON%" -c "import hashlib, pathlib, sys; print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())" "%REQUIREMENTS_FILE%" > "%TEMP%\req_hash.tmp"
    set /p REQ_HASH=<"%TEMP%\req_hash.tmp"
    del "%TEMP%\req_hash.tmp"
    
    REM 기존 해시 읽기
    set "INSTALLED_HASH="
    if exist "%REQUIREMENTS_STATE_FILE%" (
        set /p INSTALLED_HASH=<"%REQUIREMENTS_STATE_FILE%"
    )
    
    REM 해시 비교 및 설치
    if not "!REQ_HASH!"=="!INSTALLED_HASH!" (
        echo 의존성을 설치/업데이트합니다...
        "%VENV_PYTHON%" -m pip install -r "%REQUIREMENTS_FILE%"
        if !ERRORLEVEL! equ 0 (
            echo !REQ_HASH!> "%REQUIREMENTS_STATE_FILE%"
            echo 필요한 패키지가 준비되었습니다.
        ) else (
            echo 경고: 의존성 설치에 실패했습니다. 설치 로그를 확인하고 다시 시도하세요.
        )
    ) else (
        echo 필요한 파이썬 패키지가 이미 설치되어 있습니다.
    )
) else (
    echo 경고: requirements.txt 파일을 찾을 수 없습니다.
)

REM 루트 requirements.txt 확인 및 설치
set "ROOT_REQ_FILE=%SCRIPT_DIR%\requirements.txt"
if exist "%ROOT_REQ_FILE%" (
    echo 루트 requirements.txt를 확인합니다...
    "%VENV_PYTHON%" -m pip install -r "%ROOT_REQ_FILE%" > nul 2>&1
    if !ERRORLEVEL! equ 0 (
        echo 루트 의존성 확인 완료.
    ) else (
        echo 경고: 루트 의존성 설치 중 오류가 발생했습니다.
    )
)

REM PyTorch CUDA 버전 확인 및 설치
echo PyTorch 상태를 확인합니다...

REM PyTorch가 설치되어 있는지 확인
"%VENV_PYTHON%" -c "import torch" > nul 2>&1
if !ERRORLEVEL! neq 0 (
    echo PyTorch가 설치되지 않았습니다. CUDA 빌드를 설치합니다 ^(cu124^)...
    "%VENV_PYTHON%" -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu124 torch torchvision torchaudio 2>nul
    if !ERRORLEVEL! neq 0 (
        "%VENV_PYTHON%" -m pip install --upgrade torch torchvision torchaudio
    )
) else (
    REM CPU 빌드 확인
    "%VENV_PYTHON%" -c "import torch; exit(0 if '+cpu' in torch.__version__ else 1)" 2>nul
    if !ERRORLEVEL! equ 0 (
        echo CPU 빌드 PyTorch 감지 → CUDA 빌드로 교체합니다 ^(cu124^)...
        "%VENV_PYTHON%" -m pip uninstall -y torch torchvision torchaudio > nul 2>&1
        "%VENV_PYTHON%" -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu124 torch torchvision torchaudio 2>nul
        if !ERRORLEVEL! neq 0 (
            "%VENV_PYTHON%" -m pip install --upgrade torch torchvision torchaudio
        )
    ) else (
        echo PyTorch CUDA 빌드 또는 호환 빌드가 감지되었습니다.
    )
)

REM PyTorch 버전 정보 출력
echo PyTorch 상태 확인:
"%VENV_PYTHON%" -c "import torch; print('Torch:', torch.__version__); print('CUDA available:', torch.cuda.is_available())" 2>nul
if !ERRORLEVEL! neq 0 (
    echo ^(PyTorch 정보를 가져올 수 없습니다^)
)

REM Ollama 서버 상태 확인 및 시작
echo Ollama 서버 상태를 확인합니다...
curl -s http://localhost:11434/api/version > nul 2>&1
if !ERRORLEVEL! neq 0 (
    echo Ollama 서버가 실행되지 않았습니다. 자동으로 시작합니다...
    where ollama > nul 2>&1
    if !ERRORLEVEL! equ 0 (
        REM 백그라운드에서 ollama serve 실행
        start /B ollama serve > nul 2>&1
        echo Ollama 서버를 시작했습니다.
        
        REM 서버 시작을 위해 잠시 대기
        echo 서버 시작을 기다리는 중...
        timeout /t 3 /nobreak > nul
        
        REM 서버 시작 확인
        curl -s http://localhost:11434/api/version > nul 2>&1
        if !ERRORLEVEL! equ 0 (
            echo Ollama 서버가 성공적으로 시작되었습니다.
        ) else (
            echo 경고: Ollama 서버 시작을 확인할 수 없습니다. 수동으로 'ollama serve'를 실행해주세요.
        )
    ) else (
        echo 경고: ollama 명령어를 찾을 수 없습니다. Ollama가 설치되어 있는지 확인하세요.
        echo 수동으로 'ollama serve' 명령어를 실행한 후 이 스크립트를 다시 실행하세요.
    )
) else (
    echo Ollama 서버가 이미 실행 중입니다.
)

REM Cloudflare Tunnel 상태 확인 및 시작
if /i "!TUNNEL_ENABLED!"=="true" (
    echo Cloudflare Tunnel이 활성화되어 있습니다. 상태를 확인합니다...
    
    REM cloudflared 설치 확인
    where cloudflared > nul 2>&1
    if !ERRORLEVEL! neq 0 (
        echo 경고: cloudflared 명령어를 찾을 수 없습니다.
        echo Cloudflare Tunnel을 사용하려면 cloudflared를 설치해야 합니다.
        echo 설치 방법: https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/installation/
        echo.
    ) else (
        REM 터널 토큰 확인
        if "!CLOUDFLARE_TUNNEL_TOKEN!"=="" (
            echo 경고: CLOUDFLARE_TUNNEL_TOKEN이 설정되지 않았습니다.
            echo .env 파일에 CLOUDFLARE_TUNNEL_TOKEN을 설정해주세요.
            echo.
        ) else (
            REM 이미 실행 중인 cloudflared 프로세스 확인
            tasklist /FI "IMAGENAME eq cloudflared.exe" 2>nul | find /I "cloudflared.exe" > nul
            if !ERRORLEVEL! equ 0 (
                echo Cloudflare Tunnel이 이미 실행 중입니다.
            ) else (
                echo Cloudflare Tunnel을 시작합니다...
                
                REM .cloudflared 디렉토리 생성
                if not exist "%SCRIPT_DIR%\.cloudflared" mkdir "%SCRIPT_DIR%\.cloudflared"
                
                REM 백그라운드에서 cloudflared 실행
                start /B cloudflared tunnel --config "%SCRIPT_DIR%\.cloudflared\config.yml" run --token "!CLOUDFLARE_TUNNEL_TOKEN!" > "%SCRIPT_DIR%\.cloudflared\tunnel.log" 2>&1
                echo Cloudflare Tunnel을 시작했습니다.
                
                REM 터널 시작을 위해 잠시 대기
                timeout /t 2 /nobreak > nul
                
                REM 터널 시작 확인
                tasklist /FI "IMAGENAME eq cloudflared.exe" 2>nul | find /I "cloudflared.exe" > nul
                if !ERRORLEVEL! equ 0 (
                    echo ✓ Cloudflare Tunnel이 성공적으로 시작되었습니다.
                    echo   로그 위치: %SCRIPT_DIR%\.cloudflared\tunnel.log
                ) else (
                    echo 경고: Cloudflare Tunnel 시작을 확인할 수 없습니다.
                    echo 로그를 확인하세요: %SCRIPT_DIR%\.cloudflared\tunnel.log
                )
            )
        )
    )
) else (
    echo Cloudflare Tunnel이 비활성화되어 있습니다. ^(TUNNEL_ENABLED=false^)
)
echo.

REM 웹서버 실행
echo 가상환경의 파이썬으로 웹서버를 실행합니다...
echo 서버 URL: http://localhost:8080
echo ^(웹브라우저에서 http://localhost:8080 에 접속하세요^)
echo.

cd /d "%SCRIPT_DIR%"
"%VENV_PYTHON%" -m sttEngine.server
set EXIT_CODE=!ERRORLEVEL!

echo.
if !EXIT_CODE! equ 0 (
    echo 서버가 정상적으로 종료되었습니다.
) else (
    echo 서버가 오류와 함께 종료되었습니다. ^(오류코드: !EXIT_CODE!^)
    echo 오류 상세 내용을 확인하세요.
)

pause
endlocal