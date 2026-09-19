# Validation methodology

Validation is layered and versioned. A result applies only to the exact artifacts, environment, bounds, and tool versions recorded in its evidence.

## Developer checks

The default host gate is:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Target-facing changes additionally require the QNX cross-build and relevant Raspberry Pi 5 tests described in `docs/development.md`. An unavailable target check is recorded as unavailable, never passed.

## Firmware validation pipeline

1. Parse a bounded proposal and verify scenario identity/hash.
2. Assemble with line-specific diagnostics and produce canonical bytes.
3. Decode every reachable instruction and validate entry point/control-flow targets.
4. Validate memory regions, manifest capabilities, program/stack bounds, and cycle/deadline budgets.
5. Run opcode vectors, regressions, scenario cases, and fault cases.
6. Explore a bounded world/input state space with canonical-state deduplication.
7. Produce a concrete counterexample for rejection when possible.
8. Run the candidate in shadow against mirrored inputs and compare outputs, state, and cycles.
9. Emit deterministic reason codes and evidence bound to firmware and compiled scenario hashes.
10. Let the separate deployment gate decide whether activation is permitted.

## Test classes

- **Unit:** encoding, arithmetic edges, traps, permissions, diagnostics, schema/canonicalization, address allocation, policy truth tables, safe-state resolution, dynamics, and lifecycle rules.
- **Property:** decode robustness, valid encode/decode round trips, memory bounds, replay determinism, policy monotonicity, canonical compilation, stable allocation, and complete emergency coverage where dependency support permits.
- **Integration:** assemble/run, compile scenarios, dynamically add system elements through the editor API, reject invalid scenarios, validate safe/unsafe firmware, replay traces, verify hashes, compare shadow output, parse advisory checker findings, redact test credentials, and exercise lifecycle transitions.
- **QNX target:** process startup/IPC, timing observations, priority relationships, controller continuity when nonessential services die, watchdog behavior, and last-known-good rollback.
- **Fault injection:** illegal opcode, protected write, infinite loop, corruption, stale heartbeat, pressure anomalies, stuck feedback, electrical/continuity/clearance loss, unsafe update timing, and process failure.

## Trace and replay

A versioned execution trace must reconstruct starting machine/world state, input, instruction address and decoding, register/`HI`/`LO`/`PC` changes, memory and output writes, cycle count, traps, invariant evaluations, and ending state. Canonical state hashes exclude timestamps, PIDs, host memory addresses, and map iteration order.

Replay passes only when the same versioned inputs and artifacts produce the same event sequence and final canonical state. A trace is diagnostic evidence, not proof that untraced behavior is safe.

## Advisory AI checker evaluation

AI evaluation is separate from firmware validation and cannot affect deployment evidence. Versioned fixtures pair a bounded state snapshot with a written rule set and expected relevant rule IDs. Cases include clear violations, nominal state, insufficient information, stale hashes, prompt injection in string fields, malformed and oversized output, timeout, refusal, and backend loss.

The OpenAI test backend and hackathon `llama.cpp` backend consume the same local request and response types. Evaluation records parse success, rule-ID validity, cited-field validity, expected finding recall, false positives, `unknown` handling, latency, backend/model identity, prompt-contract version, and rule/snapshot hashes. No test assumes model output is deterministic, and no score is accepted as safety evidence. The local backend must also pass an offline startup and failure-isolation test with remote-provider fallback disabled.

## Current validation record

On 2026-09-19, the S32 decode/encode workspace passed `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo test --workspace`. Thirteen tests cover all 50 golden decoder/encoder
round trips, all 50 source-level instruction encodings, labels, pseudo-ops,
directives, symbol expressions, diagnostics, illegal and reserved encodings,
field-overflow rejection, cycle costs, representative arbitrary decoder inputs,
and CLI word parsing. The `check` and `assemble` commands were also exercised on
`examples/countdown.s32`, including binary output and an unknown-symbol failure.
A clean QNX SDP 8.0 Build 14 workspace cross-build for
`aarch64-unknown-nto-qnx800` passed. The operator reports that the earlier seed
ran on the Raspberry Pi 5; exact target evidence and execution of the new app
remain open.
