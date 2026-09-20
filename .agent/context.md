# Repository context

Last updated: 2026-09-20 during Ratatui implementation and QNX validation.

## Current implementation

Control operations follow two accepted design principles: an operator starts
one persistent S32 invocation that runs until completion or bounded failure,
and `sentinel.hardware/v0` YAML contains hardware inventory only. Operational
thresholds, waits, sequencing, loops, and actuator requests belong to assembly.
The operator interface now treats both hardware-inventory and full-scenario
operations as missions: `mission-run` selects the validated adapter from the
declared schema. `mission-advice` applies the same bounded advisory contract to
any mission snapshot and written-rule set before a human considers execution;
the deterministic system retains all proceed and output authority.

The repository has a Rust workspace. `sentinel-core` implements
typed S32 decode/encode, the source assembler, validated manifests and sparse
memory, and complete S32 v0 reference execution with permissions, capabilities,
atomic traps, and deterministic cycle budgets. The CLI can run standalone S32
source to completion or pause it interactively after each instruction while
showing decoded operations, cycle use, state changes, and memory writes.
`sentinel-scenario` implements
bounded strict YAML parsing, typed validation, canonical bundle compilation,
stable MMIO allocation, generated symbols, and a generic deterministic
runtime. The app exposes VM and scenario commands, including an end-to-end
firmware harness with YAML-allocated MMIO. The former formal completion
blockers and unfinished roadmap are retained only as historical context.

`sentinel-app tui` now provides Assemble, Run, Mission, and Advisory views.
The application pins Ratatui 0.29.0 without optional backends and implements a
safe ANSI/`stty` backend that contains no FFI or `unsafe` code. It supports
keyboard navigation, resize polling, compact terminals, bounded source
editing with save-conflict detection, assembly symbols/encodings, coherent VM
stepping and run/pause, tank/rocket mission playback, and asynchronous advisory
checks with a permanent no-authority banner. It consumes typed Rust results and
does not parse CLI output.

`examples/rocket-launch-default.yaml` is the default normalized launch-pad
digital twin. It compiles to 23 MMIO slots and exercises both pressure
channels, fill/main/vent valves and feedback, electrical sources, ignition,
readiness and clearance, phases, countdown, safe states, deterministic faults,
hold, abort, and supervised new attempts. `sentinel-safety` implements typed
accepted/overridden/rejected output decisions, deterministic safe-state
precedence, authority-loss containment, and armed replacement denial.
`sentinel-ai-check` implements the bounded and hash-bound snapshot, written
rule, finding, provenance, and response-validation contract plus a standard-
library authenticated `llama.cpp` client and feature-gated OpenAI test client. The two crates
have no production dependency between them, and an integration test proves
that an advisory finding cannot alter output decisions.
The app loads the gitignored mode-0600 `config/ai-llama-default.env`, with
process environment taking precedence. The single file contains the localhost
endpoint, absolute QNX paths, credential, and model hash. The server launcher
computes and records the first GGUF hash there and rejects a later mismatch
without logging the stored credential. The repository tracks only an example.
An initial live QNX request reached the local Qwen server but exceeded the
30-second client timeout; a later 512-token request completed in 31.643 seconds
with an invalid response. A direct request captured a `stop` completion whose
content was valid structured JSON wrapped in a Markdown JSON fence. The client
now disables reasoning, tightens output bounds, reports non-stop finish reasons
as incomplete, and removes only a complete outer JSON fence before applying
strict local parsing and semantic validation. This fix still needs a rebuilt
QNX run. A subsequent live run still returned an invalid provider response;
the client now reports which response stage failed and supports an explicit,
bounded raw-response capture path for diagnosis. Capture is disabled by
default, and the client does not add the provider credential to it.
The captured response contained an unconstrained singular `finding` object,
proving that the deployed server ignored the adapter's direct
`response_format.schema` field. The request now sends the official nested
`response_format.json_schema` wrapper and the llama.cpp-native top-level
`json_schema` constraint while preserving strict local validation.

Canonical S32 source presentation uses lowercase MIPS-style mnemonics,
directives, and registers with spaced operands. Markdown source examples use
the `asm` fence, and standalone source files use the `.asm` extension so editors
can select assembly highlighting. The assembler remains case-insensitive for
source compatibility.

Current requirements give AI one advisory function: compare a bounded current-state snapshot against versioned written safety rules and emit structured, rule-linked findings. OpenAI is test-only. On QNX, the deployment client connects through localhost to a pinned Qwen2.5 1.5B GGUF served by authenticated `llama.cpp` on the same host, with no remote fallback. AI does not generate firmware or participate in deterministic validation, evidence, activation, safety policy, or output control. This scope and topology are recorded by `SCOPE-001`, DEC-011, and DEC-013.

## Navigation

- `Sentinel-32-Project-Kickstarter.md`: initial brief and historical source used for bootstrap; current facts should migrate to canonical docs.
- `AGENTS.md`: binding repository workflow and safety constraints for coding agents.
- `README.md`: project entry point and honest current status.
- `.agent/plan.md`: task dependencies, status, and acceptance criteria.
- `docs/project.md`: scope, non-goals, vocabulary, success boundary.
- `docs/architecture.md`: implemented boundaries and TUI integration.
- `docs/tui.md`: Ratatui layouts, implementation, state flow, and visual rules.
- `docs/tui-user-guide.md`: host and QNX launch, complete key map, workflows,
  advisory configuration, and terminal recovery.
- `docs/development.md`: host/QNX workflow and historical target audit.
- `docs/configuration.md`: intended configuration for core services and the advisory checker backends.
- `docs/advisory-ai.md`: exact provider-neutral snapshot, written-rule, finding, hash, limit, and authority contract.
- `docs/ai-user-guide.md`: deployment-server, QNX client, rocket sample, OpenAI test, and failure-isolation procedure.
- `docs/demo.md`: reproducible TUI walkthroughs for assembly, VM execution,
  rocket/tank missions, advisory checks, and AI failure isolation, followed by
  the scriptable QNX CLI demonstration.
- `docs/s32-isa.md`: accepted and implemented ISA v0 contract.
- `docs/scenario-schema.md`: accepted schema v0 contract and implemented concrete forms.
- `crates/sentinel-core`: portable S32 ISA, assembler, VM, memory, traps, cycles, and tests.
- `crates/sentinel-scenario`: strict parser, compiler, canonical artifacts, MMIO symbols, and deterministic runtime.
- `crates/sentinel-safety`: deterministic safe-state resolution and output request decisions.
- `crates/sentinel-ai-check`: provider-neutral advisory contract, validators, `llama.cpp` client, and feature-gated OpenAI test client.
- `crates/sentinel-app`: host/QNX CLI and Ratatui interface for VM, mission,
  and advisory workflows.
- `examples/sample-analysis.asm`: standalone sample-analysis demonstration using a
  stack buffer, subroutine call, loop, comparisons, loads/stores, and HI/LO. It
  deterministically produces sum 66, maximum 25, threshold count 3, average
  13, and remainder 1 in 91 instructions and 112 virtual cycles.
- `examples/lab-scenario.yaml`: hardware-only tank-pressure and two-valve MMIO inventory with no controller actions.
- `crates/sentinel-scenario/src/test-scenario.yaml`: internal full-schema compiler fixture.
- `examples/valve-controller.asm`: commented firmware that fills to a pressure target, holds ten simulated seconds, unloads, and closes both valves.
- `examples/rocket-launch-default.yaml`: full declarative normalized rocket launch-pad scenario.
- `examples/rocket-controller.asm`: persistent S32 controller for the nominal
  normalized rocket run; firmware owns thresholds, waits, interlock checks,
  actuator sequencing, ignition feedback, shutdown, and its abort path.
- `mission-run` prints each completed full-scenario command frame with its first and
  last assembly PCs, instruction count, virtual-cycle count, scenario
  transition, active deterministic rules, hold/abort state, requested outputs,
  and applied outputs.
  Separate uppercase `ISSUE` lines identify inhibit/hold/abort rules, `NOTICE`
  lines retain advisory-only rules, and `ISSUE-SUMMARY` aggregates both.
- `mission-advice` applies the provider-neutral advisory contract to any
  bounded mission snapshot and configured written-rule set before an operator
  considers starting `mission-run`; it has no proceed or output authority.
- `examples/ai/tank-proposed-action-snapshot.json` and
  `tank-written-rules.json` demonstrate a second mission type with a
  deliberately concerning proposed action; the proposal cannot reach mission
  outputs.
- `docs/safety-model.md`: claims, invariant families, containment, evidence.
- `docs/threat-model.md`: assets, untrusted boundaries, abuse cases, controls.
- `docs/validation.md`: validation layers and current results.
- `docs/decisions/`: accepted bootstrap architecture decisions and revisit triggers.
- `docs/decisions/DEC-015-ratatui-interface.md`: accepted TUI roadmap reset.
- `docs/decisions/DEC-012-single-invocation-control.md`: operator-started firmware remains active until its bounded operation completes.

## Verified environment

Host checks use upstream Rust 1.98.1 on `x86_64-unknown-linux-gnu`. Ratatui TUI
unit/buffer tests and a real host PTY render/input/restore smoke pass. QNX SDP
8.0 Build 14 and linked toolchain `qnx800` (Rust 1.85.1-dev) complete the
AArch64 QNX 8.0 release cross-build with the custom ANSI backend. The release
binary has SHA-256
`6f5b7807f4ab4b3f131b0f1f10857313670440bf995b7a68c2e063235f0da324`.
That exact artifact and fixtures were uploaded and executed on QNX 8.0.0 image
`2026/06/05-16:21:14EDT` on the Raspberry Pi 5. Through an SSH pseudo-terminal,
the TUI rendered Assemble/Run/Mission, accepted batched navigation and step
keys, redrew after 80x24 to 70x18 resize, and restored cursor, alternate screen,
and terminal state on `q`. The deployed CLI also validated the standalone
fixture that preceded the current sample-analysis replacement.
The TUI advisory worker resolves its default rules and AI configuration from
the same demo root, so launching from QNX `release/` does not require a
relative `config/` directory or a redundant `S32_AI_RULES_PATH` export.
The documented QNX deployment root is `/data/home/qnxuser/sentinel-32`, with
release, example, and artifact subdirectories. `scripts/qnx-pi-upload.sh`
transfers an already-built release and fixtures to `qnxuser@qnxpi59.local`;
`scripts/qnx-build-upload.sh` runs the full host lane, host and QNX release
builds, scenario artifact generation, and that upload in one command. Target
testing remains a separate operator SSH session.

## Next work

The TUI roadmap is complete. Preserve the CLI for automation and diagnosis and
keep later changes within the recorded UI and safety boundaries. Copy the
replacement standalone example to the QNX demo tree during the next
authenticated target session; the noninteractive refresh attempt was rejected
by target authentication.

## Working tree note

The roadmap reset began with pre-existing uncommitted AI provider work in the
tree. Preserve unrelated work.
