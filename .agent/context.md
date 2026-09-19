# Repository context

Last updated: 2026-09-19 during `BOOT-002`, `ISA-002`, and `VM-001`.

## Current implementation

The repository now has a dependency-free Rust workspace. `sentinel-core`
implements typed S32 decode/encode, precise illegal-versus-reserved decoding,
architectural cycle costs, and the source assembler. `sentinel-app` exposes
`decode`, `check`, and `assemble` commands. `BOOT-002` remains formally blocked
only by the missing target-side evidence dependency in `BUILD-001`.

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
- `crates/sentinel-core`: portable S32 ISA types, decoder, encoder, assembler, cycles, and tests.
- `crates/sentinel-app`: host/QNX command-line interface for decoding, checking, and assembling.
- `examples/countdown.s32`: source-level assembler smoke example.
- `docs/safety-model.md`: claims, invariant families, containment, evidence.
- `docs/threat-model.md`: assets, untrusted boundaries, abuse cases, controls.
- `docs/validation.md`: validation layers and current results.
- `docs/decisions/`: accepted bootstrap architecture decisions and revisit triggers.

## Verified environment

Host checks pass under upstream Rust 1.98.1 on `x86_64-unknown-linux-gnu`.
QNX SDP 8.0 Build 14 and linked toolchain `qnx800` now produce the AArch64
QNX 8.0 release binary. The operator reports successful Raspberry Pi 5
execution; exact target image, commands, output, and exit status remain to be
captured before `BUILD-001` is complete.

## Next work

Finish the target evidence for `BUILD-001`, then close `BOOT-002` and `ISA-002`.
Continue `VM-001` with machine state and execution semantics, and `SCEN-002`
with bounded source decoding and typed validation.

## Working tree note

The tree began from commit `c30b2c0`; preserve unrelated work.
