#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
source_dir="${repo_root}/whisper.cpp"

os_name="$(uname -s)"
case "${os_name}" in
  Darwin) platform_os="macos" ;;
  Linux) platform_os="linux" ;;
  *) platform_os="$(printf '%s' "${os_name}" | tr '[:upper:]' '[:lower:]')" ;;
esac

arch_name="$(uname -m)"
case "${arch_name}" in
  arm64) platform_arch="aarch64" ;;
  amd64) platform_arch="x86_64" ;;
  *) platform_arch="${arch_name}" ;;
esac

target="${platform_os}-${platform_arch}"
build_root="${repo_root}/.build/whisper/${target}"
build_dir="${build_root}/build"
runtime_bin="${build_root}/bin"
whisper_bin="${runtime_bin}/whisper-cli"

if [[ ! -d "${source_dir}" ]]; then
  printf 'whisper.cpp source directory not found: %s\n' "${source_dir}" >&2
  exit 1
fi

if [[ -x "${whisper_bin}" ]]; then
  printf 'whisper_cli=%s\n' "${whisper_bin}"
  exit 0
fi

mkdir -p "${build_dir}" "${runtime_bin}"

jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '4')"

cmake -S "${source_dir}" -B "${build_dir}" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_RUNTIME_OUTPUT_DIRECTORY="${runtime_bin}" \
  -DBUILD_SHARED_LIBS=OFF \
  -DWHISPER_BUILD_TESTS=OFF \
  -DWHISPER_BUILD_SERVER=OFF \
  -DWHISPER_BUILD_EXAMPLES=ON

cmake --build "${build_dir}" --target whisper-cli -j"${jobs}"

printf 'whisper_cli=%s\n' "${whisper_bin}"
