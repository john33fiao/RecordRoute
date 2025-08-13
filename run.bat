@echo off
REM Windows용 RecordRoute 웹서버 실행 스크립트

REM 이 스크립트가 있는 디렉토리 기준으로 경로 설정
set "SCRIPT_DIR=%~dp0"
REM 경로 끝의 백슬래시 제거
if "%SCRIPT_DIR:~-1%"=="\" set "SCRIPT_DIR=%SCRIPT_DIR:~0,-1%"

REM .env 파일이 있으면 환경변수 로드
if exist "%SCRIPT_DIR%\.env" (
    echo .env 파일에서 환경변수를 로드합니다.
    for /f "usebackq tokens=1* delims== eol=#" %%a in ("%SCRIPT_DIR%\.env") do (
        if not "%%b"=="" (
            set "%%a=%%b"
        )
    )
    echo [DEBUG] 로드된 토큰: %PYANNOTE_TOKEN%
)

REM 가상환경의 Python 실행 파일 경로
set "VENV_PYTHON=%SCRIPT_DIR%\venv\Scripts\python.exe"

REM 웹서버 스크립트 경로
set "WEB_SERVER=%SCRIPT_DIR%\server.py"

REM 가상환경 존재 확인
if not exist "%VENV_PYTHON%" (
    echo 오류: 가상환경(venv)을 찾을 수 없습니다.
    echo 먼저 setup.bat 스크립트를 실행하여 가상환경을 설정하세요.
    pause
    exit /b 1
)

REM 웹서버 스크립트 존재 확인
if not exist "%WEB_SERVER%" (
    echo 오류: 웹서버 스크립트(server.py)를 찾을 수 없습니다.
    pause
    exit /b 1
)

REM 웹서버 실행
echo 가상환경의 파이썬으로 웹서버를 실행합니다...
echo 서버 URL: http://localhost:8080
echo (웹브라우저에서 http://localhost:8080 에 접속하세요)
echo.
"%VENV_PYTHON%" "%WEB_SERVER%"

REM 서버 종료 후 일시 정지
echo.
echo 서버가 종료되었습니다.
pause
