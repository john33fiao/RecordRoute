#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"

printf 'RecordRoute server starting on http://127.0.0.1:38080/\n'
printf 'Web UI: http://127.0.0.1:38080/\n'

cd "$SCRIPT_DIR/rust"
exec cargo run --release -- server
