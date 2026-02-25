#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cleanup() {
  local exit_code=$?
  if [[ -n "${API_PID:-}" ]] && kill -0 "$API_PID" >/dev/null 2>&1; then
    kill "$API_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "${FRONTEND_PID:-}" ]] && kill -0 "$FRONTEND_PID" >/dev/null 2>&1; then
    kill "$FRONTEND_PID" >/dev/null 2>&1 || true
  fi
  exit "$exit_code"
}
trap cleanup INT TERM EXIT

echo "[RecordRoute] Starting Rust API server..."
(
  cd "$ROOT_DIR"
  cargo run --bin recordroute-orchestrator
) &
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

