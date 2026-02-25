#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_MODE="dev" # dev|release

usage() {
  cat <<'USAGE'
Usage: scripts/run_unix.sh [options]

Options:
  --release    Run compiled release binary instead of cargo run
  -h, --help   Show help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release)
      RUN_MODE="release"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[ERROR] Unknown option: $1"
      usage
      exit 1
      ;;
  esac
  shift
done

require_command() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "[ERROR] Required command not found: $cmd"
    return 1
  fi
}

require_command npm
if [[ "$RUN_MODE" == "dev" ]]; then
  require_command cargo
fi

cleanup() {
  local exit_code=$?
  trap - INT TERM EXIT
  if [[ -n "${API_PID:-}" ]] && kill -0 "$API_PID" >/dev/null 2>&1; then
    kill "$API_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "${FRONTEND_PID:-}" ]] && kill -0 "$FRONTEND_PID" >/dev/null 2>&1; then
    kill "$FRONTEND_PID" >/dev/null 2>&1 || true
  fi
  wait >/dev/null 2>&1 || true
  exit "$exit_code"
}
trap cleanup INT TERM EXIT

if [[ "$RUN_MODE" == "release" ]]; then
  RELEASE_BIN="$ROOT_DIR/target/release/recordroute-orchestrator"
  if [[ ! -x "$RELEASE_BIN" ]]; then
    echo "[ERROR] Release binary not found: $RELEASE_BIN"
    echo "        Run scripts/install_unix.sh first or cargo build --release."
    exit 1
  fi

  echo "[RecordRoute] Starting Rust API server (release binary)..."
  (
    cd "$ROOT_DIR"
    "$RELEASE_BIN"
  ) &
else
  echo "[RecordRoute] Starting Rust API server (cargo run)..."
  (
    cd "$ROOT_DIR"
    cargo run --bin recordroute-orchestrator
  ) &
fi
API_PID=$!

echo "[RecordRoute] Starting frontend dev server..."
(
  cd "$ROOT_DIR/frontend"
  npm run dev
) &
FRONTEND_PID=$!

echo "[RecordRoute] Rust API PID: $API_PID"
echo "[RecordRoute] Frontend PID: $FRONTEND_PID"
echo "[RecordRoute] Press Ctrl+C to stop both services."

wait -n "$API_PID" "$FRONTEND_PID"
