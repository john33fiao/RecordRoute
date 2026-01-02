#!/bin/bash

# RecordRoute Setup Script for macOS/Linux
# ==========================================

# Set directory to where the script is
cd "$(dirname "$0")" || exit
SCRIPT_DIR=$(pwd)

echo "================================"
echo "   RecordRoute Setup (macOS)"
echo "================================"
echo

# 1. Check Python
echo "Checking Python installation..."
if command -v python3 &>/dev/null; then
    PY_CMD=python3
else
    echo "Error: python3 is not installed."
    echo "Please install Python 3.8+ (brew install python)"
    exit 1
fi

$PY_CMD --version

# 2. Virtual Environment
echo
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

# 2.5 Define venv python
VENV_PYTHON="$SCRIPT_DIR/venv/bin/python"

# 3. Install Dependencies
echo
echo "Step 2: Installing Dependencies..."

# Upgrade pip first
"$VENV_PYTHON" -m pip install --upgrade pip

if [ -f "sttEngine/requirements.txt" ]; then
    echo "Installing sttEngine dependencies..."
    "$VENV_PYTHON" -m pip install -r sttEngine/requirements.txt
else
    echo "Warning: sttEngine/requirements.txt not found."
fi

if [ -f "requirements.txt" ]; then
    echo "Installing root dependencies..."
    # Note: If requirements.txt has --extra-index-url for CUDA, pip on Mac looks at it but 
    # falls back to PyPI if no mac wheels are found there. 
    "$VENV_PYTHON" -m pip install -r requirements.txt
fi

# 3.5 PyTorch Verification (Mac usually uses MPS or CPU)
echo
echo "Verifying PyTorch..."
"$VENV_PYTHON" -c "import torch; print(f'Torch: {torch.__version__}'); print(f'MPS available: {torch.backends.mps.is_available() if hasattr(torch.backends, \"mps\") else False}')"

# 4. Ollama Check
echo
echo "Step 3: Checking Ollama..."
if command -v ollama &>/dev/null; then
    echo "Ollama is installed."
    echo "Checking models..."
    if ollama list | grep -q "gemma3:4b"; then
        echo "Model 'gemma3:4b' is already present."
    else
        echo "Pulling model 'gemma3:4b-it-qat'..."
        ollama pull gemma3:4b-it-qat
    fi
else
    echo "Ollama is NOT installed."
    echo "Please download from https://ollama.ai"
    echo "After verify installation, run: ollama pull gemma3:4b-it-qat"
fi

# 5. FFmpeg Check
echo
echo "Step 4: Checking FFmpeg..."
if command -v ffmpeg &>/dev/null; then
    echo "FFmpeg is installed."
else
    echo "FFmpeg is NOT installed."
    echo "Please install via Homebrew: brew install ffmpeg"
fi

echo
echo "================================"
echo "      Setup Complete!"
echo "================================"
echo "You can now run './run.command' to start the server."
echo
