@echo off
REM Whisper 모델 다운로드 스크립트 (Windows)
REM 이 스크립트는 whisper.cpp에서 사용할 Whisper 모델을 다운로드합니다.

setlocal EnableDelayedExpansion

REM 프로젝트 루트 경로 설정
set SCRIPT_DIR=%~dp0
cd /d "%SCRIPT_DIR%..\.."
set PROJECT_ROOT=%CD%
set MODELS_DIR=%PROJECT_ROOT%\models

echo =========================================
echo Whisper 모델 다운로드
echo =========================================
echo.

REM 모델 선택 옵션
echo 다운로드할 Whisper 모델을 선택하세요:
echo.
echo 1) ggml-tiny.bin     - 가장 빠름, 낮은 정확도 (~75MB)
echo 2) ggml-base.bin     - 균형잡힌 성능 (권장, ~142MB)
echo 3) ggml-small.bin    - 높은 정확도 (~466MB)
echo 4) ggml-medium.bin   - 매우 높은 정확도 (~1.5GB)
echo 5) ggml-large-v3.bin - 최고 정확도, 한국어 최적화 (~2.9GB)
echo.
set /p choice="선택 (1-5) [기본값: 2]: "
if "%choice%"=="" set choice=2

if "%choice%"=="1" (
    set MODEL_NAME=ggml-tiny.bin
) else if "%choice%"=="2" (
    set MODEL_NAME=ggml-base.bin
) else if "%choice%"=="3" (
    set MODEL_NAME=ggml-small.bin
) else if "%choice%"=="4" (
    set MODEL_NAME=ggml-medium.bin
) else if "%choice%"=="5" (
    set MODEL_NAME=ggml-large-v3.bin
) else (
    echo 잘못된 선택입니다. 기본값(base)을 사용합니다.
    set MODEL_NAME=ggml-base.bin
)

REM models 디렉토리 생성
echo.
echo models 디렉토리 생성 중...
if not exist "%MODELS_DIR%" mkdir "%MODELS_DIR%"

REM 모델 다운로드 URL
set MODEL_URL=https://huggingface.co/ggerganov/whisper.cpp/resolve/main/%MODEL_NAME%
set MODEL_PATH=%MODELS_DIR%\%MODEL_NAME%

REM 이미 존재하는지 확인
if exist "%MODEL_PATH%" (
    echo.
    echo 경고: %MODEL_NAME% 파일이 이미 존재합니다.
    set /p overwrite="덮어쓰시겠습니까? (y/N): "
    if /i not "!overwrite!"=="y" (
        echo 다운로드를 취소합니다.
        exit /b 0
    )
    del "%MODEL_PATH%"
)

echo.
echo 다운로드 중: %MODEL_NAME%
echo URL: %MODEL_URL%
echo 저장 위치: %MODEL_PATH%
echo.

REM PowerShell을 사용하여 다운로드
powershell -Command "& {$ProgressPreference = 'SilentlyContinue'; Invoke-WebRequest -Uri '%MODEL_URL%' -OutFile '%MODEL_PATH%'}"

REM 다운로드 성공 확인
if exist "%MODEL_PATH%" (
    echo.
    echo =========================================
    echo 다운로드 완료!
    echo =========================================
    echo.
    echo 모델 파일: %MODEL_PATH%
    echo.
    echo 이제 RecordRoute를 실행할 수 있습니다:
    echo   cd %PROJECT_ROOT%\recordroute-rs
    echo   cargo run --release
    echo.

    REM .env 파일 확인 및 제안
    if not exist "%PROJECT_ROOT%\.env" (
        echo 팁: .env 파일을 생성하여 모델 경로를 설정할 수 있습니다:
        echo   echo WHISPER_MODEL=./models/%MODEL_NAME% ^> %PROJECT_ROOT%\.env
        echo.
    )
) else (
    echo.
    echo Error: 다운로드에 실패했습니다.
    echo curl이 설치되어 있다면 다음 명령어로 수동으로 다운로드하세요:
    echo   curl -L -o "%MODEL_PATH%" "%MODEL_URL%"
    exit /b 1
)

pause
