#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
BINARY_PATH="${SCRIPT_DIR}/rust/target/release/recordroute_rust"

if [[ ! -x "${BINARY_PATH}" ]]; then
  printf 'RecordRoute runtime binary is missing. Run ./setup.sh first.\n' >&2
  exit 1
fi

printf 'RecordRoute server starting on http://127.0.0.1:38080/\n'
printf 'Web UI: http://127.0.0.1:38080/\n'

exec "${BINARY_PATH}" server
