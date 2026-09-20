# Development and QNX build

## Supported lanes

Use the host-native lane for fast development:

```sh
CARGO_TARGET_DIR=target/host cargo fmt --check
CARGO_TARGET_DIR=target/host \
  cargo clippy --workspace --all-targets -- -D warnings
CARGO_TARGET_DIR=target/host cargo test --workspace
```

Rust build scripts are host executables and require the distrobox to provide a
host linker named `cc`, even when the project dependencies are pure Rust. In the
Ubuntu-based development distrobox, install it once with
`sudo apt-get install build-essential` after refreshing the package index. A
Fedora-based distrobox uses `sudo dnf install gcc` instead. Verify the result
with `command -v cc` before running the host lane or a clean QNX build.

Host and QNX commands use separate Cargo target directories because the custom
QNX compiler also creates host-side build artifacts. Sharing one target
directory can make stable Rust load dependencies produced by the incompatible
QNX compiler and fail with `E0514`. The automated build script pins absolute
stable `rustc` and `rustdoc` paths for its host lane, so it also works when the
calling shell has already sourced `qnxsdp-env.sh`. A compiler fingerprint in
`target/host` causes that cache alone to be cleaned when the host compiler
changes or when an unrecognized cache predates the fingerprint.

The target lane must use the QNX-modified compiler supplied for QNX SDP 8.0 on a supported x86 Linux or Windows host. QNX documents `aarch64-unknown-nto-qnx800` for AArch64 and requires the custom compiler to be linked into rustup before indirect use through Cargo. Source the installed SDP environment on Linux before building.

```sh
source <QNX_SDP_8_INSTALL>/qnxsdp-env.sh
rustup toolchain link <LOCAL_QNX_TOOLCHAIN_NAME> <QNX_RUST_TOOLCHAIN_DIRECTORY>
rustc +<LOCAL_QNX_TOOLCHAIN_NAME> --version --verbose
rustc +<LOCAL_QNX_TOOLCHAIN_NAME> --print cfg \
  --target aarch64-unknown-nto-qnx800
cargo +<LOCAL_QNX_TOOLCHAIN_NAME> build \
  --target aarch64-unknown-nto-qnx800 --release \
  --target-dir target/qnx800
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
cargo +qnx800 build --target aarch64-unknown-nto-qnx800 --release \
  --target-dir target/qnx800
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

After adding functional `sentinel-core`, `sentinel-scenario`, and `sentinel-app`
workspace members,
the full QNX lane completed with:

```sh
source /var/home/qubik65536/qnx800/qnxsdp-env.sh
cargo +qnx800 build --workspace \
  --target aarch64-unknown-nto-qnx800 --release \
  --target-dir target/qnx800
```

The current scenario-enabled `sentinel-app` is an AArch64 ELF64 PIE using
`/usr/lib/ldqnx-64.so.2`. Its SHA-256 on 2026-09-19 is
`50a2c8546e1f256f85d7429b7f1b3e68409559fbc13cdcab6b88cb6824146aae`.
This result covers cross-compilation and linking, not target execution.

## Expanded workspace cross-build attempt — 2026-09-19

After adding `sentinel-safety`, `sentinel-ai-check`, the default rocket
scenario, and its persistent assembly runner, the documented QNX command was
run again under Bash:

```sh
source /var/home/qubik65536/qnx800/qnxsdp-env.sh
cargo +qnx800 build --workspace \
  --target aarch64-unknown-nto-qnx800 --release \
  --target-dir target/qnx800
```

The QNX environment initialized and the target Rust compiler compiled the new
`sentinel-scenario`, `sentinel-ai-check`, and `sentinel-safety` libraries. The
final `sentinel-app` link failed with exit status 129 because the QNX license
manager timed out acquiring
`/home/qubik65536/.qnx/license/licenses.lck` after repeated ten-second waits.
This is an unavailable license check, not a Rust compilation failure and not a
passing QNX build. No new target artifact or target-execution evidence was
recorded.

The same lane was rerun after adding the authenticated standard-library
`llama.cpp` client and AI CLI. The QNX compiler compiled the target objects,
including the networking code. The final link again failed with exit status
129 after repeated license-lock timeouts. This confirms compilation only; it
does not establish a passing QNX build or target execution of the client.

## Raspberry Pi 5 deployment directory

The documented QNX target deployment root is
`/data/home/qnxuser/sentinel-32`. The release executable is stored under
`release/`, fixtures under `examples/`, and provenance or generated files under
`artifacts/`. All target commands below use absolute paths; testing does not
depend on `/tmp` or the current working directory.

Create it once on the Pi if it does not already exist:

```sh
mkdir -p /data/home/qnxuser/sentinel-32/release \
  /data/home/qnxuser/sentinel-32/examples \
  /data/home/qnxuser/sentinel-32/artifacts
chmod 755 /data/home/qnxuser/sentinel-32
```

From the repository root inside the configured distrobox, upload an already
built QNX release and its fixtures with:

```sh
./scripts/qnx-pi-upload.sh
```

The script only stages and transfers files; it does not build or run target
tests. It targets `qnxuser@qnxpi59.local` and uploads the executable, examples,
`Cargo.lock`, binary SHA-256, and source revision. Extra files passed as
arguments are copied into the target `artifacts/` directory. It does not store
the SSH password or place it in a command argument.

For the normal edit-to-Pi workflow, run the complete validation, host release
build, scenario artifact compilation, QNX release build, and upload with one
command:

```sh
./scripts/qnx-build-upload.sh
```

This wrapper requires `cc` and the installed QNX environment described above.
It invokes `qnx-pi-upload.sh` after every build succeeds and leaves target test
execution to the operator's existing SSH session.

## VM-001 Raspberry Pi 5 test

Build from a Bash shell on the licensed development host. The QNX environment
script deliberately rejects other shells:

```sh
bash
source /var/home/qubik65536/qnx800/qnxsdp-env.sh
cargo +qnx800 build --workspace \
  --target aarch64-unknown-nto-qnx800 --release \
  --target-dir target/qnx800
file target/qnx800/aarch64-unknown-nto-qnx800/release/sentinel-app
sha256sum target/qnx800/aarch64-unknown-nto-qnx800/release/sentinel-app
```

`file` must identify an AArch64 QNX PIE with interpreter
`/usr/lib/ldqnx-64.so.2`. The prior three-crate artifact had SHA-256
`50a2c8546e1f256f85d7429b7f1b3e68409559fbc13cdcab6b88cb6824146aae`;
it does not identify the current source. Record the newly built binary's hash
with the tested commit instead of expecting the prior value.

Copy both the executable and source fixture to the Pi using the documented
`qnxuser@qnxpi59.local` target:

```sh
scp target/qnx800/aarch64-unknown-nto-qnx800/release/sentinel-app \
  qnxuser@qnxpi59.local:/data/home/qnxuser/sentinel-32/release/sentinel-app
scp examples/countdown.asm \
  qnxuser@qnxpi59.local:/data/home/qnxuser/sentinel-32/examples/countdown.asm
scp examples/valve-controller.asm \
  qnxuser@qnxpi59.local:/data/home/qnxuser/sentinel-32/examples/valve-controller.asm
ssh qnxuser@qnxpi59.local
```

On the Pi, record the target image identity, run the positive case, and capture
its exit status:

```sh
uname -a
chmod 755 /data/home/qnxuser/sentinel-32/release/sentinel-app
/data/home/qnxuser/sentinel-32/release/sentinel-app
/data/home/qnxuser/sentinel-32/release/sentinel-app run \
  /data/home/qnxuser/sentinel-32/examples/countdown.asm 9
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
/data/home/qnxuser/sentinel-32/release/sentinel-app run \
  /data/home/qnxuser/sentinel-32/examples/countdown.asm 8
echo $?
```

Expected stderr and exit status are:

```text
error: cycle budget exceeded: cycles=8, next_cost=1, budget=8
2
```

Record the date, source commit, binary SHA-256, `uname -a` output, exact transfer
and execution commands, full positive output, both exit statuses, and whether
the device was a Raspberry Pi 5.

## SCEN-002 Raspberry Pi 5 test

Use the same final binary and target identity captured above. The upload script
places the hardware manifest and assembly program under the documented target
tree. Validate and compile the hardware inventory:

```sh
/data/home/qnxuser/sentinel-32/release/sentinel-app hardware-check \
  /data/home/qnxuser/sentinel-32/examples/lab-scenario.yaml
echo $?
/data/home/qnxuser/sentinel-32/release/sentinel-app hardware-compile \
  /data/home/qnxuser/sentinel-32/examples/lab-scenario.yaml \
  /data/home/qnxuser/sentinel-32/artifacts/hardware-bundle.json \
  /data/home/qnxuser/sentinel-32/artifacts/hardware-symbols.inc
echo $?
```

Both commands must exit `0` and report five MMIO entries. Then run the
assembly-owned action sequence against those YAML-declared registers:

```sh
/data/home/qnxuser/sentinel-32/release/sentinel-app tank-run \
  /data/home/qnxuser/sentinel-32/examples/lab-scenario.yaml \
  /data/home/qnxuser/sentinel-32/examples/valve-controller.asm 1000
echo $?
```

The MMIO preamble must map inlet and outlet requests to `0x50000000` and
`0x50000004`, their feedback to `0x60000000` and `0x60000004`, and pressure
telemetry to `0x40000000`. Verify these stages:

- seconds 1 through 5: inlet open, outlet closed, pressure rises to 50000;
- seconds 6 through 15: both closed and pressure remains 50000;
- seconds 16 through 20: inlet closed, outlet open, pressure falls to zero;
- second 21: both valves closed at zero pressure.

The final line must report one completed execution with 203 instructions, 257
virtual cycles, 21 simulated seconds, and both requests closed. This proves the
operator started firmware once, YAML only supplied hardware existence and
encoding, and assembly retained control until completion. The command must exit
`0`.

Also verify fail-closed argument handling:

```sh
/data/home/qnxuser/sentinel-32/release/sentinel-app tank-run \
  /data/home/qnxuser/sentinel-32/examples/lab-scenario.yaml \
  /data/home/qnxuser/sentinel-32/examples/valve-controller.asm 256
echo $?
```

It must report a cycle-budget-exceeded error and exit `2`; the firmware must not
be restarted to finish the operation.
Record the exact output, exit statuses, tested source commit, binary hash, and
target image identity. Keep the executable and fixtures in the documented
deployment tree. Generated hardware and rocket bundle/symbol artifacts may be
replaced by the next tested release after their evidence is captured.

## SCEN-003 Raspberry Pi 5 smoke test

The build/upload wrapper transfers the default rocket scenario, its S32
controller, and the host-built canonical bundle and symbols. On the target,
first validate the scenario:

```sh
/data/home/qnxuser/sentinel-32/release/sentinel-app scenario-check \
  /data/home/qnxuser/sentinel-32/examples/rocket-launch-default.yaml
echo $?
```

The command must report scenario `rocket_launch_default`, publication `1`, 23
MMIO entries, bundle hash
`72975636c2b7c0db7931bc09defcd3ce27adc3ab98f76fa4d908128f94c25388`,
and exit `0`.

Then start the persistent assembly-controlled sample:

```sh
/data/home/qnxuser/sentinel-32/release/sentinel-app rocket-run \
  /data/home/qnxuser/sentinel-32/examples/rocket-launch-default.yaml \
  /data/home/qnxuser/sentinel-32/examples/rocket-controller.asm \
  5000
echo $?
```

The procedure is:

1. The operator starts one S32 invocation and the runner records
   `start_loading` as the operator-start transition.
2. Assembly polls both pressure MMIO registers and independently commands each
   fill valve until the normalized firmware threshold of `50000` is reached.
3. Assembly closes the fill valves for two command frames, then reads controller
   power, both buses, continuity, flight readiness, range and pad clearance, and
   the remote inhibit. Any failed check enters the assembly abort path.
4. The runner records explicit simulated `arm_launch` and
   `begin_terminal_count` supervisor approvals when the firmware requests the
   armed state. The approvals are outside firmware authority.
5. Assembly observes the scenario timer until tick 5, opens both main valves,
   requests ignition, and waits for ignition feedback.
6. Assembly commands all valves closed and ignition safe for two frames, waits
   for the one-tick feedback delay to settle, and halts.

The run must begin with `rocket-firmware scenario=rocket_launch_default`, the
same bundle hash, `start=operator_once`, and `cycle_budget=5000`. Its final line
must report:

```text
operation=complete firmware_status=halted firmware_steps=248 firmware_cycles=391 scenario_ticks=19 final_phase=complete final_requests=[actuator.fuel_fill_valve=closed,actuator.fuel_main_valve=closed,actuator.ignition=safe,actuator.oxidizer_fill_valve=closed,actuator.oxidizer_main_valve=closed,actuator.vent_valve=closed]
```

The process must exit `0`. This is a deterministic nominal software-twin run;
it is not a real launch procedure, formal proof, or physical timing evidence.
Run the fault and boundary truth tables on the host with:

```sh
cargo test -p sentinel-scenario rocket_ -- --nocapture
```

Record the QNX commands, complete output, exit statuses, tested commit, binary
hash, and target image with the run evidence.

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
