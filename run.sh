#!/bin/bash

# RecordRoute 웹서버 실행 스크립트 (Linux/macOS)

# 이 스크립트가 있는 디렉토리 기준으로 경로 설정
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"

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
    echo "먼저 setup.command 또는 setup.sh 스크립트를 실행하여 가상환경을 설정하세요."
    exit 1
fi

# 웹서버 스크립트 존재 확인
if [ ! -f "$WEB_SERVER" ]; then
    echo "오류: 웹서버 스크립트(server.py)를 찾을 수 없습니다."
    exit 1
fi

# 의존성 확인 및 설치 (requirements.txt가 변경되었거나 새 패키지가 필요한 경우)
REQUIREMENTS_FILE="$SCRIPT_DIR/sttEngine/requirements.txt"
REQUIREMENTS_STATE_FILE="$SCRIPT_DIR/venv/.requirements_hash"

if [ -f "$REQUIREMENTS_FILE" ]; then
    echo "필요한 파이썬 패키지를 확인합니다..."

    REQ_HASH=$("$VENV_PYTHON" -c "import hashlib, pathlib, sys; print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())" "$REQUIREMENTS_FILE")
    INSTALLED_HASH=""

    if [ -f "$REQUIREMENTS_STATE_FILE" ]; then
        INSTALLED_HASH=$(cat "$REQUIREMENTS_STATE_FILE")
    fi

    if [ "$REQ_HASH" != "$INSTALLED_HASH" ]; then
        echo "의존성을 설치/업데이트합니다..."
        if "$VENV_PYTHON" -m pip install -r "$REQUIREMENTS_FILE"; then
            echo "$REQ_HASH" > "$REQUIREMENTS_STATE_FILE"
            echo "필요한 패키지가 준비되었습니다."
        else
            echo "경고: 의존성 설치에 실패했습니다. 설치 로그를 확인하고 다시 시도하세요."
        fi
    else
        echo "필요한 파이썬 패키지가 이미 설치되어 있습니다."
    fi
else
    echo "경고: requirements.txt 파일을 찾을 수 없습니다."
fi

# 루트 requirements.txt 확인 및 설치
ROOT_REQ_FILE="$SCRIPT_DIR/requirements.txt"
if [ -f "$ROOT_REQ_FILE" ]; then
    echo "루트 requirements.txt를 확인합니다..."
    if "$VENV_PYTHON" -m pip install -r "$ROOT_REQ_FILE" > /dev/null 2>&1; then
        echo "루트 의존성 확인 완료."
    else
        echo "경고: 루트 의존성 설치 중 오류가 발생했습니다."
    fi
fi

# PyTorch CUDA 버전 확인 및 설치 (선택적)
echo "PyTorch 상태를 확인합니다..."
TORCH_VER_FILE="$SCRIPT_DIR/venv/lib/python*/site-packages/torch/version.py"
TORCH_VER_FILE_EXPANDED=$(find "$SCRIPT_DIR/venv/lib" -name "version.py" -path "*/torch/*" 2>/dev/null | head -1)

if [ -z "$TORCH_VER_FILE_EXPANDED" ]; then
    echo "PyTorch가 설치되지 않았습니다. CUDA 빌드를 설치합니다 (cu124)..."
    "$VENV_PYTHON" -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu124 torch torchvision torchaudio 2>/dev/null || \
    "$VENV_PYTHON" -m pip install --upgrade torch torchvision torchaudio
else
    if grep -q "+cpu'" "$TORCH_VER_FILE_EXPANDED" 2>/dev/null; then
        echo "CPU 빌드 PyTorch 감지 → CUDA 빌드로 교체합니다 (cu124)..."
        "$VENV_PYTHON" -m pip uninstall -y torch torchvision torchaudio > /dev/null 2>&1
        "$VENV_PYTHON" -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu124 torch torchvision torchaudio 2>/dev/null || \
        "$VENV_PYTHON" -m pip install --upgrade torch torchvision torchaudio
    else
        echo "PyTorch CUDA 빌드 또는 호환 빌드가 감지되었습니다."
    fi
fi

# PyTorch 버전 정보 출력
echo "PyTorch 상태 확인:"
"$VENV_PYTHON" -c "import torch; print('Torch:', torch.__version__); print('CUDA available:', torch.cuda.is_available())" 2>/dev/null || echo "(PyTorch 정보를 가져올 수 없습니다)"

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

# Cloudflare Tunnel 상태 확인 및 시작
if [ "${TUNNEL_ENABLED}" = "true" ]; then
    echo "Cloudflare Tunnel이 활성화되어 있습니다. 상태를 확인합니다..."

    # cloudflared 설치 확인
    if ! command -v cloudflared > /dev/null 2>&1; then
        echo "경고: cloudflared 명령어를 찾을 수 없습니다."
        echo "Cloudflare Tunnel을 사용하려면 cloudflared를 설치해야 합니다."
        echo "설치 방법: https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/installation/"
        echo ""
    else
        # 터널 토큰 확인
        if [ -z "${CLOUDFLARE_TUNNEL_TOKEN}" ]; then
            echo "경고: CLOUDFLARE_TUNNEL_TOKEN이 설정되지 않았습니다."
            echo ".env 파일에 CLOUDFLARE_TUNNEL_TOKEN을 설정해주세요."
            echo ""
        else
            # 이미 실행 중인 cloudflared 프로세스 확인
            if pgrep -x "cloudflared" > /dev/null; then
                echo "Cloudflare Tunnel이 이미 실행 중입니다."
            else
                echo "Cloudflare Tunnel을 시작합니다..."

                # 백그라운드에서 cloudflared 실행
                nohup cloudflared tunnel --config "$SCRIPT_DIR/.cloudflared/config.yml" run --token "$CLOUDFLARE_TUNNEL_TOKEN" > "$SCRIPT_DIR/.cloudflared/tunnel.log" 2>&1 &
                TUNNEL_PID=$!
                echo "Cloudflare Tunnel을 시작했습니다. (PID: $TUNNEL_PID)"

                # 터널 시작을 위해 잠시 대기
                sleep 2

                # 터널 시작 확인
                if pgrep -x "cloudflared" > /dev/null; then
                    echo "✓ Cloudflare Tunnel이 성공적으로 시작되었습니다."
                    echo "  로그 위치: $SCRIPT_DIR/.cloudflared/tunnel.log"
                else
                    echo "경고: Cloudflare Tunnel 시작을 확인할 수 없습니다."
                    echo "로그를 확인하세요: $SCRIPT_DIR/.cloudflared/tunnel.log"
                fi
            fi
        fi
    fi
else
    echo "Cloudflare Tunnel이 비활성화되어 있습니다. (TUNNEL_ENABLED=false)"
fi
echo

# 웹서버 실행
echo "가상환경의 파이썬으로 웹서버를 실행합니다..."
echo "서버 URL: http://localhost:8080"
echo "(웹브라우저에서 http://localhost:8080 에 접속하세요)"
echo

cd "$SCRIPT_DIR"
"$VENV_PYTHON" -m sttEngine.server
EXIT_CODE=$?

echo
if [ $EXIT_CODE -eq 0 ]; then
    echo "서버가 정상적으로 종료되었습니다."
else
    echo "서버가 오류와 함께 종료되었습니다. (오류코드: $EXIT_CODE)"
    echo "오류 상세 내용을 확인하세요."
fi
