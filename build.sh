#!/bin/bash

# RecordRoute 전체 빌드 스크립트 (macOS/Linux)
# 이 스크립트는 프로젝트의 모든 컴포넌트를 순차적으로 빌드합니다.

set -e  # 오류 발생 시 스크립트 중단

echo "===================================="
echo "RecordRoute 전체 빌드 시작"
echo "===================================="
echo ""

# 빌드 시작 시간 기록
START_TIME=$(date +%s)

# 1. 의존성 설치
echo "[1/3] NPM 의존성 설치 중..."
echo "------------------------------------"
npm install
echo ""

# 2. Rust 프로젝트 빌드
echo "[2/3] Rust 프로젝트 빌드 중..."
echo "------------------------------------"
cd recordroute-rs
cargo build --release
cd ..
echo ""

# 3. Electron 앱 빌드
echo "[3/3] Electron 앱 빌드 중..."
echo "------------------------------------"
npm run build
echo ""

# 빌드 완료
END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))

echo "===================================="
echo "빌드 완료!"
echo "소요 시간: ${ELAPSED}초"
echo "===================================="
echo ""
echo "빌드된 파일 위치: dist/"
echo ""
