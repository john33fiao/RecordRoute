@echo off
chcp 65001 > nul
title RecordRoute 초기 설정

set "SCRIPT_DIR=%~dp0"
cd /d "%SCRIPT_DIR%"

REM Prefer a system Python outside the local venv for creating venv
set "SYSTEM_PYTHON="
for /f "delims=" %%P in ('where python 2^>nul') do (
    echo %%P | findstr /I /C:"%SCRIPT_DIR%\venv\Scripts\python.exe" >nul
    if errorlevel 1 (
        if not defined SYSTEM_PYTHON set "SYSTEM_PYTHON=%%P"
    )
)

echo "================================"
echo "   RecordRoute 초기 설정"
echo "================================"
echo.

REM Python 설치 확인
set "PY_CMD=python"
where py >nul 2>&1
if not errorlevel 1 (
    set "PY_CMD=py -3"
)
%PY_CMD% --version >nul 2>&1
if errorlevel 1 (
    echo "오류: Python이 설치되지 않았습니다."
    echo "Python 3.8 이상을 설치한 후 다시 실행하세요."
    pause
    exit /b 1
)

echo 1단계: 가상환경 생성...
if exist "venv" (
    echo "기존 가상환경이 있습니다. 삭제 후 재생성하시겠습니까? (Y/N)"
    set /p confirm=
if /i "%confirm%"=="Y" (
        echo "가상환경을 비활성화합니다(활성 상태인 경우)."
        if exist "venv\Scripts\deactivate.bat" (
            call venv\Scripts\deactivate.bat
        )
        echo "기존 venv 폴더를 삭제합니다..."
        rmdir /s /q venv
        if errorlevel 1 (
            echo "경고: venv 삭제 중 문제가 발생했습니다. 다른 프로세스가 사용 중일 수 있습니다."
            echo "해당 폴더를 수동으로 닫거나 PC를 재부팅한 후 다시 시도하세요."
            pause
        )
    ) else (
        echo "기존 가상환경을 사용합니다."
        goto INSTALL_DEPS
    )
)

REM Choose Python interpreter to create venv
set "CREATE_PY=%PY_CMD%"
if defined SYSTEM_PYTHON set "CREATE_PY=%SYSTEM_PYTHON%"

%CREATE_PY% -m venv venv
if errorlevel 1 (
    echo "오류: 가상환경 생성에 실패했습니다."
    pause
    exit /b 1
)

:INSTALL_DEPS
echo.
echo "2단계: 가상환경 활성화 및 의존성 설치..."
call venv\Scripts\activate

if exist "sttEngine\requirements.txt" (
    pip install -r sttEngine\requirements.txt
) else (
    echo "경고: sttEngine\requirements.txt 파일을 찾을 수 없습니다."
)

if exist "requirements.txt" (
    pip install -r requirements.txt
)

echo.
echo "PyTorch CUDA 빌드 확인 및 설치..."
set "TORCH_VER_FILE=%SCRIPT_DIR%\venv\Lib\site-packages\torch\version.py"
if not exist "%TORCH_VER_FILE%" (
    echo "PyTorch가 설치되지 않았습니다. CUDA 빌드를 설치합니다 (cu124)..."
    %PY_CMD% -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu124 torch
) else (
    findstr /C:"+cpu'" "%TORCH_VER_FILE%" >nul 2>&1
    if not errorlevel 1 (
        echo "CPU 빌드 PyTorch 감지 → CUDA 빌드로 교체합니다 (cu124)..."
        %PY_CMD% -m pip uninstall -y torch >nul 2>&1
        %PY_CMD% -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu124 torch
    ) else (
        echo "PyTorch CUDA 빌드 또는 호환 빌드가 감지되었습니다."
    )
)

python -c "import torch; print('Torch:', torch.__version__); print('CUDA runtime:', getattr(getattr(torch,'version',None),'cuda',None)); print('cuda.is_available:', bool(getattr(torch,'cuda',None) and torch.cuda.is_available()))"

echo.
echo "3단계: Ollama 설치 확인..."
where ollama >nul 2>&1
if errorlevel 1 (
    echo "Ollama가 설치되지 않았습니다."
    echo "https://ollama.ai 에서 다운로드하여 설치하세요."
    echo "설치 후 'ollama pull gemma3:4b-it-qat' 명령어를 실행하세요."
) else (
    echo "Ollama가 설치되어 있습니다."
    echo "필요한 모델을 다운로드합니다..."
    ollama list | findstr "gemma3:4b" >nul
    if errorlevel 1 (
        echo "gemma3:4b 모델을 다운로드합니다..."
        ollama pull gemma3:4b-it-qat
    ) else (
        echo "gemma3:4b 모델이 이미 설치되어 있습니다."
    )
)

echo.
echo "4단계: FFmpeg 확인..."
where ffmpeg >nul 2>&1
if errorlevel 1 (
    echo "FFmpeg가 설치되지 않았습니다."
    echo "winget install FFmpeg 또는 수동 설치가 필요합니다."
) else (
    echo "FFmpeg가 설치되어 있습니다."
)

echo.
echo "================================"
echo "      설정이 완료되었습니다!"
echo "================================"
echo "이제 run.bat 파일을 실행하여 서버를 시작할 수 있습니다."
echo.
pause
