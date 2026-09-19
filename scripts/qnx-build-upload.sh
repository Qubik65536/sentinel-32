#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
QNX_ENV=/var/home/qubik65536/qnx800/qnxsdp-env.sh
HOST_TOOLCHAIN=stable
TARGET_TRIPLE=aarch64-unknown-nto-qnx800
HOST_TARGET_DIR="$PROJECT_ROOT/target/host"
QNX_TARGET_DIR="$PROJECT_ROOT/target/qnx800"
ARTIFACT_DIR="$PROJECT_ROOT/target/qnx-artifacts"
QNX_APP="$QNX_TARGET_DIR/$TARGET_TRIPLE/release/sentinel-app"
HOST_STAMP="$HOST_TARGET_DIR/.sentinel-rustc"

if ! command -v cc >/dev/null 2>&1; then
  echo "error: host linker 'cc' is unavailable" >&2
  echo "install it in Ubuntu with: sudo apt-get install build-essential" >&2
  exit 1
fi

if [[ ! -f "$QNX_ENV" ]]; then
  echo "error: QNX environment script does not exist: $QNX_ENV" >&2
  exit 1
fi

HOST_RUSTC=$(rustup which --toolchain "$HOST_TOOLCHAIN" rustc)
HOST_RUSTDOC=$(rustup which --toolchain "$HOST_TOOLCHAIN" rustdoc)
HOST_FINGERPRINT=$(
  {
    printf '%s\n' "$HOST_RUSTC" "$HOST_RUSTDOC"
    "$HOST_RUSTC" --version --verbose
    "$HOST_RUSTDOC" --version
  } | sha256sum | awk '{print $1}'
)

host_cargo() {
  CARGO_TARGET_DIR="$HOST_TARGET_DIR" \
    RUSTC="$HOST_RUSTC" \
    RUSTDOC="$HOST_RUSTDOC" \
    cargo +"$HOST_TOOLCHAIN" "$@"
}

cd "$PROJECT_ROOT"

if [[ -d "$HOST_TARGET_DIR" ]] &&
  [[ ! -f "$HOST_STAMP" || "$(cat "$HOST_STAMP")" != "$HOST_FINGERPRINT" ]]
then
  echo "==> Removing host artifacts from a different Rust compiler"
  cargo +"$HOST_TOOLCHAIN" clean --target-dir "$HOST_TARGET_DIR"
fi
mkdir -p "$HOST_TARGET_DIR"
printf '%s\n' "$HOST_FINGERPRINT" >"$HOST_STAMP"

echo "==> Validating the host workspace"
host_cargo fmt --check
host_cargo clippy --workspace --all-targets -- -D warnings
host_cargo test --workspace

echo "==> Building the host release workspace"
host_cargo build --workspace --release

echo "==> Compiling the lab hardware artifacts"
mkdir -p "$ARTIFACT_DIR"
host_cargo run --quiet --release -p sentinel-app -- hardware-compile \
  examples/lab-scenario.yaml \
  "$ARTIFACT_DIR/hardware-bundle.json" \
  "$ARTIFACT_DIR/hardware-symbols.inc"

echo "==> Building the QNX release workspace"
# shellcheck source=/dev/null
source "$QNX_ENV"
CARGO_TARGET_DIR="$QNX_TARGET_DIR" \
  cargo +qnx800 build --workspace --target "$TARGET_TRIPLE" --release

echo "==> Inspecting the QNX executable"
file "$QNX_APP"
sha256sum "$QNX_APP"

echo "==> Uploading the QNX release and artifacts"
"$PROJECT_ROOT/scripts/qnx-pi-upload.sh" \
  "$ARTIFACT_DIR/hardware-bundle.json" \
  "$ARTIFACT_DIR/hardware-symbols.inc"

echo "==> Build and upload complete"
echo "Open the existing QNX SSH session and run the documented target tests."
