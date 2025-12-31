@echo off
REM llama.cpp 빌드 스크립트 (Windows)
REM 이 스크립트는 third-party/llama.cpp를 빌드하여 llama-server를 생성합니다.

setlocal enabledelayedexpansion

set "SCRIPT_DIR=%~dp0"
set "PROJECT_ROOT=%SCRIPT_DIR%..\.."
set "LLAMA_DIR=%PROJECT_ROOT%\third-party\llama.cpp"
set "BUILD_DIR=%LLAMA_DIR%\build"

echo =========================================
echo llama.cpp 빌드 시작
echo =========================================
echo.

REM llama.cpp 디렉토리 존재 확인
if not exist "%LLAMA_DIR%" (
    echo Error: llama.cpp 서브모듈이 없습니다.
    echo 다음 명령어를 실행하세요:
    echo   git submodule update --init --recursive
    exit /b 1
)

cd /d "%LLAMA_DIR%"

REM 서브모듈 업데이트
echo 서브모듈 업데이트 중...
git submodule update --init --recursive

REM 빌드 디렉토리 생성
if not exist "%BUILD_DIR%" mkdir "%BUILD_DIR%"
cd /d "%BUILD_DIR%"

REM CMake 설정
echo CMake 설정 중...
cmake .. ^
    -DCMAKE_BUILD_TYPE=Release ^
    -DGGML_NATIVE=OFF ^
    -DGGML_CUDA=OFF ^
    -DLLAMA_BUILD_TESTS=OFF ^
    -DLLAMA_BUILD_EXAMPLES=ON ^
    -DLLAMA_BUILD_SERVER=ON

if errorlevel 1 (
    echo CMake 설정 실패
    exit /b 1
)

REM 빌드
echo 빌드 중... (시간이 다소 걸릴 수 있습니다)
cmake --build . --config Release

if errorlevel 1 (
    echo 빌드 실패
    exit /b 1
)

echo.
echo =========================================
echo llama.cpp 빌드 완료!
echo =========================================
echo.
echo llama-server 위치: %BUILD_DIR%\bin\Release\llama-server.exe
echo.
echo llama-server 실행 예시:
echo   %BUILD_DIR%\bin\Release\llama-server.exe -m C:\path\to\model.gguf --host 127.0.0.1 --port 8081
echo.

endlocal
