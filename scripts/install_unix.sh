#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

CHECK_ONLY=false
MODEL_POLICY="prompt" # prompt|pull|cancel

usage() {
  cat <<'USAGE'
Usage: scripts/install_unix.sh [options]

Options:
  --check      Validate env/model/prerequisites only (no install/build)
  --yes-pull   Auto-pull missing models (non-interactive friendly)
  --no-pull    Fail immediately when model is missing
  -h, --help   Show help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --check)
      CHECK_ONLY=true
      ;;
    --yes-pull)
      MODEL_POLICY="pull"
      ;;
    --no-pull)
      MODEL_POLICY="cancel"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[ERROR] Unknown option: $1"
      usage
      exit 1
      ;;
  esac
  shift
done

echo "[RecordRoute] Unix install started."

require_command() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "[ERROR] Required command not found: $cmd"
    return 1
  fi
}

require_env() {
  local var_name="$1"
  local label="$2"
  if [[ -z "${!var_name:-}" ]]; then
    echo "[ERROR] ${label} env (${var_name}) is not set."
    return 1
  fi
}

pull_model() {
  local model_var="$1"
  local model_dir="$2"
  local model_repo_var="$3"
  local model_file="${!model_var}"
  local model_repo="${!model_repo_var:-}"

  if [[ -z "$model_repo" ]]; then
    echo "[ERROR] Pull requested but ${model_repo_var} is not set."
    echo "        Set repo id (e.g. org/repo) and rerun."
    return 1
  fi

  if ! command -v huggingface-cli >/dev/null 2>&1; then
    echo "[ERROR] huggingface-cli is required to pull models."
    echo "        Install: pip install -U huggingface_hub"
    return 1
  fi

  echo "[INFO] Pulling ${model_file} from ${model_repo} ..."
  huggingface-cli download "$model_repo" "$model_file" \
    --local-dir "$ROOT_DIR/$model_dir" \
    --local-dir-use-symlinks False
}

resolve_model_path() {
  local model_var="$1"
  local model_dir="$2"
  local model_value="${!model_var}"

  if [[ -f "$model_value" ]]; then
    printf '%s' "$model_value"
    return 0
  fi

  printf '%s' "$ROOT_DIR/$model_dir/$model_value"
}

ensure_model() {
  local model_var="$1"
  local model_dir="$2"
  local model_repo_var="$3"
  local model_value="${!model_var}"
  local model_path
  model_path="$(resolve_model_path "$model_var" "$model_dir")"

  if [[ -f "$model_path" ]]; then
    echo "[OK] ${model_var} -> ${model_path}"
    return 0
  fi

  echo "[WARN] Model not found for ${model_var} (${model_value})."
  echo "       Expected path: ${model_path}"

  local action="$MODEL_POLICY"
  if [[ "$action" == "prompt" ]]; then
    if [[ -t 0 ]]; then
      local choice
      read -r -p "Choose action: [C]ancel install / [P]ull model: " choice
      if [[ "${choice^^}" == "P" ]]; then
        action="pull"
      else
        action="cancel"
      fi
    else
      action="cancel"
      echo "[WARN] Non-interactive shell detected; defaulting to cancel."
      echo "       Use --yes-pull to allow automatic model pull."
    fi
  fi

  if [[ "$action" == "pull" ]]; then
    pull_model "$model_var" "$model_dir" "$model_repo_var" || return 1
    model_path="$(resolve_model_path "$model_var" "$model_dir")"
    if [[ -f "$model_path" ]]; then
      echo "[OK] Pulled ${model_var} -> ${model_path}"
      return 0
    fi

    echo "[ERROR] Model still missing after pull attempt: ${model_path}"
    return 1
  fi

  echo "[ERROR] Install canceled because model file is missing."
  return 1
}

require_command npm
require_command cargo

require_env RECORDROUTE_DEFAULT_STT_MODEL "STT default model"
require_env RECORDROUTE_DEFAULT_SUMMARIZE_MODEL "Summarize default model"
require_env RECORDROUTE_DEFAULT_EMBED_MODEL "Embed default model"

ensure_model RECORDROUTE_DEFAULT_STT_MODEL "models/stt" RECORDROUTE_STT_MODEL_REPO
ensure_model RECORDROUTE_DEFAULT_SUMMARIZE_MODEL "models/text" RECORDROUTE_SUMMARIZE_MODEL_REPO
ensure_model RECORDROUTE_DEFAULT_EMBED_MODEL "models/embed" RECORDROUTE_EMBED_MODEL_REPO

if [[ "$CHECK_ONLY" == true ]]; then
  echo "[RecordRoute] Check mode passed (no install/build executed)."
  exit 0
fi

echo "[1/3] Installing frontend dependencies..."
npm --prefix frontend install

echo "[2/3] Building frontend..."
npm --prefix frontend run build

echo "[3/3] Building Rust orchestrator..."
cargo build --release

echo "[RecordRoute] Install completed successfully."
