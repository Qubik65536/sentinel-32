#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
MODEL_PATH=${S32_LLAMA_MODEL_PATH:-"$HOME/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf"}
MODEL_ID=${S32_LLAMA_MODEL_ID:-}
LLAMA_SERVER_BIN=${S32_LLAMA_SERVER_BIN:-llama-server}
CONFIG_PATH=${S32_AI_CONFIG_PATH:-"$PROJECT_ROOT/config/ai-llama-default.env"}

if [[ -f "$CONFIG_PATH" ]]; then
  while IFS='=' read -r name value; do
    case "$name" in
      S32_LLAMA_API_KEY)
        if [[ -z "${S32_LLAMA_API_KEY:-}" ]]; then S32_LLAMA_API_KEY=$value; fi
        ;;
      S32_LLAMA_MODEL_SHA256)
        if [[ -z "${S32_LLAMA_MODEL_SHA256:-}" ]]; then S32_LLAMA_MODEL_SHA256=$value; fi
        ;;
      S32_LLAMA_MODEL_ID)
        if [[ -z "$MODEL_ID" ]]; then MODEL_ID=$value; fi
        ;;
      S32_AI_CHECK_MODE|S32_AI_RULES_PATH|S32_LLAMA_BASE_URL|S32_AI_CHECK_TIMEOUT_MS|S32_AI_MAX_OUTPUT_TOKENS|S32_AI_MAX_SNAPSHOT_BYTES|S32_AI_MAX_OUTPUT_BYTES) ;;
      ''|'#'*) ;;
      *)
        echo "error: unsupported runtime-config key: $name" >&2
        exit 1
        ;;
    esac
  done <"$CONFIG_PATH"
fi

: "${S32_LLAMA_API_KEY:?set S32_LLAMA_API_KEY in ai-llama-default.env}"
MODEL_ID=${MODEL_ID:-qwen2.5-1.5b}

if [[ "$S32_LLAMA_API_KEY" == replace-locally ]]; then
  echo "error: replace the placeholder API key in ai-llama-default.env" >&2
  exit 1
fi

if [[ ! "$S32_LLAMA_API_KEY" =~ ^[A-Za-z0-9._~-]+$ ]]; then
  echo "error: S32_LLAMA_API_KEY contains characters unsupported by the env-file format" >&2
  exit 1
fi

if [[ ! -f "$MODEL_PATH" ]]; then
  echo "error: model is not a regular file: $MODEL_PATH" >&2
  exit 1
fi

actual_hash=$(sha256sum "$MODEL_PATH" | cut -d ' ' -f 1)
if [[ "${S32_LLAMA_MODEL_SHA256:-}" == written-by-startup ]]; then
  S32_LLAMA_MODEL_SHA256=
fi
if [[ -n "${S32_LLAMA_MODEL_SHA256:-}" && "$actual_hash" != "$S32_LLAMA_MODEL_SHA256" ]]; then
  echo "error: model SHA-256 does not match the approved value" >&2
  exit 1
fi

config_dir=$(dirname -- "$CONFIG_PATH")
mkdir -p "$config_dir"
umask 077
config_tmp=$(mktemp "$config_dir/.ai-llama-default.XXXXXX")
cleanup() {
  if [[ -n "${config_tmp:-}" ]]; then rm -f -- "$config_tmp"; fi
}
trap cleanup EXIT
hash_written=false
while IFS= read -r line || [[ -n "$line" ]]; do
  case "$line" in
    S32_LLAMA_MODEL_SHA256=*)
      printf 'S32_LLAMA_MODEL_SHA256=%s\n' "$actual_hash" >>"$config_tmp"
      hash_written=true
      ;;
    *) printf '%s\n' "$line" >>"$config_tmp" ;;
  esac
done <"$CONFIG_PATH"
if [[ "$hash_written" == false ]]; then
  printf 'S32_LLAMA_MODEL_SHA256=%s\n' "$actual_hash" >>"$config_tmp"
fi
chmod 600 "$config_tmp"
mv -f "$config_tmp" "$CONFIG_PATH"
config_tmp=

if ! command -v "$LLAMA_SERVER_BIN" >/dev/null 2>&1; then
  echo "error: llama-server executable was not found" >&2
  exit 1
fi

export LLAMA_API_KEY=$S32_LLAMA_API_KEY
echo "starting llama.cpp model=$MODEL_ID sha256=$actual_hash bind=0.0.0.0:8080"
exec "$LLAMA_SERVER_BIN" \
  -m "$MODEL_PATH" \
  --device BLAS \
  -ngl 0 \
  -c 4096 \
  --host 0.0.0.0 \
  --port 8080 \
  --alias "$MODEL_ID"
