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

On 2026-09-19, the rocket-twin and advisory-contract workspace passed
`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
and `cargo test --workspace`. Seventy default-feature tests cover all 50 golden decoder/encoder
and source-assembly vectors; assembler syntax and failures; every interpreter
operation family; signed and unsigned arithmetic; branch, jump, and link
behavior; reset and `R0`; sparse mappings, permissions, capabilities, and stack
bounds; stable trap classes and precedence; atomic fault behavior; full-cost
cycle-budget refusal; externally driven telemetry/feedback updates that cannot
alter actuator or protected slots; and end-to-end countdown execution. The
scenario tests cover strict source rejection, typed schema/semantic/coverage
errors, stable canonical hashes, presentation separation, clean and stable MMIO
allocation, reproducible symbols, every dynamic operation and fault family,
one-shot faults, priority, deterministic phase progression, and firmware access
to hardware-only YAML-allocated MMIO. The integration test observes the two
assembly-issued actuator requests in order and verifies the final request.

The default rocket scenario compiles to 23 MMIO slots and bundle hash
`72975636c2b7c0db7931bc09defcd3ce27adc3ab98f76fa4d908128f94c25388`.
Its tests cover loading through completion, exact pressure edges at 49,999,
50,000, 70,000, 70,001, and 90,000 normalized milli-units, drift,
overpressure, stale pressure, stuck feedback, bus loss, continuity/readiness/
clearance loss, countdown timing, asynchronous hold and abort, persistent abort,
supervised new-attempt reset, and repeatable events/final state. Safety tests
cover accepted, overridden, rejected, invalid, inhibited, abort-latched, and
authority-lost outputs; abort-over-hazard precedence; armed replacement denial;
and advisory-result isolation. Advisory contract fixtures cover nominal,
possible violation, insufficient data, stale/cross-paired hashes, invented rule
and field citations, malformed and oversized output, and injection-shaped text.
Provider tests cover endpoint validation, credential redaction, provenance
wrapping, token-limited completion detection, and post-generation rejection of invalid citations. The feature-
gated OpenAI parser tests cover structured output and refusal, and both the app
and AI crate pass clippy/tests with `openai-test` enabled. The rocket AI fixture
passes through the CLI with exact hashes and two expected advisory findings.
The strict config test covers parsing plus unknown/duplicate-key rejection. A
launcher smoke test confirms a mode-0600 default file,
automatic SHA-256 recording, and rejection after the model bytes change.

The assembly-controlled nominal rocket integration test starts one persistent
S32 invocation and verifies loading, stabilization, simulated supervisor
approvals, terminal count, ignition feedback, completion, and confirmed
closed/safe shutdown. The observed host run uses 19 scenario ticks, 248
instructions, and 391 virtual cycles. These bounded observed counts are
regression evidence, not WCET.
Each of the 19 command frames records a nonzero assembly instruction and
virtual-cycle count plus aligned first/last PCs, allowing an operator to
correlate firmware execution with triggered rules and hold/abort state.

The host `run examples/countdown.asm 9` case halts after nine steps and cycles
with `PC=0x00000014` and `R1=0`. Budget 8 is rejected before `HALT` with exit
code 2. A prior clean QNX SDP 8.0 Build 14 workspace release cross-build for
`aarch64-unknown-nto-qnx800` produced an AArch64 QNX PIE with SHA-256
`50a2c8546e1f256f85d7429b7f1b3e68409559fbc13cdcab6b88cb6824146aae`.
The current tank hardware manifest compiles to bundle hash
`b96e97412c6762b20b78a04f7de2b3450d668f2a8f030022479a471a7683681c`.
One host invocation executes 203 instructions over 257 virtual cycles: it fills
to 50000, counts a ten-second hold in firmware, unloads to zero, closes both
valves, and halts. The simulator updates read-only pressure without restarting
the machine. The current QNX link was
attempted but the local QNX
license lock timed out, so the earlier binary hash is not evidence for this
source. The expanded five-crate workspace compiled its QNX-target libraries on
2026-09-19, then the final app link again failed after the QNX license lock
timed out. QNX linking and Raspberry Pi 5 execution remain to be captured; the
exact operator procedure is in `docs/development.md`.
The later AI-provider build also compiled the QNX target objects and failed at
the same licensed final-link step. Live QNX `llama-server` requests have since
reached the service, but no response has yet passed the complete advisory
contract. No live OpenAI result is claimed in this record.

The host lane passes 72 tests after adding the deployed
`llama-server` compatibility case. That case reproduces a successful `stop`
completion with schema-valid content inside one whole-response Markdown JSON
fence. The adapter removes that wrapper and then runs the unchanged strict
response parser and semantic validator. A rebuilt client has not yet completed
a live QNX advisory check, so structured-response compatibility remains
partially observed rather than validated end to end.
The QNX-modified toolchain also completed `cargo +qnx800 check --workspace
--target aarch64-unknown-nto-qnx800 --release` for this source revision.
Parser tests now distinguish malformed completion envelopes from malformed
findings JSON. The optional bounded diagnostic capture is an operator aid, not
safety evidence, and remains disabled unless explicitly configured.
The request-shape regression test verifies that the schema is present in both
the nested OpenAI-compatible `response_format.json_schema.schema` location and
llama.cpp's top-level `json_schema` location. The previously used direct
`response_format.schema` field is absent.
