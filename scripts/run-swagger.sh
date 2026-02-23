#!/usr/bin/env bash
set -euo pipefail
export RECORDROUTE_SWAGGER_HOST="${RECORDROUTE_SWAGGER_HOST:-0.0.0.0}"
export RECORDROUTE_SWAGGER_PORT="${RECORDROUTE_SWAGGER_PORT:-14000}"
export RECORDROUTE_SWAGGER_ROOT="${RECORDROUTE_SWAGGER_ROOT:-docs/swagger}"
cargo run --bin swagger_server
