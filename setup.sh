#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rust_manifest="${script_dir}/rust/Cargo.toml"
frontend_dir="${script_dir}/frontend"
frontend_package_json="${frontend_dir}/package.json"
frontend_lockfile="${frontend_dir}/package-lock.json"

ensure_bundled_sources() {
  local required_modules=(
    "${script_dir}/modules/ffmpeg"
    "${script_dir}/modules/whisper.cpp"
    "${script_dir}/modules/llama.cpp"
  )
  local missing=0

  for module_path in "${required_modules[@]}"; do
    if [[ ! -d "${module_path}" ]]; then
      missing=1
      break
    fi
  done

  if [[ "${missing}" -eq 0 ]]; then
    return 0
  fi

  if ! command -v git >/dev/null 2>&1; then
    printf 'setup requires Git to initialize bundled sources under modules/. Install Git and rerun setup.\n' >&2
    exit 1
  fi

  printf 'Initializing bundled sources with git submodule update --init --recursive...\n'
  if ! git -C "${script_dir}" submodule update --init --recursive; then
    printf 'setup could not initialize bundled sources under modules/. Make sure this repository is a normal Git checkout, then rerun setup.\n' >&2
    exit 1
  fi

  for module_path in "${required_modules[@]}"; do
    if [[ ! -d "${module_path}" ]]; then
      printf 'setup could not find bundled sources after initialization: %s\n' "${module_path}" >&2
      exit 1
    fi
  done
}

ensure_bundled_sources

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

printf 'Preparing runtime models...\n'
cargo run --manifest-path "${rust_manifest}" --release -- prepare-models
