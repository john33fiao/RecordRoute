@echo off
REM RecordRoute 웹서버 실행 스크립트 (Windows)

setlocal enabledelayedexpansion

REM 이 스크립트가 있는 디렉토리를 기준으로 경로 설정
set SCRIPT_DIR=%~dp0
set SCRIPT_DIR=%SCRIPT_DIR:~0,-1%

echo.
echo ============================================
echo RecordRoute 웹서버 시작
echo ============================================
echo.

REM .env 파일이 있으면 환경변수 로드
if exist "%SCRIPT_DIR%\.env" (
    echo .env 파일에서 환경변수를 로드합니다.
    for /f "delims=" %%i in ('type "%SCRIPT_DIR%\.env" ^| findstr /v "^#"') do (
        set "%%i"
    )
    echo [DEBUG] PYANNOTE_TOKEN이 로드되었습니다.
) else (
    echo .env 파일을 찾을 수 없습니다. (선택사항)
)

REM 가상환경의 Python 실행 파일 경로
set VENV_PYTHON=%SCRIPT_DIR%\venv\Scripts\python.exe

REM 웹서버 스크립트 경로
set WEB_SERVER=%SCRIPT_DIR%\sttEngine\server.py

REM 가상환경 존재 확인
if not exist "%VENV_PYTHON%" (
    echo.
    echo [오류] 가상환경^(venv^)을 찾을 수 없습니다.
    echo 먼저 setup.bat 스크립트를 실행하여 가상환경을 설정하세요.
    echo.
    pause
    exit /b 1
)

REM 웹서버 스크립트 존재 확인
if not exist "%WEB_SERVER%" (
    echo.
    echo [오류] 웹서버 스크립트^(server.py^)를 찾을 수 없습니다.
    echo.
    pause
    exit /b 1
)

REM 의존성 확인 및 설치
set REQUIREMENTS_FILE=%SCRIPT_DIR%\sttEngine\requirements.txt
set REQUIREMENTS_STATE_FILE=%SCRIPT_DIR%\venv\.requirements_hash

if exist "%REQUIREMENTS_FILE%" (
    echo 필요한 파이썬 패키지를 확인합니다...
    
    REM requirements.txt 해시 계산
    for /f %%i in ('certutil -hashfile "%REQUIREMENTS_FILE%" SHA256 ^| findstr /v "certutil"') do (
        set REQ_HASH=%%i
        goto :hash_done
    )
    :hash_done
    
    set INSTALLED_HASH=
    if exist "%REQUIREMENTS_STATE_FILE%" (
        for /f "delims=" %%i in ('type "%REQUIREMENTS_STATE_FILE%"') do (
            set INSTALLED_HASH=%%i
        )
    )
    
    if not "!REQ_HASH!"=="!INSTALLED_HASH!" (
        echo 의존성을 설치/업데이트합니다...
        "%VENV_PYTHON%" -m pip install -r "%REQUIREMENTS_FILE%"
        if !errorlevel! equ 0 (
            echo !REQ_HASH! > "%REQUIREMENTS_STATE_FILE%"
            echo 필요한 패키지가 준비되었습니다.
        ) else (
            echo [경고] 의존성 설치에 실패했습니다. 설치 로그를 확인하고 다시 시도하세요.
        )
    ) else (
        echo 필요한 파이썬 패키지가 이미 설치되어 있습니다.
    )
) else (
    echo [경고] sttEngine\requirements.txt 파일을 찾을 수 없습니다.
)

REM 루트 requirements.txt 확인 및 설치
set ROOT_REQ_FILE=%SCRIPT_DIR%\requirements.txt
if exist "%ROOT_REQ_FILE%" (
    echo 루트 requirements.txt를 확인합니다...
    "%VENV_PYTHON%" -m pip install -r "%ROOT_REQ_FILE%" >nul 2>&1
    if !errorlevel! equ 0 (
        echo 루트 의존성 확인 완료.
    ) else (
        echo [경고] 루트 의존성 설치 중 오류가 발생했습니다.
    )
)

REM PyTorch CUDA 버전 확인 및 설치
echo.
echo PyTorch 상태를 확인합니다...

"%VENV_PYTHON%" -c "import torch; print('Torch: ' + torch.__version__); print('CUDA available: ' + str(torch.cuda.is_available()))" >nul 2>&1

if !errorlevel! equ 0 (
    REM PyTorch가 이미 설치됨 - CUDA 빌드 확인
    for /f %%i in ('"%VENV_PYTHON%" -c "import torch; print(torch.__version__)" 2^>nul') do (
        echo PyTorch %%i이 설치되어 있습니다.
    )
) else (
    echo PyTorch가 설치되지 않았습니다. CUDA 빌드를 설치합니다 (cu124)...
    "%VENV_PYTHON%" -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu124 torch torchvision torchaudio
    if !errorlevel! neq 0 (
        echo [경고] CUDA 빌드 설치 실패. 기본 빌드로 설치합니다...
        "%VENV_PYTHON%" -m pip install --upgrade torch torchvision torchaudio
    )
)

REM PyTorch 버전 정보 출력
echo.
echo PyTorch 상태 확인:
"%VENV_PYTHON%" -c "import torch; print('Torch: ' + torch.__version__); print('CUDA available: ' + str(torch.cuda.is_available()))" 2>nul || echo (PyTorch 정보를 가져올 수 없습니다)

REM Ollama 서버 상태 확인 및 시작
echo.
echo Ollama 서버 상태를 확인합니다...

for /f %%i in ('powershell -Command "try { $r = Invoke-WebRequest -Uri http://localhost:11434/api/version -TimeoutSec 2 -ErrorAction SilentlyContinue; if ($r.StatusCode -eq 200) { Write-Host 'running' } } catch { }" 2^>nul') do (
    if "%%i"=="running" (
        echo Ollama 서버가 이미 실행 중입니다.
        goto :ollama_done
    )
)

echo Ollama 서버가 실행되지 않았습니다. 자동으로 시작합니다...

REM ollama 명령어 확인
where ollama >nul 2>&1
if !errorlevel! equ 0 (
    echo Ollama 서버를 시작하고 있습니다...
    start /b ollama serve >nul 2>&1
    
    REM 서버 시작을 위해 3초 대기
    echo 서버 시작을 기다리는 중...
    timeout /t 3 /nobreak >nul
    
    REM 서버 시작 확인
    for /f %%i in ('powershell -Command "try { $r = Invoke-WebRequest -Uri http://localhost:11434/api/version -TimeoutSec 2 -ErrorAction SilentlyContinue; if ($r.StatusCode -eq 200) { Write-Host 'ok' } } catch { }" 2^>nul') do (
        if "%%i"=="ok" (
            echo Ollama 서버가 성공적으로 시작되었습니다.
            goto :ollama_done
        )
    )
    echo [경고] Ollama 서버 시작을 확인할 수 없습니다. 수동으로 'ollama serve'를 실행해주세요.
) else (
    echo [경고] ollama 명령어를 찾을 수 없습니다.
    echo Ollama가 설치되어 있는지 확인하세요.
    echo 수동으로 'ollama serve' 명령어를 실행한 후 이 스크립트를 다시 실행하세요.
)

:ollama_done

REM Cloudflare Tunnel 상태 확인 및 시작 (선택사항)
if /i "!TUNNEL_ENABLED!"=="true" (
    echo.
    echo Cloudflare Tunnel이 활성화되어 있습니다. 상태를 확인합니다...
    
    where cloudflared >nul 2>&1
    if !errorlevel! neq 0 (
        echo [경고] cloudflared 명령어를 찾을 수 없습니다.
        echo Cloudflare Tunnel을 사용하려면 cloudflared를 설치해야 합니다.
        echo 설치 방법: https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/installation/
    ) else (
        if "!CLOUDFLARE_TUNNEL_TOKEN!"=="" (
            echo [경고] CLOUDFLARE_TUNNEL_TOKEN이 설정되지 않았습니다.
            echo .env 파일에 CLOUDFLARE_TUNNEL_TOKEN을 설정해주세요.
        ) else (
            REM cloudflared 프로세스 확인
            tasklist | find /i "cloudflared.exe" >nul
            if !errorlevel! equ 0 (
                echo Cloudflare Tunnel이 이미 실행 중입니다.
            ) else (
                echo Cloudflare Tunnel을 시작합니다...
                if exist "%SCRIPT_DIR%\.cloudflared\config.yml" (
                    start /b cloudflared tunnel --config "%SCRIPT_DIR%\.cloudflared\config.yml" run --token "!CLOUDFLARE_TUNNEL_TOKEN!" >"%SCRIPT_DIR%\.cloudflared\tunnel.log" 2>&1
                    echo Cloudflare Tunnel을 시작했습니다.
                    echo 로그 위치: %SCRIPT_DIR%\.cloudflared\tunnel.log
                ) else (
                    echo [경고] Cloudflare Tunnel 설정 파일을 찾을 수 없습니다.
                )
            )
        )
    )
) else (
    echo Cloudflare Tunnel이 비활성화되어 있습니다. ^(TUNNEL_ENABLED=false^)
)

echo.
echo ============================================
echo 웹서버를 시작합니다...
echo ============================================
echo.
echo 서버 URL: http://localhost:8080
echo (웹브라우저에서 http://localhost:8080 에 접속하세요)
echo.

cd /d "%SCRIPT_DIR%"
"%VENV_PYTHON%" -m sttEngine.server
set EXIT_CODE=!errorlevel!

echo.
if !EXIT_CODE! equ 0 (
    echo 서버가 정상적으로 종료되었습니다.
) else (
    echo 서버가 오류와 함께 종료되었습니다. (오류코드: !EXIT_CODE!)
    echo 오류 상세 내용을 확인하세요.
)

endlocal
pause
