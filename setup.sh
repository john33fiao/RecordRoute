#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rust_manifest="${script_dir}/rust/Cargo.toml"

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
