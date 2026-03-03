@echo off
chcp 65001 > nul
setlocal EnableExtensions EnableDelayedExpansion

REM RecordRoute 웹서버 실행 스크립트 (Windows)

REM 스크립트 디렉토리 설정
set "SCRIPT_DIR=%~dp0"
if "%SCRIPT_DIR:~-1%"=="\" set "SCRIPT_DIR=%SCRIPT_DIR:~0,-1%"

REM .env 파일에서 환경변수 로드
if exist "%SCRIPT_DIR%\.env" (
    echo .env 파일에서 환경변수를 로드합니다.
    for /f "usebackq eol=# delims=" %%a in ("%SCRIPT_DIR%\.env") do (
        set "line=%%a"
        if /i "!line:~0,7!"=="export " set "line=!line:~7!"
        if not "!line!"=="" set "!line!"
    )
    echo [DEBUG] PYANNOTE_TOKEN이 로드되었습니다.
)

if not defined OLLAMA_KEEP_ALIVE set "OLLAMA_KEEP_ALIVE=1m"

REM Provider 설정 정규화
set "LLM_PROVIDER_VALUE=%LLM_PROVIDER%"
if "%LLM_PROVIDER_VALUE%"=="" set "LLM_PROVIDER_VALUE=ollama"
set "EMBEDDING_PROVIDER_VALUE=%EMBEDDING_PROVIDER%"
if "%EMBEDDING_PROVIDER_VALUE%"=="" set "EMBEDDING_PROVIDER_VALUE=%LLM_PROVIDER_VALUE%"
if /i "%LLM_PROVIDER_VALUE%"=="ollama" (
    set "NEED_OLLAMA=true"
) else if /i "%EMBEDDING_PROVIDER_VALUE%"=="ollama" (
    set "NEED_OLLAMA=true"
) else (
    set "NEED_OLLAMA=false"
)

set "VENV_PYTHON=%SCRIPT_DIR%\venv\Scripts\python.exe"

set "PLATFORM_SUFFIX=WINDOWS"
set "TRANSCRIBE_MODEL_KEY=TRANSCRIBE_MODEL_%PLATFORM_SUFFIX%"
set "SUMMARY_MODEL_KEY=SUMMARY_MODEL_%PLATFORM_SUFFIX%"
set "EMBEDDING_MODEL_KEY=EMBEDDING_MODEL_%PLATFORM_SUFFIX%"

set "DEFAULT_TRANSCRIBE_MODEL=large-v3-turbo"
set "DEFAULT_SUMMARY_MODEL=gpt-oss:20b"
set "DEFAULT_EMBEDDING_MODEL=bge-m3:latest"

if exist "%VENV_PYTHON%" if exist "%SCRIPT_DIR%\sttEngine\config.py" (
    for /f "tokens=1,2,3 delims=|" %%a in ('"%VENV_PYTHON%" -c "from sttEngine.config import get_default_model; print('|'.join([get_default_model('TRANSCRIBE'), get_default_model('SUMMARY'), get_default_model('EMBEDDING')]))"') do (
        set "DEFAULT_TRANSCRIBE_MODEL=%%a"
        set "DEFAULT_SUMMARY_MODEL=%%b"
        set "DEFAULT_EMBEDDING_MODEL=%%c"
    )
)

call set "SUMMARY_MODEL_RESOLVED=%%%SUMMARY_MODEL_KEY%%%"
if "%SUMMARY_MODEL_RESOLVED%"=="" (
    call set "SUMMARY_MODEL_RESOLVED=%%SUMMARY_MODEL%%"
    if "%SUMMARY_MODEL_RESOLVED%"=="" (
        set "SUMMARY_MODEL_RESOLVED=%DEFAULT_SUMMARY_MODEL%"
    )
)

call set "EMBEDDING_MODEL_RESOLVED=%%%EMBEDDING_MODEL_KEY%%%"
if "%EMBEDDING_MODEL_RESOLVED%"=="" (
    call set "EMBEDDING_MODEL_RESOLVED=%%EMBEDDING_MODEL%%"
    if "%EMBEDDING_MODEL_RESOLVED%"=="" (
        set "EMBEDDING_MODEL_RESOLVED=%DEFAULT_EMBEDDING_MODEL%"
    )
)

REM 가상환경 존재 확인
if not exist "%VENV_PYTHON%" (
    echo 오류: 가상환경(venv)을 찾을 수 없습니다.
    echo 먼저 setup.bat 스크립트를 실행하여 가상환경을 설정하세요.
    exit /b 1
)

REM 의존성 확인 및 설치
set "REQUIREMENTS_FILE=%SCRIPT_DIR%\sttEngine\requirements.txt"
set "REQUIREMENTS_STATE_FILE=%SCRIPT_DIR%\venv\.requirements_hash"

if exist "%REQUIREMENTS_FILE%" (
    echo 필요한 파이썬 패키지를 확인합니다...

    "%VENV_PYTHON%" -c "import hashlib, pathlib, sys; print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())" "%REQUIREMENTS_FILE%" > "%TEMP%\req_hash.tmp"
    set /p REQ_HASH=<"%TEMP%\req_hash.tmp"
    del "%TEMP%\req_hash.tmp"

    set "INSTALLED_HASH="
    if exist "%REQUIREMENTS_STATE_FILE%" (
        set /p INSTALLED_HASH=<"%REQUIREMENTS_STATE_FILE%"
    )

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

REM Ollama optional requirements 설치
set "OLLAMA_REQ_FILE=%SCRIPT_DIR%\requirements-ollama.txt"
if /i "%NEED_OLLAMA%"=="true" if exist "%OLLAMA_REQ_FILE%" (
    echo Ollama provider가 활성화되어 선택 의존성을 확인합니다...
    "%VENV_PYTHON%" -m pip install -r "%OLLAMA_REQ_FILE%" > nul 2>&1
    if !ERRORLEVEL! equ 0 (
        echo Ollama 선택 의존성 확인 완료.
    ) else (
        echo 경고: Ollama 선택 의존성 설치 중 오류가 발생했습니다.
    )
)

REM PyTorch CUDA 버전 확인 및 설치
echo PyTorch 상태를 확인합니다...

"%VENV_PYTHON%" -c "import torch; print('Torch:', torch.__version__); print('CUDA available:', torch.cuda.is_available())" > nul 2>&1
if !ERRORLEVEL! neq 0 (
    echo PyTorch가 설치되지 않았습니다. CUDA 빌드를 설치합니다 (cu124)...
    "%VENV_PYTHON%" -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu124 torch torchvision torchaudio 2>nul
    if !ERRORLEVEL! neq 0 (
        "%VENV_PYTHON%" -m pip install --upgrade torch torchvision torchaudio
    )
) else (
    "%VENV_PYTHON%" -c "import torch; exit(0 if '+cpu' in torch.__version__ else 1)" 2>nul
    if !ERRORLEVEL! equ 0 (
        echo CPU 빌드 PyTorch 감지 → CUDA 빌드로 교체합니다 (cu124)...
        "%VENV_PYTHON%" -m pip uninstall -y torch torchvision torchaudio > nul 2>&1
        "%VENV_PYTHON%" -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu124 torch torchvision torchaudio 2>nul
        if !ERRORLEVEL! neq 0 (
            "%VENV_PYTHON%" -m pip install --upgrade torch torchvision torchaudio
        )
    ) else (
        echo PyTorch CUDA 빌드 또는 호환 빌드가 감지되었습니다.
    )
)

echo PyTorch 상태 확인:
"%VENV_PYTHON%" -c "import torch; print('Torch:', torch.__version__); print('CUDA available:', torch.cuda.is_available())" 2>nul || echo (PyTorch 정보를 가져올 수 없습니다)

REM Ollama 서버 상태 확인 및 시작 (provider가 ollama인 경우에만)
if /i "%NEED_OLLAMA%"=="true" (
    set "OLLAMA_BASE_URL_VALUE=%OLLAMA_BASE_URL%"
    if "%OLLAMA_BASE_URL_VALUE%"=="" set "OLLAMA_BASE_URL_VALUE=http://localhost:11434"
    set "OLLAMA_HOST_VALUE=%OLLAMA_HOST%"
    if "%OLLAMA_HOST_VALUE%"=="" set "OLLAMA_HOST_VALUE=%OLLAMA_BASE_URL_VALUE%"

    set "OLLAMA_BASE_URL_NORMALIZED=%OLLAMA_BASE_URL_VALUE%"
    set "OLLAMA_HOST_NORMALIZED=%OLLAMA_HOST_VALUE%"
    call :trim_trailing_slash OLLAMA_BASE_URL_NORMALIZED
    call :trim_trailing_slash OLLAMA_HOST_NORMALIZED

    if /i "%OLLAMA_HOST_NORMALIZED%" neq "%OLLAMA_BASE_URL_NORMALIZED%" (
        echo 경고: OLLAMA_HOST(%OLLAMA_HOST_NORMALIZED%)와 OLLAMA_BASE_URL(%OLLAMA_BASE_URL_NORMALIZED%)가 다릅니다.
        echo Ollama 통신은 OLLAMA_BASE_URL(%OLLAMA_BASE_URL_NORMALIZED%) 기준으로 고정합니다.
    )
    set "OLLAMA_HOST=%OLLAMA_BASE_URL_NORMALIZED%"
    set "OLLAMA_VERSION_URL=%OLLAMA_BASE_URL_NORMALIZED%/api/version"

    echo Ollama provider가 활성화되어 서버 상태를 확인합니다...
    curl -s "%OLLAMA_VERSION_URL%" > nul 2>&1
    if !ERRORLEVEL! neq 0 (
        echo Ollama 서버가 실행되지 않았습니다. 자동으로 시작합니다...
        where ollama > nul 2>&1
        if !ERRORLEVEL! equ 0 (
            start /B ollama serve > nul 2>&1
            echo Ollama 서버를 시작했습니다.
            echo 서버 시작을 기다리는 중...
            timeout /t 3 /nobreak > nul
            curl -s "%OLLAMA_VERSION_URL%" > nul 2>&1
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
) else (
    echo Ollama provider가 비활성화되어 서버 자동 시작을 건너뜁니다.
)

if /i "%NEED_OLLAMA%"=="true" (
    echo Ollama 필수 모델 존재 여부를 확인합니다...
    set "OLLAMA_CHECK_MODELS="
    if /i "%LLM_PROVIDER_VALUE%"=="ollama" if defined SUMMARY_MODEL_RESOLVED if not "%SUMMARY_MODEL_RESOLVED%"=="" set "OLLAMA_CHECK_MODELS=%OLLAMA_CHECK_MODELS% %SUMMARY_MODEL_RESOLVED%"
    if /i "%EMBEDDING_PROVIDER_VALUE%"=="ollama" if defined EMBEDDING_MODEL_RESOLVED if not "%EMBEDDING_MODEL_RESOLVED%"=="" set "OLLAMA_CHECK_MODELS=%OLLAMA_CHECK_MODELS% %EMBEDDING_MODEL_RESOLVED%"

    call :check_ollama_models %OLLAMA_CHECK_MODELS%
    if !ERRORLEVEL! neq 0 (
        echo 필수 Ollama 모델이 준비되지 않았습니다. 서버를 시작하지 않고 종료합니다.
        echo 설치 명령: ollama pull <모델명>
        exit /b 1
    )
)

REM Cloudflare Tunnel 상태 확인 및 시작
if /i "%TUNNEL_ENABLED%"=="true" (
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
        if "%CLOUDFLARE_TUNNEL_TOKEN%"=="" (
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
                if not exist "%SCRIPT_DIR%\.cloudflared" mkdir "%SCRIPT_DIR%\.cloudflared"
                start /B cloudflared tunnel --config "%SCRIPT_DIR%\.cloudflared\config.yml" run --token "%CLOUDFLARE_TUNNEL_TOKEN%" > "%SCRIPT_DIR%\.cloudflared\tunnel.log" 2>&1
                echo Cloudflare Tunnel을 시작했습니다.

                timeout /t 2 /nobreak > nul
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
    echo Cloudflare Tunnel이 비활성화되어 있습니다. (TUNNEL_ENABLED=false)
)
echo.

REM 프론트엔드 빌드 확인
if not exist "%SCRIPT_DIR%\frontend\dist" (
    echo 프론트엔드 빌드가 없습니다. 빌드를 시도합니다...
    where npm >nul 2>&1
    if !ERRORLEVEL! equ 0 (
        cd /d "%SCRIPT_DIR%\frontend"
        call npm install
        call npm run build
        if !ERRORLEVEL! equ 0 (
            echo 프론트엔드 빌드 완료.
        ) else (
            echo 경고: 프론트엔드 빌드에 실패했습니다. 레거시 UI로 실행됩니다.
        )
        cd /d "%SCRIPT_DIR%"
    ) else (
        echo 경고: npm이 설치되지 않았습니다. 레거시 UI로 실행됩니다.
        echo 프론트엔드를 빌드하려면: cd frontend ^&^& npm install ^&^& npm run build
    )
)

echo 가상환경의 파이썬으로 웹서버를 실행합니다...
echo 서버 URL: http://localhost:8080
echo (웹브라우저에서 http://localhost:8080 에 접속하세요)
echo.

cd /d "%SCRIPT_DIR%"
"%VENV_PYTHON%" -m sttEngine.server
set EXIT_CODE=!ERRORLEVEL!

echo.
if !EXIT_CODE! equ 0 (
    echo 서버가 정상적으로 종료되었습니다.
) else (
    echo 서버가 오류와 함께 종료되었습니다. (오류코드: !EXIT_CODE!)
    echo 오류 상세 내용을 확인하세요.
)

exit /b !EXIT_CODE!
endlocal

:trim_trailing_slash
set "trim_name=%~1"
set "trim_value=!%trim_name%!"

:trim_trailing_slash_loop
if "%trim_value:~-1%"=="/" (
    set "trim_value=%trim_value:~0,-1%"
    goto trim_trailing_slash_loop
)

set "%trim_name%=%trim_value%"
exit /b 0

:check_ollama_models
setlocal EnableExtensions EnableDelayedExpansion
set "models=%*"

if "%models%"=="" (
    endlocal
    exit /b 0
)

where ollama > nul 2>&1
if !ERRORLEVEL! neq 0 (
    echo 오류: ollama 명령어를 찾을 수 없습니다.
    endlocal
    exit /b 1
)

set "installed_models="
for /f "skip=1 tokens=1" %%m in ('ollama list 2^>nul') do (
    set "installed_models=!installed_models!;%%m;"
)

if "%installed_models%"=="" (
    echo 오류: Ollama 모델 목록을 읽지 못했습니다. Ollama 서버가 실행 중인지 확인하세요.
    endlocal
    exit /b 1
)

set "missing=0"
set "seen="
for %%m in (%models%) do (
    set "model=%%m"
    if "!model!"=="" (
        echo 오류: 필요 모델명이 비어 있습니다. .env에서 SUMMARY_MODEL 또는 EMBEDDING_MODEL 값을 확인하세요.
        set "missing=1"
    ) else (
        echo !seen! | findstr /L /C:";!model!;" > nul
        if !ERRORLEVEL! neq 0 (
            set "seen=!seen!;!model!;"
            echo !installed_models! | findstr /L /C:";!model!;" > nul
            if !ERRORLEVEL! equ 0 (
                echo ✓ Ollama 모델 확인됨: !model!
            ) else (
                echo 오류: Ollama에 모델이 없습니다: !model!
                set "missing=1"
            )
        )
    )
)

if "!missing!"=="1" (
    endlocal
    exit /b 1
)

endlocal
exit /b 0
