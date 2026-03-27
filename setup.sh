#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rust_manifest="${script_dir}/rust/Cargo.toml"
package_dir="${script_dir}/package"
staging_dir="${script_dir}/.package-staging"
next_dir="${script_dir}/.package-next"

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

  git -C "${script_dir}" submodule update --init --recursive
}

copy_if_exists() {
  local src="$1"
  local dst="$2"
  if [[ -e "${src}" ]]; then
    cp -a "${src}" "${dst}"
  fi
}

ensure_file_exists() {
  local path="$1"
  local description="$2"
  if [[ ! -f "${path}" ]]; then
    printf 'missing %s: %s\n' "${description}" "${path}" >&2
    exit 1
  fi
}

ensure_bundled_sources

bash "${script_dir}/scripts/build_ffmpeg.sh"
bash "${script_dir}/scripts/build_whisper.sh"
bash "${script_dir}/scripts/build_llama.sh"

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

target_dir="${platform_os}-${platform_arch}"
ensure_file_exists "${script_dir}/.build/ffmpeg/${target_dir}/install/bin/ffmpeg" "ffmpeg binary"
ensure_file_exists "${script_dir}/.build/ffmpeg/${target_dir}/install/bin/ffprobe" "ffprobe binary"
ensure_file_exists "${script_dir}/.build/whisper/${target_dir}/bin/whisper-cli" "whisper-cli binary"
ensure_file_exists "${script_dir}/.build/llama/${target_dir}/bin/llama-cli" "llama-cli binary"
ensure_file_exists "${script_dir}/.build/llama/${target_dir}/bin/llama-embedding" "llama-embedding binary"

cargo build --manifest-path "${rust_manifest}" --release --bin recordroute --bin recordroute_server --bin recordroute_rust

rm -rf "${staging_dir}" "${next_dir}"
mkdir -p "${staging_dir}"

cp "${script_dir}/rust/target/release/recordroute" "${staging_dir}/RecordRoute"
cp "${script_dir}/rust/target/release/recordroute_server" "${staging_dir}/RecordRouteServer"
copy_if_exists "${script_dir}/.build" "${staging_dir}/.build"
copy_if_exists "${script_dir}/models" "${staging_dir}/models"
touch "${staging_dir}/.recordroute-runtime-root"

mkdir -p "${next_dir}"
cp -a "${staging_dir}/." "${next_dir}/"

for preserve in db models logs; do
  if [[ -d "${package_dir}/${preserve}" ]]; then
    rm -rf "${next_dir:?}/${preserve}"
    cp -a "${package_dir}/${preserve}" "${next_dir}/${preserve}"
  fi
done

if [[ -f "${package_dir}/.env" ]]; then
  cp -a "${package_dir}/.env" "${next_dir}/.env"
elif [[ -f "${script_dir}/.env" ]]; then
  cp -a "${script_dir}/.env" "${next_dir}/.env"
elif [[ -f "${script_dir}/.env.example" ]]; then
  cp -a "${script_dir}/.env.example" "${next_dir}/.env"
fi

mkdir -p "${next_dir}/db" "${next_dir}/logs"
rm -rf "${package_dir}"
mv "${next_dir}" "${package_dir}"

RECORDROUTE_RUNTIME_ROOT="${package_dir}" "${script_dir}/rust/target/release/recordroute_rust" prepare-models

echo "Package ready: ${package_dir}/RecordRoute"
