#!/usr/bin/env bash
set -euo pipefail
shopt -s extglob

SCRIPT_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
PACKAGE_DIR="${SCRIPT_DIR}/package"
LAUNCHER_PATH="${SCRIPT_DIR}/package/RecordRoute"
STARTUP_ENV_FILE="${PACKAGE_DIR}/.env"
STARTUP_VPN_TIMEOUT_SECS=30
STARTUP_VPN_POLL_INTERVAL_SECS=1

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

trim_whitespace() {
  local value="$1"
  value="${value##+([[:space:]])}"
  value="${value%%+([[:space:]])}"
  printf '%s\n' "${value}"
}

strip_matching_quotes() {
  local value="$1"

  if (( ${#value} >= 2 )); then
    if [[ "${value:0:1}" == '"' && "${value: -1}" == '"' ]]; then
      value="${value:1:${#value}-2}"
    elif [[ "${value:0:1}" == "'" && "${value: -1}" == "'" ]]; then
      value="${value:1:${#value}-2}"
    fi
  fi

  printf '%s\n' "${value}"
}

bool_env_is_true() {
  local value
  value="$(trim_whitespace "${1:-}")"
  value="${value,,}"

  case "${value}" in
    1 | true | yes | on) return 0 ;;
    *) return 1 ;;
  esac
}

is_startup_env_key() {
  case "$1" in
    RECORDROUTE_STARTUP_VPN_ENABLED|RECORDROUTE_STARTUP_VPN_NAME|RECORDROUTE_STARTUP_SMB_URL|RECORDROUTE_STARTUP_SMB_MOUNT_PATH|RECORDROUTE_AUDIO_ROOT)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

load_startup_env_from_package() {
  local line trimmed key value

  if [[ ! -f "${STARTUP_ENV_FILE}" ]]; then
    return 0
  fi

  while IFS= read -r line || [[ -n "${line}" ]]; do
    line="${line%$'\r'}"
    trimmed="$(trim_whitespace "${line}")"

    if [[ -z "${trimmed}" || "${trimmed:0:1}" == "#" ]]; then
      continue
    fi

    if [[ "${trimmed}" == export[[:space:]]* ]]; then
      trimmed="$(trim_whitespace "${trimmed#export}")"
    fi

    if [[ "${trimmed}" != *=* ]]; then
      continue
    fi

    key="$(trim_whitespace "${trimmed%%=*}")"
    if ! is_startup_env_key "${key}"; then
      continue
    fi

    if [[ -n "${!key+x}" ]]; then
      continue
    fi

    value="$(trim_whitespace "${trimmed#*=}")"
    value="$(strip_matching_quotes "${value}")"
    printf -v "${key}" '%s' "${value}"
    export "${key}"
  done < "${STARTUP_ENV_FILE}"
}

normalize_path() {
  local path="$1"

  if [[ -z "${path}" ]]; then
    printf '\n'
    return 0
  fi

  while [[ "${path}" != "/" && "${path}" == */ ]]; do
    path="${path%/}"
  done

  printf '%s\n' "${path}"
}

resolve_runtime_path() {
  local path="$1"

  if [[ "${path}" == /* ]]; then
    normalize_path "${path}"
  else
    normalize_path "${PACKAGE_DIR}/${path}"
  fi
}

path_is_within() {
  local candidate base
  candidate="$(normalize_path "$1")"
  base="$(normalize_path "$2")"

  case "${candidate}" in
    "${base}" | "${base}"/*) return 0 ;;
    *) return 1 ;;
  esac
}

startup_fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

print_vpn_service_list() {
  printf 'Available VPN services:\n' >&2
  scutil --nc list >&2 || true
}

vpn_status_output() {
  local service="$1"
  scutil --nc status "${service}" 2>&1 || true
}

vpn_is_connected() {
  local status_output="$1"
  [[ "${status_output}" == Connected* ]]
}

vpn_has_known_service() {
  local status_output="$1"
  [[ "${status_output}" != No\ service* ]]
}

smb_mount_spec_from_url() {
  local smb_url="$1"

  if [[ "${smb_url}" != smb://* ]]; then
    return 1
  fi

  printf '//%s\n' "${smb_url#smb://}"
}

mount_line_for_path() {
  local mount_path="$1"
  mount | grep -F " on ${mount_path} (" | head -n 1 || true
}

mount_line_is_smbfs() {
  local mount_line="$1"
  [[ "${mount_line}" == *"(smbfs"* ]]
}

mount_source_from_line() {
  local mount_line="$1"
  printf '%s\n' "${mount_line%% on *}"
}

ensure_vpn_connected() {
  local service="$1"
  local status_output
  local start_output
  local elapsed

  status_output="$(vpn_status_output "${service}")"
  if ! vpn_has_known_service "${status_output}"; then
    printf 'Configured VPN service was not found: %s\n' "${service}" >&2
    print_vpn_service_list
    exit 1
  fi

  if vpn_is_connected "${status_output}"; then
    printf 'VPN already connected: %s\n' "${service}"
    return 0
  fi

  printf 'Connecting VPN: %s\n' "${service}"
  start_output="$(scutil --nc start "${service}" 2>&1)" || {
    printf 'Failed to start VPN service: %s\n' "${service}" >&2
    if [[ -n "${start_output}" ]]; then
      printf '%s\n' "${start_output}" >&2
    fi
    print_vpn_service_list
    exit 1
  }

  for (( elapsed = 0; elapsed < STARTUP_VPN_TIMEOUT_SECS; elapsed += STARTUP_VPN_POLL_INTERVAL_SECS )); do
    sleep "${STARTUP_VPN_POLL_INTERVAL_SECS}"
    status_output="$(vpn_status_output "${service}")"
    if vpn_is_connected "${status_output}"; then
      printf 'VPN connected: %s\n' "${service}"
      return 0
    fi
  done

  printf 'Timed out waiting for VPN connection: %s\n' "${service}" >&2
  printf '%s\n' "${status_output}" >&2
  print_vpn_service_list
  exit 1
}

ensure_smb_mount_ready() {
  local smb_url="${RECORDROUTE_STARTUP_SMB_URL:-}"
  local mount_path="${RECORDROUTE_STARTUP_SMB_MOUNT_PATH:-}"
  local smb_mount_spec normalized_mount_path resolved_audio_root mount_line mounted_source mount_output

  if [[ -z "${smb_url}" && -z "${mount_path}" ]]; then
    return 0
  fi

  if [[ -z "${smb_url}" || -z "${mount_path}" ]]; then
    startup_fail "RECORDROUTE_STARTUP_SMB_URL and RECORDROUTE_STARTUP_SMB_MOUNT_PATH must be configured together."
  fi

  if [[ -z "${RECORDROUTE_AUDIO_ROOT:-}" ]]; then
    startup_fail "RECORDROUTE_AUDIO_ROOT must be explicitly set when SMB startup preflight is enabled."
  fi

  if [[ "${mount_path}" != /* ]]; then
    startup_fail "RECORDROUTE_STARTUP_SMB_MOUNT_PATH must be an absolute path."
  fi

  smb_mount_spec="$(smb_mount_spec_from_url "${smb_url}")" || {
    startup_fail "RECORDROUTE_STARTUP_SMB_URL must use the smb:// scheme."
  }
  normalized_mount_path="$(normalize_path "${mount_path}")"
  resolved_audio_root="$(resolve_runtime_path "${RECORDROUTE_AUDIO_ROOT}")"

  if ! path_is_within "${resolved_audio_root}" "${normalized_mount_path}"; then
    startup_fail "RECORDROUTE_AUDIO_ROOT (${resolved_audio_root}) must be inside the SMB mount path (${normalized_mount_path})."
  fi

  if [[ -e "${normalized_mount_path}" && ! -d "${normalized_mount_path}" ]]; then
    startup_fail "Configured SMB mount path is not a directory: ${normalized_mount_path}"
  fi

  if [[ ! -e "${normalized_mount_path}" ]]; then
    mkdir -p "${normalized_mount_path}" || {
      startup_fail "Failed to create SMB mount path: ${normalized_mount_path}"
    }
  fi

  mount_line="$(mount_line_for_path "${normalized_mount_path}")"
  if [[ -n "${mount_line}" ]]; then
    if ! mount_line_is_smbfs "${mount_line}"; then
      startup_fail "Configured SMB mount path is already occupied by a non-SMB mount: ${mount_line}"
    fi

    mounted_source="$(mount_source_from_line "${mount_line}")"
    if [[ "${mounted_source}" != "${smb_mount_spec}" ]]; then
      startup_fail "Configured SMB mount path already points to a different share (${mounted_source}); expected ${smb_mount_spec}."
    fi

    printf 'SMB share already mounted: %s -> %s\n' "${smb_url}" "${normalized_mount_path}"
    return 0
  fi

  printf 'Mounting SMB share: %s -> %s\n' "${smb_url}" "${normalized_mount_path}"
  mount_output="$(/sbin/mount_smbfs -N -o nobrowse,soft "${smb_mount_spec}" "${normalized_mount_path}" 2>&1)" || {
    printf 'Failed to mount SMB share: %s -> %s\n' "${smb_url}" "${normalized_mount_path}" >&2
    if [[ -n "${mount_output}" ]]; then
      printf '%s\n' "${mount_output}" >&2
    fi
    exit 1
  }

  mount_line="$(mount_line_for_path "${normalized_mount_path}")"
  if [[ -z "${mount_line}" ]]; then
    startup_fail "SMB mount command completed but the mount is not visible at ${normalized_mount_path}."
  fi

  if ! mount_line_is_smbfs "${mount_line}"; then
    startup_fail "Expected an SMB mount at ${normalized_mount_path}, but found: ${mount_line}"
  fi

  mounted_source="$(mount_source_from_line "${mount_line}")"
  if [[ "${mounted_source}" != "${smb_mount_spec}" ]]; then
    startup_fail "Mounted SMB share does not match the configured share. Found ${mounted_source}, expected ${smb_mount_spec}."
  fi
}

run_startup_preflight() {
  load_startup_env_from_package

  if ! bool_env_is_true "${RECORDROUTE_STARTUP_VPN_ENABLED:-}"; then
    return 0
  fi

  if [[ "$(uname -s)" != "Darwin" ]]; then
    startup_fail "RECORDROUTE_STARTUP_VPN_ENABLED=true is only supported by run.sh on macOS."
  fi

  if [[ -z "${RECORDROUTE_STARTUP_VPN_NAME:-}" ]]; then
    startup_fail "RECORDROUTE_STARTUP_VPN_NAME is required when RECORDROUTE_STARTUP_VPN_ENABLED=true."
  fi

  ensure_vpn_connected "${RECORDROUTE_STARTUP_VPN_NAME}"
  ensure_smb_mount_ready
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

run_startup_preflight

exec "${LAUNCHER_PATH}"
