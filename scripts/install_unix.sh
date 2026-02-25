#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[RecordRoute] Unix install started."

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

ensure_model() {
  local model_var="$1"
  local model_dir="$2"
  local model_repo_var="$3"
  local model_value="${!model_var}"
  local model_path="$model_value"

  if [[ ! -f "$model_path" ]]; then
    model_path="$ROOT_DIR/$model_dir/$model_value"
  fi

  if [[ -f "$model_path" ]]; then
    echo "[OK] ${model_var} -> ${model_path}"
    return 0
  fi

  echo "[WARN] Model not found for ${model_var} (${model_value})."
  echo "       Expected path: ${model_path}"

  local choice
  read -r -p "Choose action: [C]ancel install / [P]ull model: " choice
  if [[ "${choice^^}" == "P" ]]; then
    pull_model "$model_var" "$model_dir" "$model_repo_var" || return 1

    model_path="$model_value"
    if [[ ! -f "$model_path" ]]; then
      model_path="$ROOT_DIR/$model_dir/$model_value"
    fi
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

require_env RECORDROUTE_DEFAULT_STT_MODEL "STT default model"
require_env RECORDROUTE_DEFAULT_SUMMARIZE_MODEL "Summarize default model"
require_env RECORDROUTE_DEFAULT_EMBED_MODEL "Embed default model"

ensure_model RECORDROUTE_DEFAULT_STT_MODEL "models/stt" RECORDROUTE_STT_MODEL_REPO
ensure_model RECORDROUTE_DEFAULT_SUMMARIZE_MODEL "models/text" RECORDROUTE_SUMMARIZE_MODEL_REPO
ensure_model RECORDROUTE_DEFAULT_EMBED_MODEL "models/embed" RECORDROUTE_EMBED_MODEL_REPO

echo "[1/3] Installing frontend dependencies..."
npm --prefix frontend install

echo "[2/3] Building frontend..."
npm --prefix frontend run build

echo "[3/3] Building Rust orchestrator..."
cargo build --release

echo "[RecordRoute] Install completed successfully."
