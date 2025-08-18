@echo off
chcp 65001 > nul
title RecordRoute Server

REM 이 스크립트가 있는 디렉토리 기준으로 경로 설정
set "SCRIPT_DIR=%~dp0"
cd /d "%SCRIPT_DIR%"

REM .env 파일이 있으면 환경변수 로드
if exist ".env" (
    echo .env 파일에서 환경변수를 로드합니다.
    for /f "usebackq tokens=1,2 delims==" %%a in (".env") do (
        if not "%%a"=="" if not "%%a:~0,1%"=="#" (
            set "%%a=%%b"
        )
    )
    echo [DEBUG] 환경변수가 로드되었습니다.
)

REM 가상환경의 Python 실행 파일 경로
set "VENV_PYTHON=%SCRIPT_DIR%venv\Scripts\python.exe"

REM 웹서버 스크립트 경로
set "WEB_SERVER=%SCRIPT_DIR%sttEngine\server.py"

REM 가상환경 존재 확인
if not exist "%VENV_PYTHON%" (
    echo 오류: 가상환경(venv)을 찾을 수 없습니다.
    echo 먼저 설치 스크립트를 실행하여 가상환경을 설정하세요.
    pause
    exit /b 1
)

REM 웹서버 스크립트 존재 확인
if not exist "%WEB_SERVER%" (
    echo 오류: 웹서버 스크립트(server.py)를 찾을 수 없습니다.
    pause
    exit /b 1
)

REM Ollama 서버 상태 확인 및 시작
echo Ollama 서버 상태를 확인합니다...
curl -s http://localhost:11434/api/version >nul 2>&1
if errorlevel 1 (
    echo Ollama 서버가 실행되지 않았습니다. 자동으로 시작합니다...
    where ollama >nul 2>&1
    if not errorlevel 1 (
        REM 백그라운드에서 ollama serve 실행
        start /b "" ollama serve
        echo Ollama 서버를 시작했습니다.
        
        REM 서버 시작을 위해 잠시 대기
        echo 서버 시작을 기다리는 중...
        timeout /t 3 /nobreak >nul
        
        REM 서버 시작 확인
        curl -s http://localhost:11434/api/version >nul 2>&1
        if not errorlevel 1 (
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

REM 웹서버 실행
echo.
echo 가상환경의 파이썬으로 웹서버를 실행합니다...
echo 서버 URL: http://localhost:8080
echo (웹브라우저에서 http://localhost:8080 에 접속하세요)
echo.

"%VENV_PYTHON%" -m sttEngine.server

echo.
echo 서버가 종료되었습니다.
pause