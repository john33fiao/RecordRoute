#!/bin/bash

# RecordRoute Setup Script for Linux/macOS
# ==========================================

# Set directory to where the script is
cd "$(dirname "$0")" || exit
SCRIPT_DIR=$(pwd)

echo "================================"
echo "   RecordRoute Setup"
echo "================================"
echo

# Detect OS for platform-specific instructions
OS_TYPE=$(uname -s)
if [[ "$OS_TYPE" == "Darwin" ]]; then
    OS_NAME="macOS"
elif [[ "$OS_TYPE" == "Linux" ]]; then
    OS_NAME="Linux"
else
    OS_NAME="Unknown"
fi

echo "Detected OS: $OS_NAME"
echo

# 1. Check Python
echo "Step 0: Checking Python installation..."
if command -v python3 &>/dev/null; then
    PY_CMD=python3
elif command -v python &>/dev/null; then
    PY_CMD=python
else
    echo "Error: python3 is not installed."
    if [[ "$OS_NAME" == "macOS" ]]; then
        echo "Please install Python 3.8+ (brew install python)"
    else
        echo "Please install Python 3.8+ (apt-get install python3 python3-venv or yum install python3)"
    fi
    exit 1
fi

$PY_CMD --version
echo

# 2. Virtual Environment
echo "Step 1: Creating Virtual Environment..."

if [ -d "venv" ]; then
    read -p "Existing virtual environment found. Delete and recreate? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Removing existing venv..."
        rm -rf venv
        echo "Creating new venv..."
        $PY_CMD -m venv venv
    else
        echo "Using existing venv."
    fi
else
    echo "Creating new venv..."
    $PY_CMD -m venv venv
fi

if [ ! -d "venv" ]; then
    echo "Error: Failed to create virtual environment."
    exit 1
fi

# Verify venv was created successfully
if [ ! -f "venv/bin/python" ]; then
    echo "Error: Virtual environment python executable not found."
    exit 1
fi

# Define venv python
VENV_PYTHON="$SCRIPT_DIR/venv/bin/python"

echo "Virtual environment created successfully."
echo

# 3. Install Dependencies
echo "Step 2: Installing Dependencies..."
echo

# Upgrade pip first
echo "Upgrading pip..."
"$VENV_PYTHON" -m pip install --upgrade pip

if [ -f "sttEngine/requirements.txt" ]; then
    echo "Installing sttEngine dependencies..."
    if ! "$VENV_PYTHON" -m pip install -r sttEngine/requirements.txt; then
        echo "Warning: Some sttEngine dependencies failed to install."
    fi
else
    echo "Warning: sttEngine/requirements.txt not found."
fi

if [ -f "requirements.txt" ]; then
    echo "Installing root dependencies..."
    if ! "$VENV_PYTHON" -m pip install -r requirements.txt; then
        echo "Warning: Some root dependencies failed to install."
    fi
fi

echo

# 3.5 PyTorch Verification
echo "Step 2.5: Verifying PyTorch..."
if [[ "$OS_NAME" == "Linux" ]]; then
    # For Linux, attempt CUDA installation if not already present
    if ! "$VENV_PYTHON" -c "import torch; print(f'PyTorch: {torch.__version__}'); torch.cuda.is_available() and print('CUDA available')" 2>/dev/null | grep -q "CUDA available"; then
        echo "CUDA not available or PyTorch not installed with CUDA support."
        echo "Attempting to install PyTorch with CUDA support..."
        "$VENV_PYTHON" -m pip install --upgrade --index-url https://download.pytorch.org/whl/cu118 torch torchvision torchaudio
    fi
fi

"$VENV_PYTHON" -c "import torch; print(f'PyTorch: {torch.__version__}'); print(f'CUDA available: {torch.cuda.is_available() if hasattr(torch, \"cuda\") else False}')" 2>/dev/null || echo "Warning: PyTorch verification failed or not installed."
echo

# 4. Ollama Check
echo "Step 3: Checking Ollama..."
if command -v ollama &>/dev/null; then
    echo "Ollama is installed."
    echo "Checking models..."
    if ollama list 2>/dev/null | grep -q "gemma3:4b"; then
        echo "Model 'gemma3:4b' is already present."
    else
        echo "Pulling model 'gemma3:4b-it-qat'..."
        ollama pull gemma3:4b-it-qat
    fi
else
    echo "Ollama is NOT installed."
    if [[ "$OS_NAME" == "macOS" ]]; then
        echo "Please download from https://ollama.ai or run: brew install ollama"
    elif [[ "$OS_NAME" == "Linux" ]]; then
        echo "Please download from https://ollama.ai"
        echo "Or run: curl -fsSL https://ollama.ai/install.sh | sh"
    fi
    echo "After installation, run: ollama pull gemma3:4b-it-qat"
fi
echo

# 5. FFmpeg Check
echo "Step 4: Checking FFmpeg..."
if command -v ffmpeg &>/dev/null; then
    echo "FFmpeg is installed."
    ffmpeg -version | head -1
else
    echo "FFmpeg is NOT installed."
    if [[ "$OS_NAME" == "macOS" ]]; then
        echo "Please install via Homebrew: brew install ffmpeg"
    elif [[ "$OS_NAME" == "Linux" ]]; then
        echo "Please install via package manager:"
        echo "  Ubuntu/Debian: sudo apt-get install ffmpeg"
        echo "  CentOS/RHEL: sudo yum install ffmpeg"
        echo "  Arch: sudo pacman -S ffmpeg"
    fi
fi

echo
echo "================================"
echo "      Setup Complete!"
echo "================================"
echo "You can now run './run.sh' to start the server."
echo
