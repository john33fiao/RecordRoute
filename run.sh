#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
LAUNCHER_PATH="${SCRIPT_DIR}/package/RecordRoute"

if [[ ! -x "${LAUNCHER_PATH}" ]]; then
  printf 'RecordRoute package launcher is missing. Run ./setup.sh first.\n' >&2
  exit 1
fi

exec "${LAUNCHER_PATH}"
