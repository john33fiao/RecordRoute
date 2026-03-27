#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
source_dir="${repo_root}/modules/llama.cpp"

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
runtime_bin="${build_root}/bin"
llama_bin="${runtime_bin}/llama-cli"
llama_embedding_bin="${runtime_bin}/llama-embedding"
build_stamp="${build_root}/build-flags.txt"

link_llama_cli() {
  local built_llama_bin="$1"
  local built_llama_embedding_bin="$2"
  mkdir -p "${runtime_bin}"
  ln -sf "${built_llama_bin}" "${llama_bin}"
  ln -sf "${built_llama_embedding_bin}" "${llama_embedding_bin}"
}

read_build_stamp() {
  local key="$1"
  [[ -f "${build_stamp}" ]] || return 0
  sed -n "s/^${key}=//p" "${build_stamp}" | head -n 1
}

write_build_stamp() {
  local backend="$1"
  printf 'SCHEMA=2\nGGML_BACKEND=%s\n' "${backend}" > "${build_stamp}"
}

backend_build_dir() {
  local backend="$1"
  printf '%s/build/%s' "${build_root}" "${backend}"
}

built_llama_bin_for_backend() {
  local backend="$1"
  printf '%s/bin/llama-cli' "$(backend_build_dir "${backend}")"
}

built_llama_embedding_bin_for_backend() {
  local backend="$1"
  printf '%s/bin/llama-embedding' "$(backend_build_dir "${backend}")"
}

is_known_backend() {
  case "$1" in
    cpu | metal) return 0 ;;
    *) return 1 ;;
  esac
}

reset_stale_backend_build_dir() {
  local build_dir="$1"
  local backend="$2"
  local cache_path="${build_dir}/CMakeCache.txt"
  local configured_source
  local preserve_deps

  [[ -f "${cache_path}" ]] || return 0

  configured_source="$(sed -n 's/^CMAKE_HOME_DIRECTORY:INTERNAL=//p' "${cache_path}" | head -n 1)"
  [[ -z "${configured_source}" || "${configured_source}" == "${source_dir}" ]] && return 0

  printf 'Resetting stale llama build cache for %s\n' "${backend}"
  preserve_deps="${build_root}/_deps-preserve-${backend}"
  rm -rf "${preserve_deps}"
  if [[ -d "${build_dir}/_deps" ]]; then
    mv "${build_dir}/_deps" "${preserve_deps}"
  fi
  rm -rf "${build_dir}"
  mkdir -p "${build_dir}"
  if [[ -d "${preserve_deps}" ]]; then
    mv "${preserve_deps}" "${build_dir}/_deps"
  fi
}

build_backend() {
  local backend="$1"
  local build_dir
  local backend_runtime_bin
  local built_llama_bin
  local built_llama_embedding_bin
  local -a cmake_args

  build_dir="$(backend_build_dir "${backend}")"
  backend_runtime_bin="${build_dir}/bin"
  built_llama_bin="${backend_runtime_bin}/llama-cli"
  built_llama_embedding_bin="${backend_runtime_bin}/llama-embedding"

  reset_stale_backend_build_dir "${build_dir}" "${backend}"

  mkdir -p "${build_dir}" "${backend_runtime_bin}" "${runtime_bin}"

  cmake_args=(
    -DCMAKE_BUILD_TYPE=Release
    "-DCMAKE_RUNTIME_OUTPUT_DIRECTORY=${backend_runtime_bin}"
    -DBUILD_SHARED_LIBS=OFF
    -DLLAMA_BUILD_COMMON=ON
    -DLLAMA_BUILD_TOOLS=ON
    -DLLAMA_BUILD_TESTS=OFF
    -DLLAMA_BUILD_SERVER=ON
    -DLLAMA_BUILD_EXAMPLES=ON
  )

  case "${backend}" in
    metal)
      cmake_args+=(-DGGML_METAL=ON -DGGML_CUDA=OFF)
      ;;
    cpu)
      cmake_args+=(-DGGML_METAL=OFF -DGGML_CUDA=OFF)
      ;;
    *)
      printf 'unsupported llama backend: %s\n' "${backend}" >&2
      return 1
      ;;
  esac

  if cmake -S "${source_dir}" -B "${build_dir}" "${cmake_args[@]}" \
    && cmake --build "${build_dir}" --target llama-cli llama-embedding -j"${jobs}"; then
    if [[ -x "${built_llama_bin}" && -x "${built_llama_embedding_bin}" ]]; then
      link_llama_cli "${built_llama_bin}" "${built_llama_embedding_bin}"
      write_build_stamp "${backend}"
      printf 'llama_cli=%s\n' "${llama_bin}"
      printf 'llama_embedding=%s\n' "${llama_embedding_bin}"
      return 0
    fi
  fi

  return 1
}

if [[ ! -d "${source_dir}" ]]; then
  printf 'llama.cpp source directory not found: %s\n' "${source_dir}" >&2
  exit 1
fi

cached_backend="$(read_build_stamp GGML_BACKEND)"
if is_known_backend "${cached_backend}"; then
  if [[ -x "${llama_bin}" && -x "${llama_embedding_bin}" ]]; then
    printf 'llama_cli=%s\n' "${llama_bin}"
    printf 'llama_embedding=%s\n' "${llama_embedding_bin}"
    exit 0
  fi

  built_llama_bin="$(built_llama_bin_for_backend "${cached_backend}")"
  built_llama_embedding_bin="$(built_llama_embedding_bin_for_backend "${cached_backend}")"
  if [[ -x "${built_llama_bin}" && -x "${built_llama_embedding_bin}" ]]; then
    link_llama_cli "${built_llama_bin}" "${built_llama_embedding_bin}"
    printf 'llama_cli=%s\n' "${llama_bin}"
    printf 'llama_embedding=%s\n' "${llama_embedding_bin}"
    exit 0
  fi
fi

mkdir -p "${runtime_bin}"

jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '4')"

if [[ "${platform_os}" == "macos" ]]; then
  backends=(metal cpu)
else
  backends=(cpu)
fi

for backend in "${backends[@]}"; do
  if build_backend "${backend}"; then
    exit 0
  fi
done

printf 'failed to build llama-cli/llama-embedding with supported backends for %s\n' "${platform_os}" >&2
exit 1
