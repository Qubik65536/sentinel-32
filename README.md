# Sentinel-32

Sentinel-32 is a software-only safety laboratory for embedded firmware. It will run a deterministic, MIPS-inspired S32 virtual CPU against declarative mission-critical simulations, beginning with a rocket ground-launch sequencer. Deterministic validation and runtime supervision control what may run and which requests may affect the simulated system.

AI has one narrow, advisory role: compare a bounded snapshot of current state with a versioned set of written safety rules and report possible violations for an operator. OpenAI is used only to test that checker; the hackathon deployment uses a local `llama.cpp` server. AI findings never approve firmware, replace deterministic invariants, or control outputs.

This is a safety-oriented prototype and educational demonstration. It is not certified control software, bounded exploration is not formal proof, observed timing is not WCET, and all launch values are normalized simulation values.

## Current status

Repository groundwork (`BOOT-001`) is complete. The checked-in Rust program is the pre-existing host hello world; the multi-crate workspace has intentionally not been scaffolded because `BOOT-002` depends on the QNX toolchain spike.

`BUILD-001` is blocked in this environment. The host has ordinary upstream Rust but no QNX SDP environment, QNX-modified Rust toolchain, `aarch64-unknown-nto-qnx800` target, or accessible QNX 8.0 Raspberry Pi 5. See [development setup](docs/development.md) and the [work plan](.agent/plan.md).

## Host checks

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The current program can be run with `cargo run`. It demonstrates only that the host Rust installation works; it is not the Sentinel-32 runtime.

## Repository guide

Start with [project scope](docs/project.md), [architecture](docs/architecture.md), and [safety model](docs/safety-model.md). Contributors and coding agents must follow [AGENTS.md](AGENTS.md) and select work from [.agent/plan.md](.agent/plan.md).
