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

## Implemented laboratory pipeline

1. Parse bounded source and assemble it with line-specific diagnostics.
2. Produce canonical instruction bytes and decode each executed instruction.
3. Validate memory mappings, manifest capabilities, entry point, stack bounds,
   and a positive virtual-cycle budget.
4. Run the VM directly or pair the firmware with a compiled mission document.
5. For missions, update the deterministic software twin and pass each firmware
   request through the typed output-decision API.
6. Record steps, cycles, state changes, requests, applied outputs, rules,
   faults, transitions, halt, and traps for display.

This pipeline does not claim static control-flow validation, bounded
state-space exploration, replayable traces, shadow execution, deployment
approval, or activation lifecycle management.

## Test classes

- **Unit:** encoding, arithmetic edges, traps, permissions, diagnostics,
  schema/canonicalization, address allocation, policy truth tables, safe-state
  resolution, dynamics, and advisory response validation.
- **Robustness:** arbitrary decode input, encode/decode round trips, memory
  bounds, canonical compilation, stable allocation, bounded parsers, and
  deterministic repeated mission inputs.
- **Integration:** assemble/run, compile scenarios, reject invalid scenarios,
  execute tank and rocket missions, apply output decisions, parse advisory
  findings, and redact credentials.
- **TUI:** reducer/session tests, Ratatui buffer snapshots for wide and compact
  layouts, bounded scrolling/content, terminal restoration, and provider
  failure without event-loop or runner failure.
- **QNX target:** existing CLI smoke checks plus TUI launch, keyboard input,
  resize, normal exit, and terminal restoration through an allocated console
  or SSH pseudo-terminal.

## Advisory AI checker validation

AI evaluation is separate from firmware validation and cannot affect deployment evidence. Versioned fixtures pair a bounded state snapshot with a written rule set and expected relevant rule IDs. Cases include clear violations, nominal state, insufficient information, stale hashes, prompt injection in string fields, malformed and oversized output, timeout, refusal, and backend loss.

The OpenAI test backend and hackathon `llama.cpp` backend consume the same
local request and response types. Deterministic tests validate rule IDs, cited
fields, hashes, limits, refusal/error mapping, and authority isolation. Live
backend parity metrics are outside the current roadmap. No test assumes model
output is deterministic, and no score is accepted as safety evidence.

## Current validation record

On 2026-09-19, the rocket-twin and advisory-contract workspace passed
`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
and `cargo test --workspace`. Seventy default-feature tests cover all 50 golden decoder/encoder
and source-assembly vectors; assembler syntax and failures; every interpreter
operation family; signed and unsigned arithmetic; branch, jump, and link
behavior; reset and `R0`; sparse mappings, permissions, capabilities, and stack
bounds; stable trap classes and precedence; atomic fault behavior; full-cost
cycle-budget refusal; externally driven telemetry/feedback updates that cannot
alter actuator or protected slots; and end-to-end standalone sample-analysis
execution. The scenario tests cover strict source rejection, typed
schema/semantic/coverage
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
Human-readable output separates advisory `NOTICE` events from `ISSUE` events
that inhibit, hold, or abort, and emits an aggregate `ISSUE-SUMMARY`.
The common `mission-run` CLI is exercised with both the hardware-inventory tank
mission and the full-scenario rocket mission. `mission-advice` is exercised
with the same provider-neutral fixture contract and prints its lack of decision
authority before the structured findings.
The tank proposed-action fixture is hash-valid and represents both valves open;
its written rules bind that proposal and current pressure for a second mission
type. Upload packaging includes both files.

The host `run examples/sample-analysis.asm 256` case halts after 91 instructions and
112 cycles with `PC=0x00000070`, `HI=1`, `LO=13`, sum `R2=66`, maximum
`R3=25`, threshold count `R7=3`, average `R8=13`, remainder `R9=1`, and a
restored stack pointer. Budget 111 is rejected before `HALT` with exit code 2.
A prior clean QNX SDP 8.0 Build 14 workspace release cross-build for
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

The host lane passes 73 tests after adding the deployed
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

On 2026-09-20, the TUI implementation added Ratatui 0.29.0 with default
features disabled and a safe ANSI/`stty` backend. App tests cover all four
views, the compact layout, source save conflicts, VM register deltas, typed
mission frames, key decoding, ANSI output, and rejection of cell control
characters. A real host pseudo-terminal rendered the Assemble screen, accepted
`q`, exited zero, and restored the alternate screen, cursor, and shell mode.
The QNX-modified Rust 1.85.1 compiler completed the app target check and linked
an AArch64 QNX 8.0 release executable. The linked artifact and fixtures were
prepared for upload with SHA-256
`6f5b7807f4ab4b3f131b0f1f10857313670440bf995b7a68c2e063235f0da324`.
The exact artifact ran on the Raspberry Pi 5 QNX 8.0.0 image dated
`2026/06/05-16:21:14EDT`. An allocated SSH terminal rendered Assemble, Run, and
Mission; accepted batched navigation and stepping; completed the standalone
program and 19-frame rocket mission; and redrew after an 80x24 to 70x18
terminal change.
`q` emitted cursor/alternate-screen restoration and closed the SSH session
normally. The deployed SHA-256 matched the uploaded provenance file, and the
target CLI successfully checked the deployed standalone example.
The final default-feature host lane contains 81 passing tests: 12 advisory, 15
app/TUI, 31 core, 7 safety, and 16 scenario tests, plus empty doc-test suites.
The TUI advisory path was subsequently checked with `S32_AI_RULES_PATH`
absent: it resolved the displayed rule set and deployment config from the demo
tree, reached the provider, and reported the expected transport failure rather
than a missing-configuration error. Repeated requests reuse the validated
configuration while retaining environment-variable precedence.
The same unset-override smoke passed on the deployed QNX artifact: the
Advisory view displayed the rocket rule hash and advanced to `provider request
running` instead of emitting the former missing-rule error.

The renamed `examples/sample-analysis.asm` replacement passed the full 81-test
host lane and `cargo +qnx800 check --workspace --target
aarch64-unknown-nto-qnx800 --release`. A noninteractive copy to the existing
QNX demo tree was rejected by target authentication, so execution of this
replacement source on the Raspberry Pi remains unavailable until the next
authenticated target session. The already validated VM binary did not change;
only the fixture, tests, and documentation changed in this task.
