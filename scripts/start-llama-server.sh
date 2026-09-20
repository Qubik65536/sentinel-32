#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
MODEL_PATH=${S32_LLAMA_MODEL_PATH:-"$HOME/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf"}
MODEL_ID=${S32_LLAMA_MODEL_ID:-qwen2.5-1.5b}
LLAMA_SERVER_BIN=${S32_LLAMA_SERVER_BIN:-llama-server}
RUNTIME_CONFIG=${S32_AI_RUNTIME_CONFIG_PATH:-"$PROJECT_ROOT/config/ai-llama-runtime.env"}

if [[ -f "$RUNTIME_CONFIG" ]]; then
  while IFS='=' read -r name value; do
    case "$name" in
      S32_LLAMA_API_KEY)
        if [[ -z "${S32_LLAMA_API_KEY:-}" ]]; then S32_LLAMA_API_KEY=$value; fi
        ;;
      S32_LLAMA_MODEL_SHA256)
        if [[ -z "${S32_LLAMA_MODEL_SHA256:-}" ]]; then S32_LLAMA_MODEL_SHA256=$value; fi
        ;;
      ''|'#'*) ;;
      *)
        echo "error: unsupported runtime-config key: $name" >&2
        exit 1
        ;;
    esac
  done <"$RUNTIME_CONFIG"
fi

: "${S32_LLAMA_API_KEY:?set S32_LLAMA_API_KEY or create the local runtime config}"

if [[ ! "$S32_LLAMA_API_KEY" =~ ^[A-Za-z0-9._~-]+$ ]]; then
  echo "error: S32_LLAMA_API_KEY contains characters unsupported by the env-file format" >&2
  exit 1
fi

if [[ ! -f "$MODEL_PATH" ]]; then
  echo "error: model is not a regular file: $MODEL_PATH" >&2
  exit 1
fi

actual_hash=$(sha256sum "$MODEL_PATH" | cut -d ' ' -f 1)
if [[ -n "${S32_LLAMA_MODEL_SHA256:-}" && "$actual_hash" != "$S32_LLAMA_MODEL_SHA256" ]]; then
  echo "error: model SHA-256 does not match the approved value" >&2
  exit 1
fi

runtime_dir=$(dirname -- "$RUNTIME_CONFIG")
mkdir -p "$runtime_dir"
umask 077
runtime_tmp=$(mktemp "$runtime_dir/.ai-llama-runtime.XXXXXX")
cleanup() {
  if [[ -n "${runtime_tmp:-}" ]]; then rm -f -- "$runtime_tmp"; fi
}
trap cleanup EXIT
{
  printf 'S32_LLAMA_API_KEY=%s\n' "$S32_LLAMA_API_KEY"
  printf 'S32_LLAMA_MODEL_SHA256=%s\n' "$actual_hash"
} >"$runtime_tmp"
chmod 600 "$runtime_tmp"
mv -f "$runtime_tmp" "$RUNTIME_CONFIG"
runtime_tmp=

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
