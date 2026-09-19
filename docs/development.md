# Development and QNX build

## Supported lanes

Use the host-native lane for fast development:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The target lane must use the QNX-modified compiler supplied for QNX SDP 8.0 on a supported x86 Linux or Windows host. QNX documents `aarch64-unknown-nto-qnx800` for AArch64 and requires the custom compiler to be linked into rustup before indirect use through Cargo. Source the installed SDP environment on Linux before building.

```sh
source <QNX_SDP_8_INSTALL>/qnxsdp-env.sh
rustup toolchain link <LOCAL_QNX_TOOLCHAIN_NAME> <QNX_RUST_TOOLCHAIN_DIRECTORY>
rustc +<LOCAL_QNX_TOOLCHAIN_NAME> --version --verbose
rustc +<LOCAL_QNX_TOOLCHAIN_NAME> --print cfg \
  --target aarch64-unknown-nto-qnx800
cargo +<LOCAL_QNX_TOOLCHAIN_NAME> build \
  --target aarch64-unknown-nto-qnx800 --release
```

The placeholders are deliberate. Record the actual licensed installation path, local rustup name, compiler version, environment, linker invocation, and emitted cfg values during `BUILD-001`; do not encode guesses in repository configuration.

Official references:

- [QNX SDP 8.0 rust host utility](https://www.qnx.com/developers/docs/8.0/com.qnx.doc.neutrino.utilities/topic/r/rust-host.html)
- [QNX SDP installation requirements](https://www.qnx.com/developers/docs/8.0/com.qnx.doc.qnxsdp.quickstart/topic/requirements.html)
- [Selecting the QNX build OS version and environment](https://qnx.com/developers/docs/8.0/com.qnx.doc.neutrino.prog/topic/devel_OS_version.html)

## BUILD-001 environment audit — 2026-09-19

The current host is `x86_64-unknown-linux-gnu`, which is a suitable host architecture in principle. Inspection found:

```text
rustup toolchains: stable-x86_64-unknown-linux-gnu (active, default)
rustc: 1.98.1 (48a229cea 2026-09-01), LLVM 22.1.8
cargo: 1.98.1 (797e8a9bc 2026-08-05)
installed Rust targets: x86_64-unknown-linux-gnu
qcc: not found
QNX_HOST: unset
QNX_TARGET: unset
aarch64-unknown-nto-qnx800 in rustc target list: no
QNX-modified custom rustup toolchain: not found
QNX SDP/Rust installation under inspected standard paths: not found
QNX 8.0 Raspberry Pi 5 execution access: not available in this workspace
```

The ordinary compiler lists older/upstream QNX targets, but those do not satisfy this project's required QNX 8.0 target or modified-toolchain requirement. No cross-build was attempted because the required target specification, standard library, linker, and SDP sysroot are absent. No target execution was performed.

`BUILD-001` remains blocked until the following are available:

1. a licensed QNX SDP 8.0 installation and its environment script;
2. the QNX-modified Rust distribution for that SDP;
3. an agreed local rustup toolchain name after linking the real directory;
4. transfer and command access to a Raspberry Pi 5 booted into the intended QNX 8.0 image.

## Required BUILD-001 evidence

Capture and commit non-secret evidence for:

- host OS and architecture;
- QNX SDP and target image versions;
- environment initialization command, with user-specific paths generalized where practical;
- `rustc --version --verbose`, `cargo --version`, and `rustc --print cfg` from the custom toolchain;
- exact hello-world cross-build command and linker outcome;
- output of file-format inspection on the binary;
- transfer and target execution commands;
- the Raspberry Pi 5 output and exit status;
- tested commit and date.

Do not commit license data, account identifiers, credentials, or private target addresses.

## Dependency rule

All dependencies must pass the host lane. Any dependency linked into target-facing code must also pass the QNX lane before the consuming task can be completed. Host-only test dependencies must be isolated so they do not enter target resolution.
