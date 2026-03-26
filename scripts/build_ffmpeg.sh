#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
source_dir="${repo_root}/modules/ffmpeg"

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
build_root="${repo_root}/.build/ffmpeg/${target}"
build_dir="${build_root}/build"
install_dir="${build_root}/install"
ffmpeg_bin="${install_dir}/bin/ffmpeg"
ffprobe_bin="${install_dir}/bin/ffprobe"

if [[ ! -d "${source_dir}" ]]; then
  printf 'ffmpeg source directory not found: %s\n' "${source_dir}" >&2
  exit 1
fi

if [[ -x "${ffmpeg_bin}" && -x "${ffprobe_bin}" ]]; then
  printf 'ffmpeg=%s\n' "${ffmpeg_bin}"
  printf 'ffprobe=%s\n' "${ffprobe_bin}"
  exit 0
fi

mkdir -p "${build_dir}" "${install_dir}"

jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '4')"

cd "${build_dir}"
"${source_dir}/configure" \
  --prefix="${install_dir}" \
  --disable-ffplay \
  --disable-doc \
  --disable-network \
  --disable-autodetect \
  --disable-debug

make -j"${jobs}" ffmpeg ffprobe
make install

printf 'ffmpeg=%s\n' "${ffmpeg_bin}"
printf 'ffprobe=%s\n' "${ffprobe_bin}"
