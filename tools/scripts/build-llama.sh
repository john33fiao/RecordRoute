#!/bin/bash

# llama.cpp 빌드 스크립트
# 이 스크립트는 third-party/llama.cpp를 빌드하여 llama-server를 생성합니다.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
LLAMA_DIR="$PROJECT_ROOT/third-party/llama.cpp"
BUILD_DIR="$LLAMA_DIR/build"

echo "========================================="
echo "llama.cpp 빌드 시작"
echo "========================================="

# llama.cpp 디렉토리 존재 확인
if [ ! -d "$LLAMA_DIR" ]; then
    echo "Error: llama.cpp 서브모듈이 없습니다."
    echo "다음 명령어를 실행하세요:"
    echo "  git submodule update --init --recursive"
    exit 1
fi

cd "$LLAMA_DIR"

# 서브모듈 업데이트
echo "서브모듈 업데이트 중..."
git submodule update --init --recursive

# 빌드 디렉토리 생성
mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

# CMake 설정
echo "CMake 설정 중..."
cmake .. \
    -DCMAKE_BUILD_TYPE=Release \
    -DGGML_NATIVE=OFF \
    -DGGML_CUDA=OFF \
    -DLLAMA_BUILD_TESTS=OFF \
    -DLLAMA_BUILD_EXAMPLES=ON \
    -DLLAMA_BUILD_SERVER=ON

# 빌드
echo "빌드 중... (시간이 다소 걸릴 수 있습니다)"
cmake --build . --config Release -j$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)

echo ""
echo "========================================="
echo "llama.cpp 빌드 완료!"
echo "========================================="
echo ""
echo "llama-server 위치: $BUILD_DIR/bin/llama-server"
echo ""
echo "llama-server 실행 예시:"
echo "  $BUILD_DIR/bin/llama-server -m /path/to/model.gguf --host 127.0.0.1 --port 8081"
echo ""
