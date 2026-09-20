# Sentinel-32 dependency-aware plan

This is the canonical work plan. Status values are `complete`, `ready`, `blocked`, and `planned`. A task is complete only when every acceptance criterion has evidence. Safety review, bounded exploration, and timing observation retain their stated limits.

## Foundation

### BOOT-001 — Inspect and document repository

- **Category / status:** bootstrap / complete (2026-09-19)
- **Dependencies:** none
- **Description:** Establish durable project, architecture, safety, threat, validation, configuration, development, agent, context, decision, and planning documentation without overwriting useful existing work.
- **Acceptance:** Repository inventory and host baseline are recorded; `AGENTS.md`, `README.md`, `.agent/plan.md`, `.agent/context.md`, and canonical docs exist; goals, non-goals, terminology, claims, limitations, planned structure, decisions, assumptions, and open questions are explicit; all initial tasks have dependencies and acceptance criteria; available host checks pass.
- **Evidence:** `docs/development.md` and `docs/validation.md`; host fmt, clippy, and test passed on 2026-09-19.

### SCOPE-001 — Narrow AI to advisory safety-rule checking

- **Category / status:** requirements / complete (2026-09-19)
- **Dependencies:** BOOT-001
- **Description:** Remove AI firmware generation and repair from current requirements. Retain only an advisory comparison of bounded current-state snapshots with versioned written rules, using OpenAI for tests and authenticated deployment-server `llama.cpp` for the hackathon deployment.
- **Acceptance:** Project scope, architecture, configuration, safety model, threat model, validation method, demo, decisions, context, and plan agree on the narrowed role; AI has no approval, evidence, policy, activation, or output authority; OpenAI is test-only; the deployment backend is authenticated `llama.cpp` with no provider fallback; setup-guide work is planned.
- **Evidence:** `docs/project.md`, `docs/architecture.md`, `docs/configuration.md`, `docs/safety-model.md`, `docs/threat-model.md`, `docs/validation.md`, `docs/demo.md`, and `docs/decisions/DEC-011-advisory-ai-checker.md`.

### BUILD-001 — Prove QNX 8.0 AArch64 Rust build and execution

- **Category / status:** build / blocked
- **Dependencies:** BOOT-001
- **Description:** Use the licensed QNX-modified Rust compiler on a supported host to build hello world for `aarch64-unknown-nto-qnx800` and execute it on the Raspberry Pi 5.
- **Acceptance:** Real SDP environment is initialized; actual toolchain is linked with rustup; compiler/Cargo/SDP versions and `rustc --print cfg` are recorded; exact Cargo build succeeds; binary format is inspected; transfer and execution on QNX 8.0/Raspberry Pi 5 succeed; tested commit, image, output, and exit status are recorded without secrets.
- **Blocker:** The host cross-build now passes and the operator reports that the
  binary runs on the Pi. Exact target image, transfer/execution commands,
  captured output, and exit status remain required evidence. Details are in
  `docs/development.md`.

### BOOT-002 — Scaffold the Rust workspace

- **Category / status:** bootstrap / blocked
- **Dependencies:** BOOT-001, BUILD-001
- **Description:** Replace the seed package with only the crates justified by the proven architecture and establish host/QNX build configuration.
- **Acceptance:** Workspace boundaries match `docs/architecture.md`; core crates have no target-only dependency leakage; host check script or documented commands run cleanly; QNX workspace release build passes; `Cargo.lock` is committed; README/context reflect the real tree; no empty placeholder crates or scripts are added.
- **Progress:** The functional `sentinel-core`, `sentinel-scenario`, and
  `sentinel-app` crates are
  scaffolded, and both host checks and the QNX workspace release build pass.
  Completion remains blocked on the target-side evidence dependency in
  `BUILD-001`.

## ISA and VM

### ISA-001 — Specify S32 ISA v0

- **Category / status:** ISA / complete (2026-09-19)
- **Dependencies:** BOOT-001
- **Description:** Turn `docs/s32-isa.md` into the exact versioned architectural contract.
- **Acceptance:** All opcode/funct encodings, reserved fields, registers/reset, immediates, branch/jump/PC behavior, `HI/LO`, memory semantics, traps/precedence, cycle costs, assembler grammar/pseudo-ops, and canonical examples are specified; every instruction has golden encode/decode and execution vectors; incompatible-change policy is recorded and human reviewed.
- **Evidence:** `docs/s32-isa.md`; user human review accepted on 2026-09-19.

### ISA-002 — Implement the S32 assembler

- **Category / status:** ISA / blocked
- **Dependencies:** ISA-001, BOOT-002
- **Description:** Parse S32 source and produce canonical instruction bytes and diagnostics.
- **Acceptance:** Labels, comments, registers/aliases, literals, symbolic addresses, directives/pseudo-ops from ISA v0 work; diagnostics include line/column and actionable cause; encodings match all golden vectors; malformed/overflowing/unknown input fails without panic; host checks and QNX cross-build pass.
- **Progress:** The assembler implementation and CLI satisfy the functional
  acceptance criteria: all real instructions and golden vectors, labels,
  comments, aliases, checked literals/expressions, symbolic branches/jumps,
  directives, pseudo-ops, bounded output, and stable source diagnostics are
  covered by host tests and the QNX cross-build. Formal completion waits on its
  `BOOT-002` dependency.

### ISA-003 — Standardize assembly source presentation

- **Category / status:** ISA / complete (2026-09-19)
- **Dependencies:** ISA-001
- **Description:** Present S32 source in a consistent lowercase MIPS style, use the `.asm` file extension, and use the Markdown `asm` language fence for assembly examples.
- **Acceptance:** Checked-in assembly examples use the `.asm` extension, lowercase mnemonics and registers, and conventional operand spacing; the ISA contract defines the canonical presentation while retaining case-insensitive input compatibility; Markdown assembly is fenced as `asm`; examples still assemble and the host lane passes.
- **Evidence:** `examples/countdown.asm`, `examples/valve-controller.asm`, and `docs/s32-isa.md`; `cargo fmt --check`, strict workspace Clippy, all 45 workspace tests, the countdown assembler check, and the 203-step tank integration run passed on 2026-09-19.

### VM-001 — Implement the reference interpreter

- **Category / status:** VM / blocked
- **Dependencies:** ISA-001, BOOT-002
- **Description:** Execute S32 bit-exactly with region/capability enforcement and deterministic cycle accounting.
- **Acceptance:** Every instruction and trap matches ISA vectors; `R0`, `HI`, `LO`, `PC`, signed overflow, division, alignment, permissions, execute targets, stack bounds, MMIO, halt, and budgets are tested; decoder never panics for arbitrary words; execution cannot index outside mapped storage; no unsafe code; host checks and QNX cross-build pass.
- **Progress:** The functional acceptance criteria are implemented without
  dependencies or unsafe code: validated reset, sparse slots, region
  permissions, manifest capabilities, full S32 v0 execution, atomic typed
  traps, deterministic cycle budgets, and an interactive CLI that displays
  each stepped instruction, cycle charge, register change, memory write, next
  PC, and status. Host fmt, strict clippy, all 32 core tests, countdown CLI
  execution, and the QNX workspace release cross-build pass.
  Formal completion waits on the target-side evidence dependency in `BOOT-002`;
  run the current artifact using the `docs/development.md` VM-001 procedure.

### VM-002 — Implement versioned tracing and deterministic replay

- **Category / status:** VM / planned
- **Dependencies:** VM-001
- **Description:** Record sufficient deterministic events to replay machine and world execution.
- **Acceptance:** Versioned trace covers initial/final state, inputs, decoded instruction, register/`HI`/`LO`/`PC` deltas, memory/output writes, cycles, traps, and invariant evaluations; canonical hashes exclude nondeterministic process data; identical replay reproduces event sequence/final hash; malformed/version-mismatched traces fail cleanly.

## Scenario, safety, and digital twin

### SCEN-001 — Specify the versioned scenario schema

- **Category / status:** scenario / complete (2026-09-19)
- **Dependencies:** BOOT-001
- **Description:** Complete a typed declarative schema and its deterministic compilation contract.
- **Acceptance:** Telemetry, actuators, feedback, phases/transitions, typed rules, safe states, dynamics, faults, and layout are specified; canonicalization/hash domain/versioning/migration/limits are exact; code injection and nondeterminism are structurally impossible; errors for units, references, cycles, coverage, conflicts, totals, and duplicate keys are defined; deterministic/stable MMIO allocation is specified; examples include a valve and electrical switch; human review occurs.
- **Evidence:** `docs/scenario-schema.md`; user human review accepted on
  2026-09-19.

### SCEN-002 — Implement scenario compiler and generic runtime

- **Category / status:** scenario / blocked
- **Dependencies:** SCEN-001, BOOT-002
- **Description:** Validate source and compile immutable runtime bundles, policies, symbols, and deterministic dynamics.
- **Acceptance:** Compiler enforces all schema/semantic/coverage checks; canonical source and bundle hashes are stable; MMIO symbols are aligned/deterministic and stable mode is tested; generated S32 include is reproducible; runtime tick behavior is deterministic; arbitrary code payloads remain inert/rejected; host checks and QNX cross-build pass.
- **Progress:** The compiler and generic runtime satisfy the functional
  acceptance criteria, including bounded strict YAML parsing, typed semantic
  and coverage checks, domain-separated canonical hashes, exact bundle-byte
  hashing, clean and stable MMIO allocation, reproducible symbols, all dynamic
  and fault families, deterministic ticks, injection rejection, and an
  end-to-end VM harness where a hardware-only YAML inventory allocates MMIO and
  one persistent firmware invocation owns a pressure-controlled fill,
  ten-second hold, unload, and stop sequence. The runner changes physical
  telemetry between request pairs without restarting firmware and records every
  typed actuator write and instrument change. Runtime rules now block automatic
  progression in the same tick, abort-trigger transitions latch abort, dropped
  or frozen inputs age instead of refreshing, and supervised new attempts clear
  attempt-scoped runtime state. Host validation passes; the latest QNX link
  attempt was blocked by a local QNX license-lock timeout. Formal
  completion waits on `BOOT-002` and human review
  of the concrete v0 source forms and source-provenance clarification recorded
  in `docs/scenario-schema.md`.

### SCEN-003 — Implement default rocket launch scenario

- **Category / status:** scenario / blocked
- **Dependencies:** SCEN-002
- **Description:** Express the launch system entirely through the schema.
- **Acceptance:** Scenario includes both pressure channels, valve commands/feedback/timeouts, electrical sources, ignition continuity/arm/feedback, flight readiness, clearance/inhibits, sequence timer, phases/transitions, interlocks, latches, safe states, dynamics, and named faults; no rocket identifiers enter generic engine code; compilation and golden symbol/hash tests pass.
- **Progress:** Functional acceptance is implemented in
  `examples/rocket-launch-default.yaml`. It compiles to 23 deterministic MMIO
  slots and golden bundle hash
  `72975636c2b7c0db7931bc09defcd3ce27adc3ab98f76fa4d908128f94c25388`.
  `examples/rocket-controller.asm` and the `mission-run` command execute the
  nominal sequence as one persistent S32 invocation; assembly owns operational
  thresholds, waits, interlock checks, actuator requests, ignition feedback,
  shutdown, and abort commands while the runner records simulated supervisor
  approvals.
  Formal completion waits on `SCEN-002`.

### TWIN-001 — Validate rocket launch-pad state model

- **Category / status:** digital twin / blocked
- **Dependencies:** SCEN-003
- **Description:** Exercise the generic runtime as the deterministic launch-pad twin.
- **Acceptance:** Tests cover normal loading through completion; pressure drift/steps/over- and underpressure/staleness; valve delays/stuck feedback; bus loss; continuity/readiness/clearance loss; async hold/abort; countdown timing; abort irreversibility and supervised new attempt; repeated inputs yield identical traces.
- **Progress:** Host integration tests cover the listed normal, boundary,
  fault, timing, hold, abort, reset, and determinism cases. Generic runtime
  semantics now apply rule holds before automatic progression, latch requested
  abort transitions, and clear only attempt-scoped state on a supervised new
  attempt. Formal completion waits on `SCEN-003`.

### SAFE-001 — Implement invariant evaluation and safe-state output

- **Category / status:** safety / blocked
- **Dependencies:** VM-001, SCEN-002, TWIN-001
- **Description:** Evaluate typed policy independently of firmware and gate every actuator request.
- **Acceptance:** At least the eight policy families in `docs/safety-model.md` have boundary truth tables; deterministic precedence resolves global, latch, hazard, phase, and default policy; conflicts/missing coverage cannot publish; ordinary firmware cannot clear overrides; every output is accepted, overridden, or rejected with a stable reason; host and QNX checks pass with human review.
- **Progress:** `sentinel-safety` implements typed accepted, overridden, and
  rejected decisions; request validation; compiled-rule response handling;
  global/latch/hazard/phase/default precedence; bounded preservation; abort and
  authority containment; stable reason codes; and armed replacement denial.
  Host truth tables cover all eight policy families and advisory isolation.
  Formal completion waits on its blocked dependencies, a successful current
  QNX link, and human review of this trusted safety behavior.

### SAFE-002 — Implement manifest and static validation

- **Category / status:** safety / planned
- **Dependencies:** ISA-002, VM-001
- **Description:** Bind firmware to scenario identity, limits, capabilities, entry point, and required properties.
- **Acceptance:** Validation rejects illegal/unreachable encodings as applicable, invalid flow, unmapped/protected/unauthorized access, stack/program excess, budget excess, stale scenario hash, and unknown properties; system policy only reduces requested capability; safe and unsafe examples produce stable diagnostics; no candidate can create approval evidence.

### SAFE-003 — Implement scenario runner and bounded exploration

- **Category / status:** safety / planned
- **Dependencies:** SAFE-001, SAFE-002, TWIN-001
- **Description:** Run deterministic fixtures and finite exploration with canonical deduplication.
- **Acceptance:** Depth and state caps are mandatory and reported; nominal and fault paths cover phases and invariant edges; unsafe candidates yield reproducible concrete counterexamples; state canonicalization is order independent; termination at bounds is explicit; UI/docs never label results as formal proof.

### SAFE-004 — Implement evidence reports and hash binding

- **Category / status:** safety / planned
- **Dependencies:** SAFE-002, SAFE-003, VM-002
- **Description:** Produce immutable versioned validation reports for exact artifacts.
- **Acceptance:** Reports contain every field in `docs/safety-model.md`; source/bytecode/source-scenario/bundle hashes are domain separated and verified; mutation or cross-pair substitution fails; first/minimal counterexample policy is documented; deterministic decision and reason codes are separate from narrative; timing is labeled observed.

### SAFE-005 — Implement active/candidate/shadow/rollback lifecycle

- **Category / status:** safety / planned
- **Dependencies:** SAFE-004
- **Description:** Manage firmware and scenario publication/activation transactionally.
- **Acceptance:** State machine rejects invalid, replayed, stale-hash, and armed-time transitions; candidate has no output authority; shadow comparison is recorded; activation requires exact valid evidence; active and last-known-good compatible pairs survive failures; rollback cannot clear abort or bypass policy; transition tests and human review pass.

## Advisory AI safety check and UI

### MISSION-001 — Unify mission execution and advisory review

- **Category / status:** integration / blocked
- **Dependencies:** SCEN-002, AI-001
- **Description:** Present hardware-inventory and full-scenario operations as
  missions rather than separate tank and rocket products, and make the bounded
  advisory review available before any supported mission type.
- **Acceptance:** One `mission-run` command selects the validated adapter from
  the declared document schema; one `mission-advice` command accepts any valid
  AI-001 snapshot and configured written-rule set; output states that AI has no
  decision authority; provider failure cannot authorize, deny, or interrupt
  deterministic execution; existing example results remain reproducible.
- **Progress:** Both example missions complete through `mission-run`; the
  fixture-backed rocket snapshot completes through `mission-advice` with the
  authority banner; host lane and QNX target check pass. Formal completion
  waits on its blocked dependencies.

### AI-001 — Specify the advisory rule-check contract

- **Category / status:** AI / blocked
- **Dependencies:** SCOPE-001, SCEN-001, BOOT-002
- **Description:** Define provider-neutral, bounded types that compare one identified current-state snapshot with one versioned written rule set and return advisory findings.
- **Acceptance:** Input includes snapshot schema/version/hash, scenario identity/hash, observation time or tick, typed state fields, and rule-set version/hash; output is closed-schema and includes only valid rule IDs, `possible_violation`/`no_issue_observed`/`unknown`, cited state fields, bounded rationale, and provenance; missing or stale data can only yield `unknown` or a rejected response; the protocol exposes no firmware source, tool use, approval, policy mutation, activation, alarm suppression, or output-control capability; limits and typed timeout/refusal/transport/schema errors are defined; fixtures cover nominal, violation, insufficient-data, stale-hash, malformed, oversized, and injection cases.
- **Progress:** `sentinel-ai-check` and `docs/advisory-ai.md` implement the
  provider-neutral bounded snapshot, written-rule, finding, provenance, hash,
  limits, and error contract. Fixtures cover nominal, violation, unknown,
  stale/cross-paired hashes, malformed/oversized data, invented identifiers,
  invalid citations, and injection-shaped text. Production dependencies expose
  no control or safety authority. Formal completion waits on `BOOT-002`.

### AI-002 — Implement and evaluate the OpenAI test backend

- **Category / status:** AI / blocked
- **Dependencies:** AI-001
- **Description:** Add a development-only OpenAI adapter and use it to test the advisory checker contract against versioned fixtures.
- **Acceptance:** Current official API and structured-output support are verified when implemented; the adapter is excluded or rejected by the hackathon deployment profile; schema-valid findings parse through the same local types as fixtures and `llama.cpp`; nominal, violation, insufficient-data, injection, refusal, incomplete, timeout, oversized, and invalid-response cases are exercised; false positives, expected-rule recall, `unknown` handling, and latency are recorded; test model and prompt-contract versions are pinned in results; `OPENAI_API_KEY` never appears in artifacts, IPC, traces, or UI output; no control or deployment path waits on it.
- **Progress:** A compile-time `openai-test` adapter uses the current Responses API structured-output shape, `store: false`, bounded reads, redacted configuration, and the common local validator. Deployment rejects it. Parser tests cover valid output, refusal, and incomplete output; live evaluation remains blocked until an approved test model and credential are supplied.

### AI-003 — Integrate deployment-server `llama.cpp`

- **Category / status:** AI / blocked
- **Dependencies:** AI-001
- **Description:** Connect the advisory checker to a pinned GGUF model served by authenticated `llama.cpp` on the deployment server.
- **Acceptance:** The checker client connects to the authenticated deployment-server `llama.cpp` endpoint selected by DEC-013 and never falls back to OpenAI or another remote service; startup verifies the expected model hash, records model and `llama.cpp` versions and launch parameters, and proves structured-response compatibility; the backend uses the AI-001 request/response types and rejects stale hashes, invented rule IDs, invalid field citations, malformed output, and oversize output; checker timeout, process crash, and unavailable service produce visible non-authoritative status while deterministic control continues; deployment topology and target dependency status are recorded.
- **Progress:** The standard-library client implements bounded authenticated `/health`, `/props`, and `/v1/chat/completions` requests with strict JSON schema and local semantic validation. QNX testing exposed timeout and invalid-response behavior; the request now disables reasoning, bounds generated citations/rationales, seeds sampling, and distinguishes token-limit truncation. A captured live response first showed a Markdown JSON fence, which the client removes only as a complete outer wrapper. A later diagnostic capture showed an unconstrained singular `finding` object and proved that the deployed server ignored the direct `response_format.schema` field. The request now supplies the official nested `response_format.json_schema` wrapper plus the llama.cpp-native top-level `json_schema` constraint. Parser errors name the failing response stage, and an explicitly enabled diagnostic path can capture the bounded raw server envelope without the credential. Strict parsing and semantic validation remain unchanged. The launcher uses the requested Qwen/BLAS/CPU/context/bind settings, computes and records the first model hash, and rejects later mismatches. The CLI loads one gitignored mode-0600 `ai-llama-default.env` containing the localhost endpoint, absolute QNX paths, credential, and hash, with environment overrides. CLI, rocket fixtures, upload packaging, and user procedure are implemented. A rebuilt QNX client and successful live finding remain required.

### AI-004 — Validate advisory checker parity and isolation

- **Category / status:** AI / blocked
- **Dependencies:** AI-002, AI-003, SAFE-001, TWIN-001
- **Description:** Evaluate both backends on the same state/rule fixtures and prove that the checker remains outside deterministic safety authority.
- **Acceptance:** Versioned cases cover nominal state, clear violations for each relevant written-rule family, insufficient information, stale/cross-paired hashes, injection strings, malformed/oversized responses, refusal, timeout, and process loss; parse rate, rule-ID validity, field-citation validity, expected-rule recall, false positives, `unknown` handling, and latency are reported separately for OpenAI tests and deployment-server `llama.cpp`; stopping either backend leaves invariant evaluation, output gating, active control, and evidence unchanged; results are labeled advisory and do not enter deployment authority.
- **Progress:** Shared contract tests and deterministic rocket fixtures cover local validation and safety-output isolation. Backend comparison metrics and live process-loss evidence await both configured services.

### UI-001 — Implement text/NDJSON observability

- **Category / status:** UI / planned
- **Dependencies:** VM-002, TWIN-001, SAFE-001
- **Description:** Provide the authoritative headless event/state interface.
- **Acceptance:** Versioned bounded records expose source/instruction, registers, memory/output changes, scenario state, policy results, cycles, faults, and evidence references; slow/disconnected consumers cannot delay control; malformed requests cannot mutate authority; deterministic fixtures are documented.
- **Progress:** The human-readable full-scenario `mission-run` output correlates every
  completed assembly command frame with its first/last PCs, instruction and
  virtual cycle counts, scenario phase transition, active rules/faults, hold
  and abort state, supervisor event, firmware requests, and applied requests.
  It emits prominent `ISSUE` records for inhibit/hold/abort rules, distinct
  advisory `NOTICE` records, and an aggregate issue summary.
  Versioned NDJSON, register deltas, replay binding, and nonblocking transport
  remain.

### UI-002 — Implement browser operations dashboard

- **Category / status:** UI / planned
- **Dependencies:** UI-001, SAFE-004, AI-001
- **Description:** Visualize execution, scenario state, timing observations, evidence, divergence, faults, and counterexamples.
- **Acceptance:** Dashboard consumes the same versioned interface; distinguishes active/candidate/shadow state, deterministic evidence, and advisory AI findings; findings display backend, model, rule/snapshot identity, cited fields, and checker status; supports slow stepping and required machine state; escapes untrusted content; UI stop/crash leaves control running; claims retain correct limits.

### UI-003 — Implement Scenario Studio

- **Category / status:** UI / planned
- **Dependencies:** SCEN-002, UI-002
- **Description:** Edit and publish declarative scenarios through the compiler API.
- **Acceptance:** Registry, graph, phases, typed rules, safe-state matrix, fault timelines, address preview, validation errors, immutable publish flow, and armed lock exist; add a valve and electrical switch with feedback/safe states without Rust changes; unsafe/incomplete definitions cannot publish; layout cannot alter semantics accidentally.

## QNX, reliability, and delivery

### QNX-001 — Implement platform adapter

- **Category / status:** QNX / planned
- **Dependencies:** BUILD-001, BOOT-002
- **Description:** Add minimal safe wrappers for measured target needs.
- **Acceptance:** Timing, scheduling/priority, optional affinity, IPC, health, and shutdown needs are either implemented or explicitly deferred from observed cfg/APIs; unsafe is isolated and justified with `SAFETY` comments; return/pointer/length/lifetime handling is tested through safe wrappers; host fallback works; target smoke tests pass.

### QNX-002 — Split essential components into QNX processes

- **Category / status:** QNX / planned
- **Dependencies:** QNX-001, SAFE-001, SAFE-005, TWIN-001, AI-001
- **Description:** Deploy safety/output gate, active/shadow controllers, scenario runtime, verifier/gate, and the bounded advisory client at the required isolation boundaries.
- **Acceptance:** Versioned IPC and process startup/restart behavior work on target; active control has no synchronous AI/UI/verifier dependency; the target sends only versioned, size-bounded state snapshots to the configured advisory backend and accepts no control response from the checker; observed scheduling policy/priorities are recorded; killing the checker or UI preserves control; malformed IPC fails closed.

### QNX-003 — Implement watchdog and heartbeat supervision

- **Category / status:** QNX / planned
- **Dependencies:** QNX-002
- **Description:** Detect controller failure and remove output authority within configured target-observed bounds.
- **Acceptance:** Heartbeat sequence/freshness prevents replay; stale, hung, and killed controller cases trigger deterministic safe state; target timing is measured and labeled observational; last-known-good recovery is compatible and preserves abort state; monitor failure assumptions are documented.

### TEST-001 — Controlled fault-injection suite

- **Category / status:** testing / planned
- **Dependencies:** VM-002, SAFE-005, QNX-003, AI-004
- **Description:** Automate the fault matrix on host and relevant QNX processes.
- **Acceptance:** Every row in `docs/safety-model.md` plus corrupted bytes, stale heartbeats, advisory-checker/UI crash, misleading or stale AI findings, unsafe sensor sequence, and armed update has repeatable injection, detection owner, containment assertion, trace, and evidence; target-only coverage is clearly separated; no test is represented as formal proof.

### DOC-001 — Write the reproducible setup guide

- **Category / status:** documentation / blocked
- **Dependencies:** BUILD-001, BOOT-002, AI-002, AI-003
- **Description:** Document a fresh-machine path for host development, QNX cross-build/target execution, OpenAI-backed checker tests, and the authenticated deployment-server `llama.cpp` setup.
- **Acceptance:** The guide lists supported host and target prerequisites; exact install/build/test commands; QNX environment placeholders and evidence requirements; deployment-server `llama.cpp` build or install with a pinned version; approved GGUF acquisition without committing weights, license/checksum recording, and SHA-256 verification; authenticated deployment-server launch and structured-output smoke check; OpenAI test-only credential setup and redaction; configuration examples; service ordering; offline/no-remote-fallback verification; troubleshooting, cleanup, and expected outputs. A second person reproduces the applicable host and model-server steps, and unavailable QNX steps are labeled honestly.
- **Progress:** `docs/ai-user-guide.md` documents the requested deployment server, secret handling, hash verification, rocket sample, fixture/OpenAI modes, QNX commands, and failure checks. Reproduction by a second operator and live target/server evidence remain outstanding.

### DOC-002 — Reproducible demo and safety-case summary

- **Category / status:** documentation / planned
- **Dependencies:** AI-004, UI-003, TEST-001, DOC-001
- **Description:** Package the three-minute narrative, recovery, evidence, and honest limitations using the setup guide.
- **Acceptance:** A fresh supported environment can reproduce the build/deploy/demo; safe and unsafe validation, advisory written-rule finding, shadow, activation, corruption, checker loss, and controller-hang cases work; deployment-server `llama.cpp` and OpenAI test evidence are clearly distinguished; exact tested versions and hashes are recorded; safety-case summary maps claims to mechanisms/tests and discloses gaps.
- **Progress:** `docs/demo.md` now provides the implemented QNX CLI sequence
  for standalone S32 source inspection, validation, encoding, binary output,
  completion and interactive instruction-level emulation; service health;
  prepared rocket and tank advisory inputs; both persistent assembly missions;
  expected output cues; and AI-service-loss isolation. It explicitly
  distinguishes the synthetic `95000` rocket
  snapshot value from the controller's live `50000` target. The remaining
  acceptance items depend on the unimplemented lifecycle, shadow, corruption,
  watchdog, and evidence work listed in this task's dependencies.

## Critical path

```text
BOOT-001 -> BUILD-001 -> BOOT-002 -> ISA-001 -> ISA-002 + VM-001
-> SCEN-001 -> SCEN-002 -> SCEN-003 + TWIN-001 -> SAFE-001 + SAFE-002
-> SAFE-003 + VM-002 -> SAFE-004 -> SAFE-005 -> QNX-001 -> QNX-002
-> QNX-003 -> TEST-001 -> DOC-002

SCOPE-001 + SCEN-001 + BOOT-002 -> AI-001 -> AI-002 + AI-003 -> AI-004
AI-001 -> QNX-002
AI-002 + AI-003 + BUILD-001 + BOOT-002 -> DOC-001
AI-004 + DOC-001 + UI-003 + TEST-001 -> DOC-002
```

The implemented `ISA-002`, `VM-001`, and `SCEN-002` work remains formally
blocked on `BOOT-002` while target evidence for `BUILD-001` is captured.
`SCEN-002` also awaits human review of its schema clarification. The
`SCEN-003`/`TWIN-001`/`SAFE-001`/`AI-001` host functionality is implemented but
retains the dependency and review blockers recorded on those tasks. The next
unimplemented critical-path work is `SAFE-002` or `VM-002`.

## Open project questions

- How will this environment transfer to and execute commands on the QNX Raspberry Pi 5, and which target image/version is authoritative?
- What approved SHA-256 and license record apply to the selected `Qwen2.5-1.5B-Instruct-Q4_K_M.gguf` file?
- Which OpenAI test model will be pinned after current structured-output support is verified during `AI-002`?
