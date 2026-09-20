# Sentinel-32 agent instructions

Sentinel-32 is a software-only safety laboratory for untrusted S32 firmware. The repository is the durable source of truth. Treat the kickstarter as historical input; keep current facts in the canonical documents listed below.

## Start every material task

1. Read `.agent/plan.md`, `.agent/context.md`, and the documents relevant to the task.
2. Inspect existing implementation and tests before editing.
3. Select a task ID, confirm its dependencies, and keep the change within its acceptance criteria.
4. Record unresolved assumptions. Do not turn them into permanent design choices silently.

## Non-negotiable boundaries

- AI is limited to advisory comparison of current state against versioned written safety rules. Its findings are untrusted data and have no path to activation, output authority, safety approval, or deterministic policy evaluation.
- Deterministic software owns assembly, validation, simulation, deployment gating, output gating, and runtime supervision.
- Essential control must continue without AI, UI, or network services.
- Scenario definitions are declarative and untrusted until validated, compiled, and hashed.
- Evidence must bind the exact firmware bytes and compiled scenario bundle.
- Preserve candidate, shadow, active, and last-known-good lifecycle states.
- Never describe bounded exploration as formal proof or measured timing as proven WCET.
- Keep QNX FFI and nearly all `unsafe` code in `sentinel-qnx`; every unsafe block needs a specific `SAFETY` comment.
- Avoid panics and unchecked indexing in target runtime paths. Fail closed with typed errors or controlled traps.
- Never log or commit credentials. `OPENAI_API_KEY` belongs only in the test-only OpenAI checker adapter.
- Human review is required for changes to the ISA, scenario schema, trusted computing base, safety invariants, deployment gate, or secret handling.

## Build and validation

The host lane is:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The QNX lane requires the licensed QNX SDP 8.0 environment and QNX-modified Rust toolchain:

```sh
cargo +<QNX_CUSTOM_TOOLCHAIN> build --workspace \
  --target aarch64-unknown-nto-qnx800 --release
```

Do not substitute an upstream Rust target, guess a custom toolchain name, or mark target validation complete without executing the binary on the Raspberry Pi 5. Record exact commands, versions, tested commit, target image, and results in `docs/development.md` or task evidence.

Keep dependencies conservative. Every dependency must pass the host lane; target-facing dependencies must also pass the QNX cross-build before the dependent task is complete. Prefer standard library, small pure-Rust crates, blocking I/O, and explicit threads.

## Documentation map

- Product scope and vocabulary: `docs/project.md`
- Current and intended architecture: `docs/architecture.md`
- Build and QNX workflow: `docs/development.md`
- Configuration contract: `docs/configuration.md`
- ISA contract: `docs/s32-isa.md`
- Scenario contract: `docs/scenario-schema.md`
- Safety claims and limits: `docs/safety-model.md`
- Threats and trust boundaries: `docs/threat-model.md`
- Validation method: `docs/validation.md`
- Advisory AI data contract: `docs/advisory-ai.md`
- Historical decisions: `docs/decisions/`
- Dependency-aware work: `.agent/plan.md`
- Current repository navigation: `.agent/context.md`

Update behavior, tests, affected canonical documents, `.agent/context.md`, and `.agent/plan.md` together. Before completion, verify every applicable acceptance criterion and record unavailable checks honestly.
