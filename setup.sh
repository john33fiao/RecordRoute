#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rust_manifest="${script_dir}/rust/Cargo.toml"
frontend_dir="${script_dir}/frontend"
frontend_package_json="${frontend_dir}/package.json"
frontend_lockfile="${frontend_dir}/package-lock.json"

if [[ -f "${frontend_package_json}" ]]; then
  if ! command -v npm >/dev/null 2>&1; then
    printf 'frontend/package.json found, but npm is not installed or not on PATH.\n' >&2
    exit 1
  fi

  if [[ -f "${frontend_lockfile}" ]]; then
    printf 'Installing frontend dependencies with npm ci...\n'
    npm ci --prefix "${frontend_dir}"
  else
    printf 'Installing frontend dependencies with npm install...\n'
    npm install --prefix "${frontend_dir}"
  fi
else
  printf 'No frontend/package.json found, skipping frontend dependency install.\n'
fi

printf 'Building ffmpeg...\n'
bash "${script_dir}/scripts/build_ffmpeg.sh"

printf 'Building whisper...\n'
bash "${script_dir}/scripts/build_whisper.sh"

printf 'Building llama...\n'
bash "${script_dir}/scripts/build_llama.sh"

printf 'Building rust (release)...\n'
cargo build --manifest-path "${rust_manifest}" --release

printf 'Preparing llama model...\n'
cargo run --manifest-path "${rust_manifest}" --release -- prepare-llama-model
