# Sentinel-32

Sentinel-32 is a software-only safety laboratory for embedded firmware. It will run a deterministic, MIPS-inspired S32 virtual CPU against declarative mission-critical simulations, beginning with a rocket ground-launch sequencer. Deterministic validation and runtime supervision control what may run and which requests may affect the simulated system.

AI has one narrow, advisory role: compare a bounded snapshot of current state with a versioned set of written safety rules and report possible violations for an operator. OpenAI is used only to test that checker; the hackathon deployment uses an authenticated `llama.cpp` deployment server. AI findings never approve firmware, replace deterministic invariants, or control outputs.

This is a safety-oriented prototype and educational demonstration. It is not certified control software, its deterministic tests are not formal proof, observed timing is not WCET, and all launch values are normalized simulation values.

## Current status

The Rust workspace now contains `sentinel-core`, `sentinel-scenario`,
`sentinel-safety`, `sentinel-ai-check`, and `sentinel-app`.
`sentinel-core` implements typed S32 instruction decoding, canonical encoding,
source assembly, and the reference interpreter. The interpreter validates image
manifests and sparse mappings, enforces region permissions and manifest
capabilities, executes the complete S32 v0 instruction set, and applies atomic
traps and deterministic virtual-cycle budgets. `sentinel-scenario` parses the
bounded YAML subset, validates and compiles immutable scenario bundles and S32
symbols, and executes deterministic dynamics, faults, rules, and phase changes.
The default declarative rocket scenario exercises normalized propellant
pressure, valve feedback, electrical power, ignition readiness, clearance,
countdown, hold, abort, and named deterministic faults. `sentinel-safety`
turns compiled rule results and firmware requests into accepted, overridden,
or rejected outputs with stable reason codes. `sentinel-ai-check` defines and
validates the bounded advisory snapshot, written-rule, and finding contract;
it has no output or activation API. The app exposes the VM and scenario
workflows.

`sentinel-app tui` is a Ratatui interface for visually assembling source,
inspecting machine execution, running missions, and reviewing advisory
findings. It uses a small safe ANSI backend that cross-builds for QNX 8.0 and
works in an allocated SSH terminal. Existing CLI workflows remain available.
See the [terminal interface guide](docs/tui-user-guide.md),
[development setup](docs/development.md), and [work plan](.agent/plan.md).

## Host checks

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Open the visual laboratory with:

```sh
cargo run -p sentinel-app -- tui examples/sample-analysis.asm
```

Use keys `1` through `4` for Assemble, Run, Mission, and Advisory, and `?` for
contextual help. On QNX, connect with `ssh -t`, set
`S32_DEMO_ROOT=/data/home/qnxuser/sentinel-32`, and run the same `tui` command
from the deployed `release` directory.

Decode a word from the accepted S32 ISA with:

```sh
cargo run -p sentinel-app -- decode 0x00221820
```

Check, inspect, or emit the included assembly example with:

```sh
cargo run -p sentinel-app -- check examples/sample-analysis.asm
cargo run -p sentinel-app -- assemble examples/sample-analysis.asm
cargo run -p sentinel-app -- assemble examples/sample-analysis.asm sample-analysis.bin
cargo run -p sentinel-app -- run examples/sample-analysis.asm 256
cargo run -p sentinel-app -- step examples/sample-analysis.asm 256
```

The assembler supports all v0 instructions, labels, comments, `sp`/`fp`/`ra`,
decimal/hex/binary literals, checked symbol expressions, `.entry`, `.word`,
`.zero`, and the canonical `nop`, `move`, `b`, `ret`, `li`, and `la`
pseudo-instructions. Project assembly uses lowercase MIPS-style mnemonics and
registers, standalone source uses the `.asm` extension, and Markdown source
examples use fenced `asm` blocks.

The `run` command requires a positive virtual-cycle budget. Its lab manifest
maps the assembled program read/execute and provides one 64 KiB read/write
stack capability; it grants no data or MMIO capabilities. The sample-analysis
example builds a five-word stack buffer, calls a function, and halts after 91
instructions and 112 cycles with `PC=0x00000070`. Its result registers contain
sum `R2=66`, maximum `R3=25`, threshold count `R7=3`, average `R8=13`, and
remainder `R9=1`; `SP` is restored to `0x20010000`.
The interactive `step` command uses the same manifest and cycle budget. Enter
or `s` executes one instruction, `s N` executes a bounded group, `r` displays
registers, `c` continues, `h` shows help, and `q` exits without executing more
instructions. Each step reports the decoded instruction, cycle charge, next
PC, register changes, memory writes, and machine status.
Target build, transfer, execution, and evidence-capture instructions are in the
[development guide](docs/development.md#vm-001-raspberry-pi-5-test).
The documented Raspberry Pi/QNX deployment root is
`/data/home/qnxuser/sentinel-32`; target commands use absolute paths beneath
it. From the configured distrobox, `./scripts/qnx-pi-upload.sh` transfers the
already-built QNX release, examples, and provenance artifacts to
`qnxuser@qnxpi59.local`. Target tests are run separately over SSH.
Use `./scripts/qnx-build-upload.sh` to run the complete host validation and
host/QNX builds, generate the lab artifacts, and upload everything in one step.

Compile the hardware inventory and run the assembly-owned action sequence with:

```sh
cargo run -p sentinel-app -- hardware-check examples/lab-scenario.yaml
cargo run -p sentinel-app -- hardware-compile \
  examples/lab-scenario.yaml bundle.json symbols.inc
cargo run -p sentinel-app -- mission-run \
  examples/lab-scenario.yaml examples/valve-controller.asm 1000
```

The YAML contains hardware existence, value types, and reset values only. The
firmware obtains pressure, inlet, and outlet addresses from the compiled
inventory. One operator start runs a persistent assembly loop that fills to
50.000 pressure units, counts a ten-second hold in `R8`, unloads to zero, closes
both valves, and halts. The runner prints each second's readings and MMIO writes
without restarting the firmware. The
[development guide](docs/development.md#scen-002-raspberry-pi-5-test) gives the
equivalent Raspberry Pi 5 checks and expected output.

Compile or inspect the default rocket launch-pad digital twin with:

```sh
cargo run -p sentinel-app -- scenario-check \
  examples/rocket-launch-default.yaml
cargo run -p sentinel-app -- scenario-compile \
  examples/rocket-launch-default.yaml rocket-bundle.json rocket-symbols.inc
cargo run -p sentinel-app -- mission-run \
  examples/rocket-launch-default.yaml examples/rocket-controller.asm 5000
```

The scenario compiles to 23 MMIO slots and a golden bundle hash recorded by its
tests. One operator start launches a persistent S32 firmware invocation. The
assembly owns pressure polling, the normalized 50000 loading threshold, valve
commands, stabilization wait, electrical and readiness checks, countdown
observation, ignition, feedback confirmation, shutdown, and its abort path. The
runner provides the declarative software twin and records the operator start
plus two simulated supervisor approvals. A nominal run reaches `complete` in 19
scenario ticks, confirms the final closed/safe feedback state, and halts the
same firmware invocation. Host integration tests also exercise exact pressure
edges, telemetry staleness, valve faults, power and readiness loss,
asynchronous hold and abort, abort persistence, and supervised new-attempt
reset. These are deterministic software-twin results, not physical launch
parameters or formal proof. The
[development guide](docs/development.md#scen-003-raspberry-pi-5-smoke-test)
gives the QNX command and expected output.

## Repository guide

Start with [project scope](docs/project.md), [architecture](docs/architecture.md),
[safety model](docs/safety-model.md), and the
[advisory AI contract](docs/advisory-ai.md). Contributors and coding agents
must follow [AGENTS.md](AGENTS.md) and select work from
[.agent/plan.md](.agent/plan.md).

The implemented key map and host/QNX operation are in the
[terminal interface guide](docs/tui-user-guide.md). The layouts, event flow,
and terminal-safety design are in the [Ratatui interface design](docs/tui.md).

The [QNX visual and command-line demo](docs/demo.md) gives the exact target
commands and the visible result of each step. The
[advisory AI user guide](docs/ai-user-guide.md) adds deployment-server setup,
configuration, OpenAI development checks, and troubleshooting.
