#!/bin/bash
# RecordRoute Initial Setup Script
# Run this script after cloning the repository

set -e  # Exit on error

echo "=========================================="
echo "RecordRoute Setup Script"
echo "=========================================="
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Check if running from project root
if [ ! -f "electron/package.json" ]; then
    echo -e "${RED}Error: Please run this script from the RecordRoute project root directory${NC}"
    exit 1
fi

echo "This script will:"
echo "  1. Install Node.js dependencies"
echo "  2. Install electron-builder dependencies"
echo "  3. Download Whisper models (optional)"
echo ""

# Install Node.js dependencies
echo "=========================================="
echo "Step 1: Installing Node.js Dependencies"
echo "=========================================="
echo ""

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

echo -e "${GREEN}✓ Node.js dependencies installed${NC}"

# Ask about Whisper model
echo ""
echo "=========================================="
echo "Step 2: Whisper Model Setup (Optional)"
echo "=========================================="
echo ""
echo "Would you like to download the Whisper model now?"
echo "This will download whisper.cpp and the base model (~200MB)"
echo ""
read -p "Download Whisper model? (y/n): " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    if [ -f "tools/scripts/download-whisper-model.sh" ]; then
        bash tools/scripts/download-whisper-model.sh
    else
        echo -e "${YELLOW}Warning: download-whisper-model.sh not found${NC}"
    fi
else
    echo "Skipping Whisper model download"
    echo "You can download it later by running: bash tools/scripts/download-whisper-model.sh"
fi

echo ""
echo "=========================================="
echo "Setup Complete!"
echo "=========================================="
echo ""
echo "Next steps:"
echo ""
echo "  1. Development mode:"
echo "     npm start"
echo ""
echo "  2. Build for production:"
echo "     bash tools/scripts/build-all.sh"
echo ""
echo "  3. Download Whisper model (if not done):"
echo "     bash tools/scripts/download-whisper-model.sh"
echo ""
echo -e "${GREEN}Happy coding!${NC}"
echo ""
