#!/bin/bash

# Whisper 모델 다운로드 스크립트
# 이 스크립트는 whisper.cpp에서 사용할 Whisper 모델을 다운로드합니다.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
MODELS_DIR="$PROJECT_ROOT/models"

echo "========================================="
echo "Whisper 모델 다운로드"
echo "========================================="
echo ""

# 모델 선택 옵션
echo "다운로드할 Whisper 모델을 선택하세요:"
echo ""
echo "1) ggml-tiny.bin     - 가장 빠름, 낮은 정확도 (~75MB)"
echo "2) ggml-base.bin     - 균형잡힌 성능 (권장, ~142MB)"
echo "3) ggml-small.bin    - 높은 정확도 (~466MB)"
echo "4) ggml-medium.bin   - 매우 높은 정확도 (~1.5GB)"
echo "5) ggml-large-v3.bin - 최고 정확도, 한국어 최적화 (~2.9GB)"
echo ""
read -p "선택 (1-5) [기본값: 2]: " choice
choice=${choice:-2}

case $choice in
    1)
        MODEL_NAME="ggml-tiny.bin"
        ;;
    2)
        MODEL_NAME="ggml-base.bin"
        ;;
    3)
        MODEL_NAME="ggml-small.bin"
        ;;
    4)
        MODEL_NAME="ggml-medium.bin"
        ;;
    5)
        MODEL_NAME="ggml-large-v3.bin"
        ;;
    *)
        echo "잘못된 선택입니다. 기본값(base)을 사용합니다."
        MODEL_NAME="ggml-base.bin"
        ;;
esac

# models 디렉토리 생성
echo ""
echo "models 디렉토리 생성 중..."
mkdir -p "$MODELS_DIR"

# 모델 다운로드 URL
MODEL_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/${MODEL_NAME}"
MODEL_PATH="$MODELS_DIR/$MODEL_NAME"

# 이미 존재하는지 확인
if [ -f "$MODEL_PATH" ]; then
    echo ""
    echo "경고: $MODEL_NAME 파일이 이미 존재합니다."
    read -p "덮어쓰시겠습니까? (y/N): " overwrite
    if [[ ! $overwrite =~ ^[Yy]$ ]]; then
        echo "다운로드를 취소합니다."
        exit 0
    fi
    rm -f "$MODEL_PATH"
fi

echo ""
echo "다운로드 중: $MODEL_NAME"
echo "URL: $MODEL_URL"
echo "저장 위치: $MODEL_PATH"
echo ""

# wget 또는 curl 사용
if command -v wget &> /dev/null; then
    wget --show-progress -O "$MODEL_PATH" "$MODEL_URL"
elif command -v curl &> /dev/null; then
    curl -L --progress-bar -o "$MODEL_PATH" "$MODEL_URL"
else
    echo "Error: wget 또는 curl이 설치되어 있지 않습니다."
    echo "다음 명령어로 수동으로 다운로드하세요:"
    echo "  curl -L -o \"$MODEL_PATH\" \"$MODEL_URL\""
    exit 1
fi

# 다운로드 성공 확인
if [ -f "$MODEL_PATH" ]; then
    FILE_SIZE=$(du -h "$MODEL_PATH" | cut -f1)
    echo ""
    echo "========================================="
    echo "다운로드 완료!"
    echo "========================================="
    echo ""
    echo "모델 파일: $MODEL_PATH"
    echo "파일 크기: $FILE_SIZE"
    echo ""
    echo "이제 RecordRoute를 실행할 수 있습니다:"
    echo "  cd $PROJECT_ROOT/recordroute-rs"
    echo "  cargo run --release"
    echo ""

    # .env 파일 확인 및 제안
    ENV_FILE="$PROJECT_ROOT/.env"
    if [ ! -f "$ENV_FILE" ]; then
        echo "팁: .env 파일을 생성하여 모델 경로를 설정할 수 있습니다:"
        echo "  echo 'WHISPER_MODEL=./models/$MODEL_NAME' > $ENV_FILE"
        echo ""
    fi
else
    echo ""
    echo "Error: 다운로드에 실패했습니다."
    exit 1
fi
