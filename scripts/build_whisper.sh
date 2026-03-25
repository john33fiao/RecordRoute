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
runtime_bin="${build_root}/bin"
whisper_bin="${runtime_bin}/whisper-cli"
build_stamp="${build_root}/build-flags.txt"

link_whisper_cli() {
  local built_whisper_bin="$1"
  mkdir -p "${runtime_bin}"
  ln -sf "${built_whisper_bin}" "${whisper_bin}"
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

built_whisper_bin_for_backend() {
  local backend="$1"
  printf '%s/bin/whisper-cli' "$(backend_build_dir "${backend}")"
}

is_known_backend() {
  case "$1" in
    cpu | metal) return 0 ;;
    *) return 1 ;;
  esac
}

build_backend() {
  local backend="$1"
  local build_dir
  local backend_runtime_bin
  local built_whisper_bin
  local -a cmake_args

  build_dir="$(backend_build_dir "${backend}")"
  backend_runtime_bin="${build_dir}/bin"
  built_whisper_bin="${backend_runtime_bin}/whisper-cli"

  mkdir -p "${build_dir}" "${backend_runtime_bin}" "${runtime_bin}"

  cmake_args=(
    -DCMAKE_BUILD_TYPE=Release
    "-DCMAKE_RUNTIME_OUTPUT_DIRECTORY=${backend_runtime_bin}"
    -DBUILD_SHARED_LIBS=OFF
    -DWHISPER_BUILD_TESTS=OFF
    -DWHISPER_BUILD_SERVER=OFF
    -DWHISPER_BUILD_EXAMPLES=ON
  )

  case "${backend}" in
    metal)
      cmake_args+=(-DGGML_METAL=ON -DGGML_CUDA=OFF)
      ;;
    cpu)
      cmake_args+=(-DGGML_METAL=OFF -DGGML_CUDA=OFF)
      ;;
    *)
      printf 'unsupported whisper backend: %s\n' "${backend}" >&2
      return 1
      ;;
  esac

  if cmake -S "${source_dir}" -B "${build_dir}" "${cmake_args[@]}" \
    && cmake --build "${build_dir}" --target whisper-cli -j"${jobs}"; then
    if [[ -x "${built_whisper_bin}" ]]; then
      link_whisper_cli "${built_whisper_bin}"
      write_build_stamp "${backend}"
      printf 'whisper_cli=%s\n' "${whisper_bin}"
      return 0
    fi
  fi

  return 1
}

if [[ ! -d "${source_dir}" ]]; then
  printf 'whisper.cpp source directory not found: %s\n' "${source_dir}" >&2
  exit 1
fi

cached_backend="$(read_build_stamp GGML_BACKEND)"
if is_known_backend "${cached_backend}"; then
  if [[ -x "${whisper_bin}" ]]; then
    printf 'whisper_cli=%s\n' "${whisper_bin}"
    exit 0
  fi

  built_whisper_bin="$(built_whisper_bin_for_backend "${cached_backend}")"
  if [[ -x "${built_whisper_bin}" ]]; then
    link_whisper_cli "${built_whisper_bin}"
    printf 'whisper_cli=%s\n' "${whisper_bin}"
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

printf 'failed to build whisper-cli with supported backends for %s\n' "${platform_os}" >&2
exit 1
