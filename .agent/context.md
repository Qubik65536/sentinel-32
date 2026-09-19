# Repository context

Last updated: 2026-09-19 during `VM-001`.

## Current implementation

The repository has a dependency-free Rust workspace. `sentinel-core` implements
typed S32 decode/encode, the source assembler, validated manifests and sparse
memory, and complete S32 v0 reference execution with permissions, capabilities,
atomic traps, and deterministic cycle budgets. `sentinel-app` exposes `decode`,
`check`, `assemble`, and `run`. The countdown example executes end to end in
nine steps and cycles. `BOOT-002`, `ISA-002`, and `VM-001` remain formally
blocked only by the missing target-side evidence dependency in `BUILD-001`.

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
- `docs/scenario-schema.md`: accepted schema v0 contract; implementation starts under `SCEN-002`.
- `crates/sentinel-core`: portable S32 ISA, assembler, VM, memory, traps, cycles, and tests.
- `crates/sentinel-app`: host/QNX CLI for decoding, checking, assembling, and bounded execution.
- `examples/countdown.s32`: source-level assembler smoke example.
- `docs/safety-model.md`: claims, invariant families, containment, evidence.
- `docs/threat-model.md`: assets, untrusted boundaries, abuse cases, controls.
- `docs/validation.md`: validation layers and current results.
- `docs/decisions/`: accepted bootstrap architecture decisions and revisit triggers.

## Verified environment

Host checks pass under upstream Rust 1.98.1 on `x86_64-unknown-linux-gnu`.
QNX SDP 8.0 Build 14 and linked toolchain `qnx800` produce the AArch64 QNX 8.0
release binary. The interpreter-enabled artifact cross-build passed with
SHA-256 `7ac82a528974f75ea56e6ac4378b4029e7e9da3f8956a82d147c072d1d91fab4`.
The operator reports successful earlier Raspberry Pi 5 execution; exact target
image, commands, output, exit status, and execution of the current artifact
remain to be captured before `BUILD-001` is complete.

## Next work

Run the current interpreter-enabled binary on the Raspberry Pi 5 and capture
the evidence in `docs/development.md`; that closes `BUILD-001`, `BOOT-002`,
`ISA-002`, and `VM-001`. The next functional work can begin `VM-002` tracing or
`SCEN-002` bounded scenario compilation.

## Working tree note

The tree began from commit `c30b2c0`; preserve unrelated work.
