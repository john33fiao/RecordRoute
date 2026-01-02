# RecordRoute Codebase Guide for LLM Agents

This document provides context and instructions for LLM agents working on the `RecordRoute` codebase.

## Project Overview
RecordRoute is a monorepo application for converting audio/video/PDF to text and summarizing them into meeting notes.
It features a **Rust backend** for high-performance processing and an **Electron + Vanilla JS frontend** for the user interface.

## Tech Stack
- **Backend**: Rust (`recordroute-rs`)
  - Web Server: Axum
  - STT: `whisper.cpp` (via Rust bindings)
  - LLM/Embedding: `llama.cpp` (via HTTP server/bindings)
  - Search: Vector search (built-in/sled)
- **Frontend**: Vanilla HTML/CSS/JS (`frontend`)
- **Desktop App**: Electron (`electron`)
- **Build System**: npm workspaces, bash/batch scripts
- **AI Models**: GGUF format (Whisper, Llama 3, Gemma, etc.)

## Directory Structure
- `root`: NPM workspace root. Contains global build scripts (`tools/scripts`).
- `electron/`: Electron main process code.
- `frontend/`: Web UI code. Served by Axum or loaded by Electron.
- `recordroute-rs/`: core backend logic.
  - `crates/`: Workspace crates (server, stt, llm, vector, common).
- `models/`: Directory for GGUF models (ignored by git).
- `third-party/`: Submodules (llama.cpp).

## Key Workflows

### 1. Setup & Installation
```bash
# Root directory - Install Node deps
npm install

# Root directory - Install Electron/Builder deps
npm run install-deps
```

### 2. Running Locally (Development)
**Option A: Rust Server Only (Headless/Web Browser)**
```bash
cd recordroute-rs
cargo run --release
# Access at http://localhost:8080
```
*Note: Requires `models/` to be populated.*

**Option B: Electron App**
```bash
# Root directory
npm start
```

### 3. Building
- **Rust Backend**: `cargo build --release` (in `recordroute-rs`) or `npm run build:rust` (in root)
- **Electron App**: `npm run build` (builds for current OS) or `npm run build:win` / `npm run build:mac`.

## Critical Context & Conventions

### Monorepo Management
- Do **NOT** run `npm install` inside `electron/` or `frontend/` manually. Always run from root.
- The root `package.json` manages workspaces.

### Path Handling
- Relative paths in the Rust backend are usually relative to the **project root** (where `.env` resides), not the crate root.
- The application automatically detects the project root by looking for `.git` or specific markers.

### AI Models (`models/`)
- Expects GGUF format models.
- Common files: `ggml-base.bin` (Whisper), `Llama-3.2-3B-Instruct.Q4_K_M.gguf` (LLM).
- These are NOT checked into git. Agents should assume they exist in a configured environment or explain how to get them if missing.

### Environment config (`.env`)
- Located at project root.
- Controls server ports, model paths, and feature flags.
- See `recordroute-rs/CONFIGURATION.md` for details.

### Rust Architecture
- **Multi-crate**: Logic is split into `stt`, `llm`, `vector`, `server`.
- **Axum**: Used for the HTTP API.
- **Tokio**: Async runtime.

## Common Tasks for Agents

### "Fix build errors"
- Check if it's a Node dependency issue -> Run `npm install` at root.
- Check if it's a Rust binding issue -> Ensure `cmake` and `clang` are available (needed for `whisper.cpp`).
- `npm error Missing script: "build:win64"` -> Use `npm run build:win`.

### "Add a new API endpoint"
1. Edit `recordroute-rs/crates/server/src/routes/mod.rs` (or appropriate submodule).
2. Define the handler function.
3. Register the route in the router.
4. Update `recordroute-rs/API.md`.

### "Update UI"
- Modify `frontend/upload.html`, `frontend/upload.css`, or `frontend/upload.js`.
- No build step required for frontend logic alone (hot-reload supported in browser if server restarts/serves updated files, or Electron reload).