@echo off
echo Whisper 음성 인식 워크플로우 설치 스크립트
echo ===========================================

:: 가상환경 생성
echo 1. 가상환경 생성 중...
python -m venv venv
if errorlevel 1 (
    echo 오류: 가상환경 생성에 실패했습니다.
    pause
    exit /b 1
)

:: 가상환경 활성화
echo 2. 가상환경 활성화 중...
call venv\Scripts\activate.bat

:: 필요한 패키지 설치
echo 3. 패키지 설치 중...
pip install --upgrade pip
pip install -r requirements.txt

:: 시스템 의존성 확인
echo 4. 시스템 의존성 확인 중...

:: Ollama 설치 확인
ollama --version >nul 2>&1
if errorlevel 1 (
    echo [주의] Ollama가 설치되지 않았거나 PATH에 없습니다.
    echo   - https://ollama.com/download 에서 다운로드하여 설치하세요.
    echo   - 설치 후 다음 명령어로 필요한 모델을 받아두세요:
    echo     ollama pull llama3
) else (
    echo [확인] Ollama가 설치되어 있습니다.
)

:: FFmpeg 설치 확인
ffmpeg -version >nul 2>&1
if errorlevel 1 (
    echo [주의] FFmpeg가 설치되지 않았거나 PATH에 없습니다.
    echo   - Whisper가 오디오 파일을 처리하려면 FFmpeg가 필요합니다.
    echo   - https://ffmpeg.org/download.html 에서 다운로드하여 설치하고,
    echo     실행 파일 경로를 시스템 PATH 환경 변수에 추가해주세요.
    echo   - (권장) Chocolatey 사용 시: choco install ffmpeg
) else (
    echo [확인] FFmpeg가 설치되어 있습니다.
)


echo.
echo 설치 과정이 완료되었습니다!
echo.
echo --- 사용법 ---
echo 1. 가상환경 활성화: venv\Scripts\activate.bat
echo 2. 워크플로우 실행: python run_workflow.py
pause
