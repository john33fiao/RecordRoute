@echo off
REM RecordRoute 개발 모드 실행 스크립트
REM Electron 앱을 시작하면 Python 백엔드가 자동으로 실행됩니다 (main.js 참조)

REM 프로젝트 루트 디렉토리로 이동
cd /d "%~dp0"

REM 가상환경 확인
if not exist "venv\Scripts\activate.bat" (
    echo [오류] 가상환경이 없습니다.
    echo 먼저 다음 명령으로 가상환경을 생성하세요:
    echo   python -m venv venv
    echo   venv\Scripts\activate
    echo   pip install -r sttEngine\requirements.txt
    pause
    exit /b 1
)

REM Node.js 의존성 확인
if not exist "node_modules" (
    echo Node.js 의존성을 설치합니다...
    call npm install
    if errorlevel 1 (
        echo [오류] npm install 실패
        pause
        exit /b 1
    )
)

REM Electron 앱 시작
REM (Python 백엔드는 electron/main.js에서 자동으로 시작됨)
echo RecordRoute를 시작합니다...
call npm start
