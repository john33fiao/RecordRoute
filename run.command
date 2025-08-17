#!/bin/bash

# 이 스크립트가 있는 디렉토리 기준으로 경로 설정
SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)

# .env 파일이 있으면 환경변수 로드
if [ -f "$SCRIPT_DIR/.env" ]; then
  echo ".env 파일에서 환경변수를 로드합니다."
  export $(grep -v '^#' "$SCRIPT_DIR/.env" | xargs)
  echo "[DEBUG] PYANNOTE_TOKEN이 로드되었습니다."
fi

# 가상환경의 Python 실행 파일 경로
VENV_PYTHON="$SCRIPT_DIR/venv/bin/python"

# 웹서버 스크립트 경로
WEB_SERVER="$SCRIPT_DIR/sttEngine/server.py"

# 가상환경 존재 확인
if [ ! -f "$VENV_PYTHON" ]; then
    echo "오류: 가상환경(venv)을 찾을 수 없습니다."
    echo "먼저 설치 스크립트를 실행하여 가상환경을 설정하세요."
    exit 1
fi

# 웹서버 스크립트 존재 확인
if [ ! -f "$WEB_SERVER" ]; then
    echo "오류: 웹서버 스크립트(server.py)를 찾을 수 없습니다."
    exit 1
fi

# Ollama 서버 상태 확인 및 시작
echo "Ollama 서버 상태를 확인합니다..."
if ! curl -s http://localhost:11434/api/version > /dev/null 2>&1; then
    echo "Ollama 서버가 실행되지 않았습니다. 자동으로 시작합니다..."
    if command -v ollama > /dev/null 2>&1; then
        # 백그라운드에서 ollama serve 실행
        nohup ollama serve > /dev/null 2>&1 &
        OLLAMA_PID=$!
        echo "Ollama 서버를 시작했습니다. (PID: $OLLAMA_PID)"
        
        # 서버 시작을 위해 잠시 대기
        echo "서버 시작을 기다리는 중..."
        sleep 3
        
        # 서버 시작 확인
        if curl -s http://localhost:11434/api/version > /dev/null 2>&1; then
            echo "Ollama 서버가 성공적으로 시작되었습니다."
        else
            echo "경고: Ollama 서버 시작을 확인할 수 없습니다. 수동으로 'ollama serve'를 실행해주세요."
        fi
    else
        echo "경고: ollama 명령어를 찾을 수 없습니다. Ollama가 설치되어 있는지 확인하세요."
        echo "수동으로 'ollama serve' 명령어를 실행한 후 이 스크립트를 다시 실행하세요."
    fi
else
    echo "Ollama 서버가 이미 실행 중입니다."
fi

# 웹서버 실행
echo "가상환경의 파이썬으로 웹서버를 실행합니다..."
echo "서버 URL: http://localhost:8080"
echo "(웹브라우저에서 http://localhost:8080 에 접속하세요)"
echo

cd "$SCRIPT_DIR"
"$VENV_PYTHON" -m sttEngine.server

echo
echo "서버가 종료되었습니다."
