@echo off
chcp 65001 > nul
title RecordRoute 초기 설정

set "SCRIPT_DIR=%~dp0"
cd /d "%SCRIPT_DIR%"

echo ================================
echo    RecordRoute 초기 설정
echo ================================
echo.

REM Python 설치 확인
python --version >nul 2>&1
if errorlevel 1 (
    echo 오류: Python이 설치되지 않았습니다.
    echo Python 3.8 이상을 설치한 후 다시 실행하세요.
    pause
    exit /b 1
)

echo 1단계: 가상환경 생성...
if exist "venv" (
    echo 기존 가상환경이 있습니다. 삭제 후 재생성하시겠습니까? (Y/N)
    set /p confirm=
    if /i "%confirm%"=="Y" (
        rmdir /s /q venv
    ) else (
        echo 기존 가상환경을 사용합니다.
        goto INSTALL_DEPS
    )
)

python -m venv venv
if errorlevel 1 (
    echo 오류: 가상환경 생성에 실패했습니다.
    pause
    exit /b 1
)

:INSTALL_DEPS
echo.
echo 2단계: 가상환경 활성화 및 의존성 설치...
call venv\Scripts\activate

if exist "sttEngine\requirements.txt" (
    pip install -r sttEngine\requirements.txt
) else (
    echo 경고: sttEngine\requirements.txt 파일을 찾을 수 없습니다.
)

if exist "requirements.txt" (
    pip install -r requirements.txt
)

echo.
echo 3단계: Ollama 설치 확인...
where ollama >nul 2>&1
if errorlevel 1 (
    echo Ollama가 설치되지 않았습니다.
    echo https://ollama.ai 에서 다운로드하여 설치하세요.
    echo 설치 후 'ollama pull gemma3:4b-it-qat' 명령어를 실행하세요.
) else (
    echo Ollama가 설치되어 있습니다.
    echo 필요한 모델을 다운로드합니다...
    ollama list | findstr "gemma3:4b" >nul
    if errorlevel 1 (
        echo gemma3:4b 모델을 다운로드합니다...
        ollama pull gemma3:4b-it-qat
    ) else (
        echo gemma3:4b 모델이 이미 설치되어 있습니다.
    )
)

echo.
echo 4단계: FFmpeg 확인...
where ffmpeg >nul 2>&1
if errorlevel 1 (
    echo FFmpeg가 설치되지 않았습니다.
    echo winget install FFmpeg 또는 수동 설치가 필요합니다.
) else (
    echo FFmpeg가 설치되어 있습니다.
)

echo.
echo ================================
echo       설정이 완료되었습니다!
echo ================================
echo 이제 run.bat 파일을 실행하여 서버를 시작할 수 있습니다.
echo.
pause