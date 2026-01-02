#!/bin/bash
# RecordRoute Rust 자동 빌드 스크립트
# GPU를 자동으로 감지하고 최적의 feature로 빌드합니다
# GPU 빌드 실패 시 자동으로 CPU로 폴백합니다

# 색상 정의
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}🦀 RecordRoute Rust Build Script${NC}"
echo ""

# 플랫폼 감지
OS="$(uname -s)"
TRY_GPU=false
GPU_TYPE=""

case "${OS}" in
    Linux*)
        echo -e "${GREEN}📍 Platform: Linux${NC}"

        # NVIDIA GPU 확인
        if command -v nvidia-smi &> /dev/null && nvidia-smi &> /dev/null; then
            echo -e "${GREEN}✓ NVIDIA GPU detected${NC}"
            TRY_GPU=true
            GPU_TYPE="cuda"
        else
            echo -e "${YELLOW}  ℹ No NVIDIA GPU detected${NC}"
        fi
        ;;

    Darwin*)
        echo -e "${GREEN}📍 Platform: macOS${NC}"

        # Apple Silicon 확인
        ARCH="$(uname -m)"
        if [[ "$ARCH" == "arm64" ]]; then
            echo -e "${GREEN}✓ Apple Silicon detected${NC}"
            TRY_GPU=true
            GPU_TYPE="metal"
        else
            echo -e "${YELLOW}  ℹ Intel Mac detected${NC}"
        fi
        ;;

    MINGW*|MSYS*|CYGWIN*)
        echo -e "${GREEN}📍 Platform: Windows${NC}"

        # NVIDIA GPU 확인
        if command -v nvidia-smi &> /dev/null && nvidia-smi &> /dev/null; then
            echo -e "${GREEN}✓ NVIDIA GPU detected${NC}"
            TRY_GPU=true
            GPU_TYPE="cuda"
        else
            echo -e "${YELLOW}  ℹ No NVIDIA GPU detected${NC}"
        fi
        ;;

    *)
        echo -e "${YELLOW}  ⚠ Unknown platform: ${OS}${NC}"
        ;;
esac

cd "$(dirname "$0")/../../recordroute-rs"

# GPU 빌드 시도
if [[ "$TRY_GPU" == true ]]; then
    echo ""
    echo -e "${BLUE}🔨 Attempting GPU build with ${GPU_TYPE}...${NC}"
    echo -e "${YELLOW}  ℹ If this fails, the build will automatically fall back to CPU${NC}"
    echo ""

    if cargo build --release --features "$GPU_TYPE" 2>&1; then
        echo ""
        echo -e "${GREEN}✅ Build complete with GPU acceleration!${NC}"
        echo ""
        echo -e "${BLUE}ℹ Info:${NC}"
        echo -e "  • GPU acceleration: ${GREEN}Enabled${NC} (${GPU_TYPE})"
        echo -e "  • Binary: recordroute-rs/target/release/recordroute"
        echo ""
        exit 0
    else
        echo ""
        echo -e "${YELLOW}⚠ GPU build failed${NC}"
        echo -e "${YELLOW}  Common reasons:${NC}"
        if [[ "$GPU_TYPE" == "cuda" ]]; then
            echo -e "${YELLOW}  - CUDA Toolkit not installed${NC}"
            echo -e "${YELLOW}  - Visual Studio CUDA integration not installed (Windows)${NC}"
            echo -e "${YELLOW}  - nvcc compiler not in PATH${NC}"
        elif [[ "$GPU_TYPE" == "metal" ]]; then
            echo -e "${YELLOW}  - Xcode Command Line Tools not installed${NC}"
        fi
        echo ""
        echo -e "${BLUE}↻ Falling back to CPU build...${NC}"
        echo ""
    fi
fi

# CPU 빌드 (기본 또는 폴백)
echo -e "${BLUE}🔨 Building with CPU...${NC}"
echo ""

if cargo build --release; then
    echo ""
    echo -e "${GREEN}✅ Build complete!${NC}"
    echo ""
    echo -e "${BLUE}ℹ Info:${NC}"
    echo -e "  • GPU acceleration: ${YELLOW}Disabled${NC} (CPU only)"
    if [[ "$TRY_GPU" == true ]]; then
        echo -e "  • ${YELLOW}To enable GPU:${NC}"
        if [[ "$GPU_TYPE" == "cuda" ]]; then
            echo -e "    1. Install CUDA Toolkit (https://developer.nvidia.com/cuda-downloads)"
            echo -e "    2. Install Visual Studio CUDA integration (Windows)"
            echo -e "    3. Run: cd recordroute-rs && cargo build --release --features cuda"
        elif [[ "$GPU_TYPE" == "metal" ]]; then
            echo -e "    1. Install Xcode Command Line Tools: xcode-select --install"
            echo -e "    2. Run: cd recordroute-rs && cargo build --release --features metal"
        fi
    fi
    echo -e "  • Binary: recordroute-rs/target/release/recordroute"
    echo ""
    exit 0
else
    echo ""
    echo -e "${RED}❌ Build failed${NC}"
    exit 1
fi
