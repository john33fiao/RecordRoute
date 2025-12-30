#!/bin/bash
# RecordRoute 실행 스크립트 (macOS/Linux)
# 이 스크립트는 RecordRoute를 시작합니다

set -e  # 오류 발생 시 스크립트 중단

echo "RecordRoute를 시작합니다..."

# 스크립트 디렉토리로 이동
cd "$(dirname "$0")"

# Node.js와 npm이 설치되어 있는지 확인
if ! command -v npm &> /dev/null; then
    echo "오류: npm을 찾을 수 없습니다. Node.js가 설치되어 있는지 확인하세요."
    exit 1
fi

# node_modules가 없으면 설치
if [ ! -d "node_modules" ]; then
    echo "node_modules가 없습니다. npm install을 실행합니다..."
    npm install
fi

# Python venv가 없으면 생성
if [ ! -d "venv" ]; then
    echo "Python 가상환경이 없습니다. 가상환경을 생성합니다..."

    # Python3 확인
    if command -v python3 &> /dev/null; then
        PYTHON_CMD=python3
    elif command -v python &> /dev/null; then
        PYTHON_CMD=python
    else
        echo "오류: Python을 찾을 수 없습니다."
        exit 1
    fi

    $PYTHON_CMD -m venv venv

    echo "Python 패키지를 설치합니다..."
    source venv/bin/activate
    pip install -r sttEngine/requirements.txt
fi

# Electron 앱 시작
echo "RecordRoute를 실행합니다..."
npm start
