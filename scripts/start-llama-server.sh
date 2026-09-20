#!/usr/bin/env bash
set -euo pipefail

MODEL_PATH=${S32_LLAMA_MODEL_PATH:-"$HOME/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf"}
MODEL_ID=${S32_LLAMA_MODEL_ID:-qwen2.5-1.5b}
LLAMA_SERVER_BIN=${S32_LLAMA_SERVER_BIN:-llama-server}

: "${S32_LLAMA_MODEL_SHA256:?set S32_LLAMA_MODEL_SHA256 to the approved GGUF SHA-256}"
: "${S32_LLAMA_API_KEY:?set S32_LLAMA_API_KEY without placing it in source or shell history}"

if [[ ! -f "$MODEL_PATH" ]]; then
  echo "error: model is not a regular file: $MODEL_PATH" >&2
  exit 1
fi

actual_hash=$(sha256sum "$MODEL_PATH" | cut -d ' ' -f 1)
if [[ "$actual_hash" != "$S32_LLAMA_MODEL_SHA256" ]]; then
  echo "error: model SHA-256 does not match the approved value" >&2
  exit 1
fi

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
