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
WEB_SERVER="$SCRIPT_DIR/server.py"

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

# 웹서버 실행
echo "가상환경의 파이썬으로 웹서버를 실행합니다..."
echo "서버 URL: http://localhost:8080"
echo "(웹브라우저에서 http://localhost:8080 에 접속하세요)"
echo

"$VENV_PYTHON" "$WEB_SERVER"

echo
echo "서버가 종료되었습니다."
