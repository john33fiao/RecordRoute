#!/bin/bash
# RecordRoute Backend Build Script
# Note: The Python backend is now LEGACY. The main backend is Rust (recordroute-rs/).
# This script is kept for backwards compatibility but is no longer required.

echo "======================================"
echo "RecordRoute Backend Build Script"
echo "======================================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Note: The Python backend is legacy code.${NC}"
echo -e "${YELLOW}The main backend is now Rust (recordroute-rs/).${NC}"
echo -e "${YELLOW}Skipping Python backend build.${NC}"
echo ""

# Check if sttEngine directory exists (legacy Python backend)
if [ ! -d "sttEngine" ] && [ ! -d "legacy/python-backend" ]; then
    echo -e "${GREEN}✓ No Python backend found - this is expected.${NC}"
    echo -e "${GREEN}The Rust backend will be built instead.${NC}"
    exit 0
fi

echo -e "${YELLOW}Warning: Legacy Python backend detected.${NC}"
echo "If you need to build it, please:"
echo "  1. Create a virtual environment: python -m venv venv"
echo "  2. Activate it: source venv/bin/activate (Linux/macOS) or venv\\Scripts\\activate (Windows)"
echo "  3. Install dependencies: pip install -r legacy/python-backend/requirements.txt"
echo "  4. Ensure sttEngine/ is in the root directory"
echo ""
echo -e "${GREEN}Skipping Python backend build - use Rust backend instead.${NC}"
exit 0

# Legacy code below (not executed)
set -e  # Exit on error

# Check if virtual environment is activated
if [ -z "$VIRTUAL_ENV" ]; then
    echo -e "${YELLOW}Warning: Virtual environment not detected${NC}"
    echo "Attempting to activate virtual environment..."

    if [ -f "venv/bin/activate" ]; then
        source venv/bin/activate
        echo -e "${GREEN}✓ Virtual environment activated${NC}"
    elif [ -f "venv/Scripts/activate" ]; then
        source venv/Scripts/activate
        echo -e "${GREEN}✓ Virtual environment activated${NC}"
    else
        echo -e "${RED}✗ Virtual environment not found${NC}"
        echo "Please create one with: python -m venv venv"
        exit 1
    fi
fi

# Check if PyInstaller is installed
if ! python -c "import PyInstaller" 2>/dev/null; then
    echo -e "${YELLOW}PyInstaller not found. Installing...${NC}"
    pip install pyinstaller
fi

echo ""
echo "[Step 1/4] Cleaning previous build..."
rm -rf build dist bin/RecordRouteAPI bin/RecordRouteAPI.exe
echo -e "${GREEN}✓ Cleaned${NC}"

echo ""
echo "[Step 2/4] Building Python backend with PyInstaller..."
pyinstaller RecordRouteAPI.spec --clean

if [ $? -ne 0 ]; then
    echo -e "${RED}✗ PyInstaller build failed${NC}"
    exit 1
fi
echo -e "${GREEN}✓ PyInstaller build complete${NC}"

echo ""
echo "[Step 3/4] Copying executable to bin directory..."
mkdir -p bin

if [ "$(uname)" == "Darwin" ] || [ "$(expr substr $(uname -s) 1 5)" == "Linux" ]; then
    # macOS or Linux
    cp -r dist/RecordRouteAPI bin/
    echo -e "${GREEN}✓ Copied RecordRouteAPI to bin/${NC}"
elif [ "$(expr substr $(uname -s) 1 10)" == "MINGW32_NT" ] || [ "$(expr substr $(uname -s) 1 10)" == "MINGW64_NT" ]; then
    # Windows (Git Bash)
    cp -r dist/RecordRouteAPI bin/
    echo -e "${GREEN}✓ Copied RecordRouteAPI to bin/${NC}"
fi

echo ""
echo "[Step 4/5] Copying FFmpeg binary..."

# Detect platform
if [ "$(uname)" == "Darwin" ]; then
    FFMPEG_SRC="bin/ffmpeg/darwin/ffmpeg"
    FFMPEG_DST="bin/ffmpeg"
elif [ "$(expr substr $(uname -s) 1 5)" == "Linux" ]; then
    FFMPEG_SRC="bin/ffmpeg/linux/ffmpeg"
    FFMPEG_DST="bin/ffmpeg"
elif [ "$(expr substr $(uname -s) 1 10)" == "MINGW32_NT" ] || [ "$(expr substr $(uname -s) 1 10)" == "MINGW64_NT" ]; then
    FFMPEG_SRC="bin/ffmpeg/win32/ffmpeg.exe"
    FFMPEG_DST="bin/ffmpeg.exe"
fi

# Check if FFmpeg source exists
if [ -f "$FFMPEG_SRC" ]; then
    cp "$FFMPEG_SRC" "$FFMPEG_DST"
    # Ensure executable permission on Unix
    if [ "$(uname)" == "Darwin" ] || [ "$(expr substr $(uname -s) 1 5)" == "Linux" ]; then
        chmod +x "$FFMPEG_DST"
    fi
    echo -e "${GREEN}✓ Copied FFmpeg to bin/${NC}"
else
    echo -e "${YELLOW}⚠ FFmpeg binary not found at $FFMPEG_SRC${NC}"
    echo "  The build will continue, but you'll need to install FFmpeg separately."
    echo "  See bin/ffmpeg/README.md for instructions."
fi

echo ""
echo "[Step 5/5] Build summary..."
echo "------------------------------"
echo "Output directory: dist/RecordRouteAPI"
echo "Installed to: bin/RecordRouteAPI"

if [ -f "bin/RecordRouteAPI/RecordRouteAPI" ]; then
    SIZE=$(du -sh bin/RecordRouteAPI | cut -f1)
    echo "Build size: $SIZE"
    echo -e "${GREEN}✓ Build successful!${NC}"
elif [ -f "bin/RecordRouteAPI/RecordRouteAPI.exe" ]; then
    SIZE=$(du -sh bin/RecordRouteAPI | cut -f1)
    echo "Build size: $SIZE"
    echo -e "${GREEN}✓ Build successful!${NC}"
else
    echo -e "${RED}✗ Executable not found${NC}"
    exit 1
fi

echo ""
echo "======================================"
echo "Backend build complete!"
echo "======================================"
echo ""
echo "Next steps:"
echo "  1. Test the backend: bin/RecordRouteAPI/RecordRouteAPI"
echo "  2. Build Electron app: npm run build"
echo ""
