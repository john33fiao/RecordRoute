#!/bin/bash

# 이 스크립트가 있는 디렉토리 기준으로 경로 설정
SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)

# .env 파일이 있으면 환경변수 로드
if [ -f "$SCRIPT_DIR/.env" ]; then
  echo ".env 파일에서 환경변수를 로드합니다."
  export $(grep -v '^#' "$SCRIPT_DIR/.env" | xargs)
  echo "[DEBUG] 로드된 토큰: $PYANNOTE_TOKEN"
fi

# 가상환경의 Python 실행 파일 경로
VENV_PYTHON="$SCRIPT_DIR/venv/bin/python"

# 워크플로우 스크립트 경로
WORKFLOW_SCRIPT="$SCRIPT_DIR/sttEngine/run_workflow.py"

# 가상환경 존재 확인
if [ ! -f "$VENV_PYTHON" ]; then
    echo "오류: 가상환경(venv)을 찾을 수 없습니다."
    echo "먼저 설치 스크립트를 실행하여 가상환경을 설정하세요."
    exit 1
fi

# 워크플로우 스크립트 존재 확인
if [ ! -f "$WORKFLOW_SCRIPT" ]; then
    echo "오류: 워크플로우 스크립트(sttEngine/run_workflow.py)를 찾을 수 없습니다."
    exit 1
fi

# 워크플로우 스크립트 실행
echo "가상환경의 파이썬으로 워크플로우를 실행합니다..."
echo "(스크립트 경로: $WORKFLOW_SCRIPT)"

# "$@"를 통해 이 스크립트에 전달된 모든 인자를 python 스크립트로 넘김
"$VENV_PYTHON" "$WORKFLOW_SCRIPT" "$@"
