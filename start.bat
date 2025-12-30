@echo off
REM RecordRoute 실행 스크립트 (Windows)
REM 이 스크립트는 RecordRoute를 시작합니다

echo RecordRoute를 시작합니다...

REM Node.js와 npm이 설치되어 있는지 확인
where npm >nul 2>nul
if %ERRORLEVEL% NEQ 0 (
    echo 오류: npm을 찾을 수 없습니다. Node.js가 설치되어 있는지 확인하세요.
    pause
    exit /b 1
)

REM node_modules가 없으면 설치
if not exist "node_modules\" (
    echo node_modules가 없습니다. npm install을 실행합니다...
    call npm install
    if %ERRORLEVEL% NEQ 0 (
        echo 오류: npm install 실패
        pause
        exit /b 1
    )
)

REM Python venv가 없으면 생성
if not exist "venv\" (
    echo Python 가상환경이 없습니다. 가상환경을 생성합니다...
    python -m venv venv
    if %ERRORLEVEL% NEQ 0 (
        echo 오류: Python 가상환경 생성 실패
        pause
        exit /b 1
    )

    echo Python 패키지를 설치합니다...
    call venv\Scripts\activate.bat
    pip install -r sttEngine\requirements.txt
    if %ERRORLEVEL% NEQ 0 (
        echo 오류: Python 패키지 설치 실패
        pause
        exit /b 1
    )
)

REM Electron 앱 시작
echo RecordRoute를 실행합니다...
call npm start
