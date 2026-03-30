#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
PACKAGE_DIR="${SCRIPT_DIR}/package"
LAUNCHER_PATH="${SCRIPT_DIR}/package/RecordRoute"

if [[ ! -x "${LAUNCHER_PATH}" ]]; then
  printf 'RecordRoute package launcher is missing. Run ./setup.sh first.\n' >&2
  exit 1
fi

platform_target_dir() {
  local os_name platform_os arch_name platform_arch

  os_name="$(uname -s)"
  case "${os_name}" in
    Darwin) platform_os="macos" ;;
    Linux) platform_os="linux" ;;
    *) platform_os="$(printf '%s' "${os_name}" | tr '[:upper:]' '[:lower:]')" ;;
  esac

  arch_name="$(uname -m)"
  case "${arch_name}" in
    arm64 | aarch64) platform_arch="aarch64" ;;
    amd64 | x86_64) platform_arch="x86_64" ;;
    *) platform_arch="${arch_name}" ;;
  esac

  printf '%s-%s\n' "${platform_os}" "${platform_arch}"
}

file_mtime() {
  local path="$1"

  if stat -L -f '%m' "${path}" >/dev/null 2>&1; then
    stat -L -f '%m' "${path}"
  elif stat -f '%m' "${path}" >/dev/null 2>&1; then
    stat -f '%m' "${path}"
  elif stat -L -c '%Y' "${path}" >/dev/null 2>&1; then
    stat -L -c '%Y' "${path}"
  else
    stat -c '%Y' "${path}"
  fi
}

display_path() {
  local path="$1"

  if [[ "${path}" == "${SCRIPT_DIR}/"* ]]; then
    printf '%s\n' "${path#${SCRIPT_DIR}/}"
  else
    printf '%s\n' "${path}"
  fi
}

latest_input_mtime=0
latest_input_path=""
oldest_output_mtime=0
oldest_output_path=""
have_output=0
missing_output_path=""

consider_input_file() {
  local path="$1"
  local mtime

  if [[ ! -f "${path}" ]]; then
    return 0
  fi

  mtime="$(file_mtime "${path}")"
  if (( mtime > latest_input_mtime )); then
    latest_input_mtime="${mtime}"
    latest_input_path="${path}"
  fi
}

consider_input_tree() {
  local dir="$1"
  local path

  if [[ ! -d "${dir}" ]]; then
    return 0
  fi

  while IFS= read -r -d '' path; do
    consider_input_file "${path}"
  done < <(find "${dir}" -type f -print0)
}

consider_output_file() {
  local path="$1"
  local mtime

  if [[ ! -f "${path}" ]]; then
    missing_output_path="${path}"
    return 1
  fi

  mtime="$(file_mtime "${path}")"
  if (( have_output == 0 || mtime < oldest_output_mtime )); then
    oldest_output_mtime="${mtime}"
    oldest_output_path="${path}"
    have_output=1
  fi
}

target_dir="$(platform_target_dir)"
input_files=(
  "${SCRIPT_DIR}/rust/Cargo.toml"
  "${SCRIPT_DIR}/rust/Cargo.lock"
  "${SCRIPT_DIR}/setup.sh"
  "${SCRIPT_DIR}/scripts/build_ffmpeg.sh"
  "${SCRIPT_DIR}/scripts/build_whisper.sh"
  "${SCRIPT_DIR}/scripts/build_llama.sh"
  "${SCRIPT_DIR}/.build/ffmpeg/${target_dir}/install/bin/ffmpeg"
  "${SCRIPT_DIR}/.build/ffmpeg/${target_dir}/install/bin/ffprobe"
  "${SCRIPT_DIR}/.build/whisper/${target_dir}/bin/whisper-cli"
  "${SCRIPT_DIR}/.build/llama/${target_dir}/bin/llama-cli"
  "${SCRIPT_DIR}/.build/llama/${target_dir}/bin/llama-embedding"
)
output_files=(
  "${PACKAGE_DIR}/RecordRoute"
  "${PACKAGE_DIR}/RecordRouteServer"
  "${PACKAGE_DIR}/.build/ffmpeg/${target_dir}/install/bin/ffmpeg"
  "${PACKAGE_DIR}/.build/ffmpeg/${target_dir}/install/bin/ffprobe"
  "${PACKAGE_DIR}/.build/whisper/${target_dir}/bin/whisper-cli"
  "${PACKAGE_DIR}/.build/llama/${target_dir}/bin/llama-cli"
  "${PACKAGE_DIR}/.build/llama/${target_dir}/bin/llama-embedding"
)

for path in "${input_files[@]}"; do
  consider_input_file "${path}"
done
consider_input_tree "${SCRIPT_DIR}/rust/src"

for path in "${output_files[@]}"; do
  if ! consider_output_file "${path}"; then
    break
  fi
done

if [[ -n "${missing_output_path}" ]]; then
  printf 'RecordRoute package runtime is incomplete.\n' >&2
  printf 'Missing packaged artifact: %s\n' "$(display_path "${missing_output_path}")" >&2
  printf 'Run ./setup.sh and rerun ./run.sh.\n' >&2
  exit 1
fi

if [[ -n "${latest_input_path}" ]] && (( latest_input_mtime > oldest_output_mtime )); then
  printf 'RecordRoute package is stale. Repo inputs are newer than the packaged runtime.\n' >&2
  printf 'Latest changed input: %s\n' "$(display_path "${latest_input_path}")" >&2
  printf 'Oldest packaged artifact: %s\n' "$(display_path "${oldest_output_path}")" >&2
  printf 'Run ./setup.sh and rerun ./run.sh.\n' >&2
  exit 1
fi

exec "${LAUNCHER_PATH}"
