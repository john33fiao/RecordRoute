#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
source_dir="${repo_root}/llama.cpp"

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
build_root="${repo_root}/.build/llama/${target}"
build_dir="${build_root}/build"
runtime_bin="${build_root}/bin"
llama_bin="${runtime_bin}/llama-cli"
built_llama_bin="${build_dir}/bin/llama-cli"

link_llama_cli() {
  mkdir -p "${runtime_bin}"
  ln -sf "${built_llama_bin}" "${llama_bin}"
}

if [[ ! -d "${source_dir}" ]]; then
  printf 'llama.cpp source directory not found: %s\n' "${source_dir}" >&2
  exit 1
fi

if [[ -x "${llama_bin}" ]]; then
  printf 'llama_cli=%s\n' "${llama_bin}"
  exit 0
fi

if [[ -x "${built_llama_bin}" ]]; then
  link_llama_cli
  printf 'llama_cli=%s\n' "${llama_bin}"
  exit 0
fi

mkdir -p "${build_dir}" "${runtime_bin}"

jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '4')"

cmake -S "${source_dir}" -B "${build_dir}" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_RUNTIME_OUTPUT_DIRECTORY="${runtime_bin}" \
  -DBUILD_SHARED_LIBS=OFF \
  -DLLAMA_BUILD_COMMON=ON \
  -DLLAMA_BUILD_TOOLS=ON \
  -DLLAMA_BUILD_TESTS=OFF \
  -DLLAMA_BUILD_SERVER=OFF \
  -DLLAMA_BUILD_EXAMPLES=OFF

cmake --build "${build_dir}" --target llama-cli -j"${jobs}"

if [[ -x "${built_llama_bin}" ]]; then
  link_llama_cli
fi

if [[ ! -x "${llama_bin}" ]]; then
  printf 'llama-cli binary not found after build: %s\n' "${built_llama_bin}" >&2
  exit 1
fi

printf 'llama_cli=%s\n' "${llama_bin}"
