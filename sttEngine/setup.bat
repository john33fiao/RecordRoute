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

:: Ollama 설치 확인
echo 4. Ollama 설치 확인...
ollama --version >nul 2>&1
if errorlevel 1 (
    echo 주의: Ollama가 설치되지 않았습니다.
    echo Ollama를 https://ollama.ai 에서 다운로드하고 설치하세요.
    echo 설치 후 다음 명령어로 필요한 모델을 다운로드하세요:
    echo   ollama pull gpt-oss:20b
    echo   ollama pull llama3.2
)

echo 설치가 완료되었습니다!
echo.
echo 사용법:
echo   1. 가상환경 활성화: venv\Scripts\activate.bat
echo   2. 워크플로우 실행: python run_workflow.py
pause