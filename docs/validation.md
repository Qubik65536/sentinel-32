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

On 2026-09-19, the hardware-manifest workspace passed `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo test --workspace`. Forty-four tests cover all 50 golden decoder/encoder
and source-assembly vectors; assembler syntax and failures; every interpreter
operation family; signed and unsigned arithmetic; branch, jump, and link
behavior; reset and `R0`; sparse mappings, permissions, capabilities, and stack
bounds; stable trap classes and precedence; atomic fault behavior; full-cost
cycle-budget refusal; and end-to-end countdown assembly and execution. The
scenario tests cover strict source rejection, typed schema/semantic/coverage
errors, stable canonical hashes, presentation separation, clean and stable MMIO
allocation, reproducible symbols, every dynamic operation and fault family,
one-shot faults, priority, deterministic phase progression, and firmware access
to hardware-only YAML-allocated MMIO. The integration test observes the two
assembly-issued actuator requests in order and verifies the final request.

The host `run examples/countdown.s32 9` case halts after nine steps and cycles
with `PC=0x00000014` and `R1=0`. Budget 8 is rejected before `HALT` with exit
code 2. A prior clean QNX SDP 8.0 Build 14 workspace release cross-build for
`aarch64-unknown-nto-qnx800` produced an AArch64 QNX PIE with SHA-256
`50a2c8546e1f256f85d7429b7f1b3e68409559fbc13cdcab6b88cb6824146aae`.
The current hardware manifest compiles to bundle hash
`e022adf0b5591401bb229cf0b79b3772aa142cacf9e823167ea93015b8d2e196`.
Its host run records `open` followed by `closed` and halts after 17 instructions
and 19 virtual cycles. The current QNX link was attempted but the local QNX
license lock timed out, so the earlier binary hash is not evidence for this
source. QNX cross-build and Raspberry Pi 5 execution remain to be captured; the
exact operator procedure is in `docs/development.md`.
