@echo off
REM Windows용 RecordRoute 워크플로우 실행 스크립트

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

REM 워크플로우 스크립트 경로
set "WORKFLOW_SCRIPT=%SCRIPT_DIR%\sttEngine\run_workflow.py"

REM 가상환경 존재 확인
if not exist "%VENV_PYTHON%" (
    echo 오류: 가상환경(venv)을 찾을 수 없습니다.
    echo 먼저 setup.bat 스크립트를 실행하여 가상환경을 설정하세요.
    pause
    exit /b 1
)

REM 워크플로우 스크립트 존재 확인
if not exist "%WORKFLOW_SCRIPT%" (
    echo 오류: 워크플로우 스크립트(sttEngine\run_workflow.py)를 찾을 수 없습니다.
    pause
    exit /b 1
)

REM 워크플로우 스크립트 실행
echo 가상환경의 파이썬으로 워크플로우를 실행합니다...
echo (스크립트 경로: %WORKFLOW_SCRIPT%)
"%VENV_PYTHON%" "%WORKFLOW_SCRIPT%" %*

REM 실행 완료 후 일시 정지 (창이 바로 닫히는 것을 방지)
if "%~1"=="" pause
