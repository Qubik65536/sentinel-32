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

The current host is `x86_64-unknown-linux-gnu`. The QNX setup was installed after
the initial audit. The current verified state is:

```text
host rustc: 1.98.1 (48a229cea 2026-09-01), LLVM 22.1.8
host/fallback cargo: 1.98.1 (797e8a9bc 2026-08-05)
linked QNX toolchain: qnx800
QNX rustc: 1.85.1-dev, LLVM 19.1.7
QNX Rust package: QNX SDP 8.0 Build 14, June 30 2025
QNX target cfg: aarch64, little-endian, target_os=nto, target_env=nto80
cross-build artifact: ELF64 PIE, AArch64, interpreter /usr/lib/ldqnx-64.so.2
artifact SHA-256: 182936fa79b41cef453a7f312ce41012e28c5bda517c582ede0a191363a20df0
tested source commit: c30b2c06142727b51aa75931e4fae6f228905bed
```

The clean cross-build completed with:

```sh
source /var/home/qubik65536/qnx800/qnxsdp-env.sh
cargo +qnx800 build --target aarch64-unknown-nto-qnx800 --release
```

The QNX toolchain package does not contain Cargo, so rustup reports that it uses
the host Cargo while retaining the selected QNX compiler and target libraries.
The license manager must be able to create its lock under `~/.qnx/license`.

The operator reports that this hello-world artifact runs on the Raspberry Pi 5.
The exact transfer command, target command, QNX target-image version, captured
stdout, and exit status have not yet been added to repository evidence. This
reported result unblocks continued design work but does not complete the
`BUILD-001` evidence gate.

The Pi is reachable from this host on both SSH and QNX `qconn`. Automated SSH
has no configured noninteractive credential, and the attempted QNX GDB `target
qnx` handshake was rejected after retries. No target command output was
obtained through either path. Target addresses and credentials are deliberately
not recorded in the repository.

`BUILD-001` now waits only for the missing target-side evidence listed above.

## Workspace cross-build — 2026-09-19

After adding functional `sentinel-core` and `sentinel-app` workspace members,
the full QNX lane completed with:

```sh
source /var/home/qubik65536/qnx800/qnxsdp-env.sh
cargo +qnx800 build --workspace \
  --target aarch64-unknown-nto-qnx800 --release
```

The current interpreter-enabled `sentinel-app` is an AArch64 ELF64 PIE using
`/usr/lib/ldqnx-64.so.2`. Its SHA-256 on 2026-09-19 is
`7ac82a528974f75ea56e6ac4378b4029e7e9da3f8956a82d147c072d1d91fab4`.
This result covers cross-compilation and linking, not target execution.

## VM-001 Raspberry Pi 5 test

Build from a Bash shell on the licensed development host. The QNX environment
script deliberately rejects other shells:

```sh
bash
source /var/home/qubik65536/qnx800/qnxsdp-env.sh
cargo +qnx800 build --workspace \
  --target aarch64-unknown-nto-qnx800 --release
file target/aarch64-unknown-nto-qnx800/release/sentinel-app
sha256sum target/aarch64-unknown-nto-qnx800/release/sentinel-app
```

For the current tree, `file` must identify an AArch64 QNX PIE with interpreter
`/usr/lib/ldqnx-64.so.2`, and the expected SHA-256 is
`7ac82a528974f75ea56e6ac4378b4029e7e9da3f8956a82d147c072d1d91fab4`.
If the source changes, record the new tested commit and hash instead of expecting
this value.

Copy both the executable and source fixture to the Pi using the operator's SSH
account and private address. Do not put either value in the repository:

```sh
scp target/aarch64-unknown-nto-qnx800/release/sentinel-app \
  <QNX_USER>@<PI_ADDRESS>:/tmp/sentinel-app
scp examples/countdown.s32 \
  <QNX_USER>@<PI_ADDRESS>:/tmp/countdown.s32
ssh <QNX_USER>@<PI_ADDRESS>
```

On the Pi, record the target image identity, run the positive case, and capture
its exit status:

```sh
uname -a
chmod 755 /tmp/sentinel-app
/tmp/sentinel-app
/tmp/sentinel-app run /tmp/countdown.s32 9
echo $?
```

The identity command should print `sentinel-app s32-isa-v0`. The run's first
line must be exactly:

```text
status=halted steps=9 cycles=9 pc=0x00000014 hi=0x00000000 lo=0x00000000
```

The register dump must show `R01=0x00000000` and `R29=0x20010000`, and the exit
status must be `0`. Then test that the cycle budget fails closed before the
ninth instruction:

```sh
/tmp/sentinel-app run /tmp/countdown.s32 8
echo $?
```

Expected stderr and exit status are:

```text
error: cycle budget exceeded: cycles=8, next_cost=1, budget=8
2
```

Record the date, source commit, binary SHA-256, `uname -a` output, exact transfer
and execution commands, full positive output, both exit statuses, and whether
the device was a Raspberry Pi 5. Remove the two `/tmp` files after the evidence
is captured if the target should not retain test artifacts.

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
