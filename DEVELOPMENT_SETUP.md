# RecordRoute Development Environment Setup

This guide will help you set up the RecordRoute development environment.

## Quick Start

```bash
# 1. Clone the repository
git clone https://github.com/john33fiao/RecordRoute.git
cd RecordRoute

# 2. Run the setup script
bash tools/scripts/setup.sh  # Linux/macOS
# or
tools\scripts\setup.bat      # Windows

# 3. Start development mode
npm start
```

## Architecture Overview

RecordRoute uses a **Rust-based backend** (not Python!) with an Electron desktop application:

- **Backend**: Rust (`recordroute-rs/`) - High-performance backend for STT, LLM, and vector search
- **Frontend**: Electron (`electron/`) + Web UI (`frontend/`) - Desktop application interface
- **Legacy**: Python backend (`legacy/python-backend/`) - Kept for reference only, not actively used

## Setup Details

### 1. System Requirements

- **Node.js 18+** and **npm** for the Electron app
- **Rust** (via rustup) for the backend
- **CMake** for building llama.cpp
- **FFmpeg** in system PATH for audio/video processing

### 2. Initial Setup

The setup script (`tools/scripts/setup.sh` or `.bat`) will:

1. Install all Node.js dependencies for the Electron app and frontend
2. Install electron-builder dependencies
3. Optionally download Whisper models

**Note**: The setup script uses npm workspaces, so all dependencies are installed from the root directory.

### 3. Building Components

#### Rust Backend (Main Backend)

```bash
# Build Rust backend
cd recordroute-rs
cargo build --release
cd ..
```

#### llama.cpp (Required for LLM Features)

```bash
# Build llama.cpp
npm run build:llama
# or
bash tools/scripts/build-llama.sh  # Linux/macOS
tools\scripts\build-llama.bat      # Windows
```

#### Electron Application

```bash
# Development mode (no build needed)
npm start

# Production build
npm run build        # Build for current platform
npm run build:mac    # Build for macOS
npm run build:win    # Build for Windows
npm run build:linux  # Build for Linux
```

### 4. Development Workflow

```bash
# Start the Electron app in development mode
npm start

# The app will automatically connect to the Rust backend
# Make sure the Rust backend is built and running if needed
```

## Common Issues

### Issue: "Cannot compute electron version"

**Cause**: Electron is not installed in the workspace.

**Solution**: Run the updated setup script which now correctly installs workspace dependencies:
```bash
bash tools/scripts/setup.sh
```

### Issue: "Virtual environment not found" when building

**Cause**: The build script is trying to build the legacy Python backend.

**Solution**: The Python backend is legacy code and no longer required. The build scripts have been updated to skip it gracefully. The main backend is Rust.

### Issue: npm install not working in workspaces

**Cause**: npm workspaces require all installations to be run from the root directory.

**Solution**: Always run `npm install` from the project root, not from individual workspace directories.

## Project Structure

```
RecordRoute/
├── electron/           # Electron app (workspace)
│   └── package.json    # Electron dependencies
├── frontend/           # Web UI (workspace)
│   └── package.json    # Frontend dependencies
├── recordroute-rs/     # Main Rust backend
│   ├── Cargo.toml      # Rust workspace
│   └── crates/         # Rust modules
├── third-party/
│   └── llama.cpp/      # LLM engine (submodule)
├── legacy/
│   └── python-backend/ # Old Python code (reference only)
└── tools/
    └── scripts/        # Build and setup scripts
```

## Scripts Reference

| Script | Description |
|--------|-------------|
| `npm start` | Start Electron app in development mode |
| `npm run build` | Build Electron app for current platform |
| `npm run build:rust` | Build Rust backend |
| `npm run build:llama` | Build llama.cpp |
| `npm run build:all` | Build llama.cpp and Rust backend |
| `bash tools/scripts/setup.sh` | Initial project setup |
| `bash tools/scripts/build-all.sh` | Full build (backend + frontend) |

## Need Help?

- Check the main [README.md](README.md) for more information
- See [API documentation](recordroute-rs/API.md) for backend API details
- See [Architecture docs](recordroute-rs/ARCHITECTURE.md) for system design

## Contributing

When contributing, please:

1. Run `npm start` to test your changes
2. Ensure the Rust backend builds successfully
3. Test on your target platform (Windows, macOS, or Linux)
4. Update documentation as needed
