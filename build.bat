@echo off
REM RecordRoute 전체 빌드 스크립트 (Windows)
REM 이 스크립트는 프로젝트의 모든 컴포넌트를 순차적으로 빌드합니다.

echo ====================================
echo RecordRoute 전체 빌드 시작
echo ====================================
echo.

REM 빌드 시작 시간 기록
set START_TIME=%time%

REM 1. 의존성 설치
echo [1/3] NPM 의존성 설치 중...
echo ------------------------------------
call npm install
if %ERRORLEVEL% neq 0 (
    echo [오류] NPM 의존성 설치 실패
    exit /b 1
)
echo.

REM 2. Rust 프로젝트 빌드
echo [2/3] Rust 프로젝트 빌드 중...
echo ------------------------------------
cd recordroute-rs
call cargo build --release
if %ERRORLEVEL% neq 0 (
    echo [오류] Rust 빌드 실패
    cd ..
    exit /b 1
)
cd ..
echo.

REM 3. Electron 앱 빌드
echo [3/3] Electron 앱 빌드 중...
echo ------------------------------------
call npm run build
if %ERRORLEVEL% neq 0 (
    echo [오류] Electron 빌드 실패
    exit /b 1
)
echo.

REM 빌드 완료
echo ====================================
echo 빌드 완료!
echo 시작 시간: %START_TIME%
echo 종료 시간: %time%
echo ====================================
echo.
echo 빌드된 파일 위치: dist/
echo.

exit /b 0
