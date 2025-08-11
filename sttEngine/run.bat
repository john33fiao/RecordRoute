@echo off
:: Whisper 워크플로우 실행 스크립트

:: 가상환경 활성화
if exist "venv\Scripts\activate.bat" (
    echo 가상환경 활성화 중...
    call venv\Scripts\activate.bat
) else (
    echo 경고: 가상환경이 없습니다. setup.bat를 먼저 실행하세요.
    pause
    exit /b 1
)

:: Python 스크립트 실행
echo Whisper 워크플로우를 시작합니다...
python run_workflow.py %*

pause