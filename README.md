# Sentinel-32

Sentinel-32 is a software-only safety laboratory for embedded firmware. It will run a deterministic, MIPS-inspired S32 virtual CPU against declarative mission-critical simulations, beginning with a rocket ground-launch sequencer. Deterministic validation and runtime supervision control what may run and which requests may affect the simulated system.

AI has one narrow, advisory role: compare a bounded snapshot of current state with a versioned set of written safety rules and report possible violations for an operator. OpenAI is used only to test that checker; the hackathon deployment uses a local `llama.cpp` server. AI findings never approve firmware, replace deterministic invariants, or control outputs.

This is a safety-oriented prototype and educational demonstration. It is not certified control software, bounded exploration is not formal proof, observed timing is not WCET, and all launch values are normalized simulation values.

## Current status

The Rust workspace now contains `sentinel-core` and `sentinel-app`.
`sentinel-core` implements typed S32 instruction decoding, canonical encoding,
source assembly, and the reference interpreter. The interpreter validates image
manifests and sparse mappings, enforces region permissions and manifest
capabilities, executes the complete S32 v0 instruction set, and applies atomic
traps and deterministic virtual-cycle budgets. The app exposes checking,
assembly, decoding, and an end-to-end lab runner.

The QNX SDP 8.0 Build 14 toolchain cross-builds the workspace for
`aarch64-unknown-nto-qnx800`, and the operator reports successful Raspberry Pi
5 execution. `BUILD-001` remains open only until exact target-side commands,
image version, output, and exit status are captured. See
[development setup](docs/development.md) and the [work plan](.agent/plan.md).

## Host checks

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Decode a word from the accepted S32 ISA with:

```sh
cargo run -p sentinel-app -- decode 0x00221820
```

Check, inspect, or emit the included assembly example with:

```sh
cargo run -p sentinel-app -- check examples/countdown.s32
cargo run -p sentinel-app -- assemble examples/countdown.s32
cargo run -p sentinel-app -- assemble examples/countdown.s32 countdown.bin
cargo run -p sentinel-app -- run examples/countdown.s32 9
```

The assembler supports all v0 instructions, labels, comments, `SP`/`FP`/`RA`,
decimal/hex/binary literals, checked symbol expressions, `.entry`, `.word`,
`.zero`, and the canonical `NOP`, `MOVE`, `B`, `RET`, `LI`, and `LA`
pseudo-instructions.

The `run` command requires a positive virtual-cycle budget. Its lab manifest
maps the assembled program read/execute and provides one 64 KiB read/write
stack capability; it grants no data or MMIO capabilities. The countdown should
halt after nine instructions with nine cycles, `PC=0x00000014`, and `R1=0`.
Target build, transfer, execution, and evidence-capture instructions are in the
[development guide](docs/development.md#vm-001-raspberry-pi-5-test).

## Repository guide

Start with [project scope](docs/project.md), [architecture](docs/architecture.md), and [safety model](docs/safety-model.md). Contributors and coding agents must follow [AGENTS.md](AGENTS.md) and select work from [.agent/plan.md](.agent/plan.md).
