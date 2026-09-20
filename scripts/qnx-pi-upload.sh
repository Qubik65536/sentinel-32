#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
TARGET_HOST=qnxuser@qnxpi59.local
TARGET_PARENT=/data/home/qnxuser
TARGET_TRIPLE=aarch64-unknown-nto-qnx800
APP="$PROJECT_ROOT/target/qnx800/$TARGET_TRIPLE/release/sentinel-app"
STAGING_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/sentinel-qnx-upload.XXXXXX")
STAGING_TREE="$STAGING_ROOT/sentinel-32"

cleanup() {
  rm -rf "$STAGING_ROOT"
}
trap cleanup EXIT

if [[ ! -f "$APP" ]]; then
  echo "error: QNX release binary does not exist: $APP" >&2
  echo "build it before running this transfer script" >&2
  exit 1
fi

mkdir -p \
  "$STAGING_TREE/release" \
  "$STAGING_TREE/config" \
  "$STAGING_TREE/examples" \
  "$STAGING_TREE/examples/ai" \
  "$STAGING_TREE/artifacts"

cp "$APP" "$STAGING_TREE/release/sentinel-app"
cp "$PROJECT_ROOT/config/ai-llama-default.env.example" \
  "$STAGING_TREE/config/ai-llama-default.env.example"
cp "$PROJECT_ROOT/examples/countdown.asm" "$STAGING_TREE/examples/countdown.asm"
cp "$PROJECT_ROOT/examples/valve-controller.asm" \
  "$STAGING_TREE/examples/valve-controller.asm"
cp "$PROJECT_ROOT/examples/rocket-controller.asm" \
  "$STAGING_TREE/examples/rocket-controller.asm"
cp "$PROJECT_ROOT/examples/lab-scenario.yaml" \
  "$STAGING_TREE/examples/lab-scenario.yaml"
cp "$PROJECT_ROOT/examples/rocket-launch-default.yaml" \
  "$STAGING_TREE/examples/rocket-launch-default.yaml"
cp "$PROJECT_ROOT/examples/ai/rocket-pressure-snapshot.json" \
  "$STAGING_TREE/examples/ai/rocket-pressure-snapshot.json"
cp "$PROJECT_ROOT/examples/ai/rocket-written-rules.json" \
  "$STAGING_TREE/examples/ai/rocket-written-rules.json"
cp "$PROJECT_ROOT/examples/ai/tank-proposed-action-snapshot.json" \
  "$STAGING_TREE/examples/ai/tank-proposed-action-snapshot.json"
cp "$PROJECT_ROOT/examples/ai/tank-written-rules.json" \
  "$STAGING_TREE/examples/ai/tank-written-rules.json"
cp "$PROJECT_ROOT/examples/ai/rocket-fixture-response.json" \
  "$STAGING_TREE/examples/ai/rocket-fixture-response.json"
cp "$PROJECT_ROOT/Cargo.lock" "$STAGING_TREE/artifacts/Cargo.lock"

(
  cd "$STAGING_TREE/release"
  sha256sum sentinel-app >../artifacts/sentinel-app.sha256
)

{
  git -C "$PROJECT_ROOT" rev-parse HEAD
  git -C "$PROJECT_ROOT" status --short
} >"$STAGING_TREE/artifacts/source-revision.txt"

for artifact in "$@"; do
  if [[ ! -f "$artifact" ]]; then
    echo "error: extra artifact is not a regular file: $artifact" >&2
    exit 1
  fi
  cp "$artifact" "$STAGING_TREE/artifacts/"
done

echo "Uploading to $TARGET_HOST:$TARGET_PARENT/sentinel-32"
echo "Enter the SSH password if prompted."
scp -r "$STAGING_TREE" "$TARGET_HOST:$TARGET_PARENT/"

echo "Upload complete. Target layout:"
find "$STAGING_TREE" -type f \
  -printf '  /data/home/qnxuser/sentinel-32/%P\n' | sort
