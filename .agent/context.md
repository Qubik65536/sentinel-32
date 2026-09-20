# Repository context

Last updated: 2026-09-19 during `ISA-003`.

## Current implementation

Control operations follow two accepted design principles: an operator starts
one persistent S32 invocation that runs until completion or bounded failure,
and `sentinel.hardware/v0` YAML contains hardware inventory only. Operational
thresholds, waits, sequencing, loops, and actuator requests belong to assembly.

The repository has a Rust workspace. `sentinel-core` implements
typed S32 decode/encode, the source assembler, validated manifests and sparse
memory, and complete S32 v0 reference execution with permissions, capabilities,
atomic traps, and deterministic cycle budgets. `sentinel-scenario` implements
bounded strict YAML parsing, typed validation, canonical bundle compilation,
stable MMIO allocation, generated symbols, and a generic deterministic
runtime. The app exposes VM and scenario commands, including an end-to-end
firmware harness with YAML-allocated MMIO. `BOOT-002`, `ISA-002`,
`VM-001`, and `SCEN-002` remain formally blocked by the missing target-side
evidence dependency in `BUILD-001`; `SCEN-002` also awaits review of its schema
clarification.

Canonical S32 source presentation uses lowercase MIPS-style mnemonics,
directives, and registers with spaced operands. Markdown source examples use
the `asm` fence, and standalone source files use the `.asm` extension so editors
can select assembly highlighting. The assembler remains case-insensitive for
source compatibility.

Current requirements give AI one advisory function: compare a bounded current-state snapshot against versioned written safety rules and emit structured, rule-linked findings. OpenAI is test-only. The hackathon deployment runs the checker client and a pinned local GGUF model served by `llama.cpp` on a companion host, with a loopback-only model endpoint and no remote fallback. AI does not generate firmware or participate in deterministic validation, evidence, activation, safety policy, or output control. This scope is recorded by `SCOPE-001` and `docs/decisions/DEC-011-advisory-ai-checker.md`.

## Navigation

- `Sentinel-32-Project-Kickstarter.md`: initial brief and historical source used for bootstrap; current facts should migrate to canonical docs.
- `AGENTS.md`: binding repository workflow and safety constraints for coding agents.
- `README.md`: project entry point and honest current status.
- `.agent/plan.md`: task dependencies, status, and acceptance criteria.
- `docs/project.md`: scope, non-goals, vocabulary, success boundary.
- `docs/architecture.md`: intended boundaries and lifecycle.
- `docs/development.md`: host/QNX workflow and `BUILD-001` audit/blocker.
- `docs/configuration.md`: intended configuration for core services and the advisory checker backends.
- `docs/s32-isa.md`: accepted ISA v0 contract; implemented incrementally under `ISA-002` and `VM-001`.
- `docs/scenario-schema.md`: accepted schema v0 contract and the concrete forms implemented by `SCEN-002`.
- `crates/sentinel-core`: portable S32 ISA, assembler, VM, memory, traps, cycles, and tests.
- `crates/sentinel-scenario`: strict parser, compiler, canonical artifacts, MMIO symbols, and deterministic runtime.
- `crates/sentinel-app`: host/QNX CLI for VM and scenario workflows.
- `examples/countdown.asm`: source-level assembler smoke example.
- `examples/lab-scenario.yaml`: hardware-only tank-pressure and two-valve MMIO inventory with no controller actions.
- `crates/sentinel-scenario/src/test-scenario.yaml`: internal full-schema compiler fixture.
- `examples/valve-controller.asm`: commented firmware that fills to a pressure target, holds ten simulated seconds, unloads, and closes both valves.
- `docs/safety-model.md`: claims, invariant families, containment, evidence.
- `docs/threat-model.md`: assets, untrusted boundaries, abuse cases, controls.
- `docs/validation.md`: validation layers and current results.
- `docs/decisions/`: accepted bootstrap architecture decisions and revisit triggers.
- `docs/decisions/DEC-012-single-invocation-control.md`: operator-started firmware remains active until its bounded operation completes.

## Verified environment

Host checks pass under upstream Rust 1.98.1 on `x86_64-unknown-linux-gnu`.
QNX SDP 8.0 Build 14 and linked toolchain `qnx800` produced the prior AArch64
QNX 8.0 release binary. The prior scenario-enabled artifact had SHA-256
`50a2c8546e1f256f85d7429b7f1b3e68409559fbc13cdcab6b88cb6824146aae`.
The hardware-only update passes the host lane; its latest QNX link attempt was
blocked by a local QNX license-lock timeout, so that older hash does not identify
the current source.
The operator reports successful earlier Raspberry Pi 5 execution; exact target
image, commands, output, exit status, and execution of the current artifact
remain to be captured before `BUILD-001` is complete.
The documented QNX deployment root is `/data/home/qnxuser/sentinel-32`, with
release, example, and artifact subdirectories. `scripts/qnx-pi-upload.sh`
transfers an already-built release and fixtures to `qnxuser@qnxpi59.local`;
`scripts/qnx-build-upload.sh` runs the full host lane, host and QNX release
builds, scenario artifact generation, and that upload in one command. Target
testing remains a separate operator SSH session.

## Next work

Run the current binary's VM and scenario checks on the Raspberry Pi 5 and
capture the evidence in `docs/development.md`; review the SCEN-002 schema
clarification. The next functional work is `SCEN-003`, the default rocket
scenario, or `VM-002` tracing.

## Working tree note

The tree began from commit `c30b2c0`; preserve unrelated work.
