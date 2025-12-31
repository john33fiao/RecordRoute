#!/bin/bash
# RecordRoute Full Build Script
# Phase 3: Build both Python backend and Electron frontend

set -e  # Exit on error

echo "=========================================="
echo "RecordRoute Full Build Script"
echo "=========================================="
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Parse arguments
SKIP_BACKEND=false
SKIP_FRONTEND=false
TARGET=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-backend)
            SKIP_BACKEND=true
            shift
            ;;
        --skip-frontend)
            SKIP_FRONTEND=true
            shift
            ;;
        --target)
            TARGET="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--skip-backend] [--skip-frontend] [--target win|mac|linux]"
            exit 1
            ;;
    esac
done

# Build backend
if [ "$SKIP_BACKEND" = false ]; then
    echo ""
    echo "=========================================="
    echo "Step 1: Building Python Backend"
    echo "=========================================="
    echo ""

    if [ -f "tools/scripts/build-backend.sh" ]; then
        bash tools/scripts/build-backend.sh
    else
        echo -e "${YELLOW}Warning: tools/scripts/build-backend.sh not found, skipping backend build${NC}"
    fi
else
    echo ""
    echo "[Skipped] Python backend build"
fi

# Install Node.js dependencies if needed
if [ "$SKIP_FRONTEND" = false ]; then
    echo ""
    echo "=========================================="
    echo "Step 2: Installing Node.js Dependencies"
    echo "=========================================="
    echo ""

    if [ ! -d "node_modules" ]; then
        echo "Installing root dependencies..."
        npm install
        echo ""
        echo "Installing electron workspace dependencies..."
        npm install -w electron
        echo ""
        echo "Installing frontend workspace dependencies..."
        npm install -w frontend
        echo ""
        echo "Installing electron-builder dependencies..."
        npm run install-deps
        echo -e "${GREEN}✓ Dependencies installed${NC}"
    else
        echo "Node modules already installed"
    fi
fi

# Build Electron app
if [ "$SKIP_FRONTEND" = false ]; then
    echo ""
    echo "=========================================="
    echo "Step 3: Building Electron Application"
    echo "=========================================="
    echo ""

    if [ -z "$TARGET" ]; then
        # Auto-detect platform
        if [ "$(uname)" == "Darwin" ]; then
            TARGET="mac"
        elif [ "$(expr substr $(uname -s) 1 5)" == "Linux" ]; then
            TARGET="linux"
        elif [ "$(expr substr $(uname -s) 1 10)" == "MINGW32_NT" ] || [ "$(expr substr $(uname -s) 1 10)" == "MINGW64_NT" ]; then
            TARGET="win"
        else
            TARGET="linux"  # Default
        fi
    fi

    echo "Building for platform: $TARGET"

    case $TARGET in
        win|windows)
            npm run build:win
            ;;
        mac|macos|darwin)
            npm run build:mac
            ;;
        linux)
            npm run build:linux
            ;;
        *)
            echo "Building for all platforms..."
            npm run build
            ;;
    esac

    echo -e "${GREEN}✓ Electron build complete${NC}"
else
    echo ""
    echo "[Skipped] Electron application build"
fi

echo ""
echo "=========================================="
echo "Build Complete!"
echo "=========================================="
echo ""
echo "Output locations:"
echo "  - Python backend: bin/RecordRouteAPI/"
echo "  - Electron app: dist/"
echo ""
echo "To run in development mode: npm start"
echo ""
