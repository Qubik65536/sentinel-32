# Repository context

Last updated: 2026-09-19 during `SCOPE-001`.

## Current implementation

The repository began as an uncommitted Cargo binary package. `Cargo.toml` defines `sentinel-32` version `0.1.0`, edition 2024, with no dependencies. `src/main.rs` prints `Hello, world!`. This is a host/toolchain seed only and contains no Sentinel-32 functionality. `Cargo.lock` is committed-ready. The proposed multi-crate workspace does not exist because `BOOT-002` depends on the unproven QNX build.

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
- `docs/s32-isa.md`: incomplete ISA baseline; owned by `ISA-001`.
- `docs/scenario-schema.md`: incomplete schema contract; owned by `SCEN-001`.
- `docs/safety-model.md`: claims, invariant families, containment, evidence.
- `docs/threat-model.md`: assets, untrusted boundaries, abuse cases, controls.
- `docs/validation.md`: validation layers and current results.
- `docs/decisions/`: accepted bootstrap architecture decisions and revisit triggers.

## Verified environment

Host checks pass under upstream Rust 1.98.1 on `x86_64-unknown-linux-gnu`. Only the native target is installed. No QNX SDP environment, `qcc`, QNX-modified rustup toolchain, qnx800 target, or Raspberry Pi 5 access was found. `BUILD-001` is blocked and no QNX claim is complete.

## Next work

The required next technical task is `BUILD-001`. While external QNX prerequisites are being arranged, the independent specification tasks `ISA-001` and `SCEN-001` are ready. `AI-001` waits on `SCEN-001` and `BOOT-002`; later AI tasks implement only the advisory checker. Do not scaffold implementation crates, add target dependencies, or begin the emulator/UI/advisory-checker runtime before `BUILD-001` satisfies its acceptance criteria.

## Working tree note

There are no commits yet. All pre-existing files and bootstrap documents are currently untracked. Preserve them unless a selected task explicitly replaces their role.
