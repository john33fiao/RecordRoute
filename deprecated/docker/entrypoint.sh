#!/usr/bin/env bash
set -euo pipefail

cd /app

export HOST="${HOST:-0.0.0.0}"
export PORT="${PORT:-8080}"
export WORKFLOW_DEVICE="${WORKFLOW_DEVICE:-auto}"

if [ "${SKIP_TORCH_SETUP:-0}" != "1" ]; then
  if command -v nvidia-smi >/dev/null 2>&1; then
    echo "[docker] NVIDIA GPU 감지: CUDA PyTorch 설치/업데이트를 시도합니다."
    pip install --upgrade --index-url https://download.pytorch.org/whl/cu124 torch torchvision torchaudio || true
  else
    echo "[docker] NVIDIA GPU 미감지: 기본 PyTorch를 사용합니다."
  fi
fi

if [ "${START_OLLAMA:-0}" = "1" ]; then
  if command -v ollama >/dev/null 2>&1; then
    echo "[docker] ollama serve를 백그라운드로 시작합니다."
    nohup ollama serve >/tmp/ollama.log 2>&1 &
  else
    echo "[docker] ollama 바이너리가 없어 START_OLLAMA=1을 무시합니다."
  fi
fi

python -c "from sttEngine.http_api.app import main; main(host='${HOST}', port=int('${PORT}'))"
