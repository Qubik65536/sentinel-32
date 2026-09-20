# Sentinel-32 dependency-aware plan

This is the canonical work plan. On 2026-09-20 the unfinished verifier,
browser, lifecycle, process-splitting, watchdog, evidence, provider-evaluation,
and delivery roadmap was retired in favor of one operator-facing Ratatui TUI.
Existing implementations remain available, but retired tasks are not completion
requirements and must not be presented as planned safety capabilities.

Status values are `complete`, `ready`, `planned`, `blocked`, and `retired`. A
task is complete only when every acceptance criterion has evidence. Safety
review, bounded exploration, and timing observations retain their stated
limits.

## Preserved baseline

### BASE-001 — Preserve the implemented Sentinel-32 laboratory

- **Category / status:** baseline / complete (2026-09-20)
- **Dependencies:** none
- **Description:** Preserve the implemented ISA, assembler, VM, scenario
  compiler/runtime, deterministic output decisions, advisory checker contract,
  mission examples, and existing command-line workflows as the foundation for
  the TUI.
- **Acceptance:** Existing source and tests remain; the TUI consumes typed Rust
  APIs rather than parsing human-readable CLI output; the command-line commands
  remain available for scripts and target diagnosis; existing safety and AI
  authority boundaries are unchanged.
- **Evidence:** `sentinel-core`, `sentinel-scenario`, `sentinel-safety`,
  `sentinel-ai-check`, and `sentinel-app`; the host lane passed before this
  roadmap reset as recorded in `docs/development.md`.

The completed historical task IDs are `BOOT-001`, `SCOPE-001`, `ISA-001`,
`ISA-003`, and `SCEN-001`. Functional work from `BOOT-002`, `ISA-002`,
`VM-001`, `SCEN-002`, `SCEN-003`, `TWIN-001`, `SAFE-001`, `MISSION-001`, and
`AI-001` is preserved by `BASE-001`; their former formal completion blockers no
longer drive the roadmap.

### ROADMAP-001 — Replace the unfinished roadmap with a Ratatui TUI

- **Category / status:** planning / complete (2026-09-20)
- **Dependencies:** BASE-001
- **Description:** Make a visual terminal application the remaining product
  objective and remove the browser dashboard, Scenario Studio, NDJSON service,
  lifecycle, process decomposition, watchdog, bounded exploration, evidence
  report, static verifier, provider parity, and setup-guide programs from the
  active plan.
- **Acceptance:** The project, architecture, context, and plan agree on the new
  scope; retired work is named; unresolved dependency and target assumptions
  are explicit; all new implementation work has dependencies and acceptance
  criteria.
- **Evidence:** This plan, `docs/project.md`, `docs/architecture.md`, and
  `.agent/context.md`.

## Ratatui TUI

### TUI-008 — Scroll and decode Run panes

- **Category / status:** UI / complete (2026-09-20)
- **Dependencies:** TUI-005
- **Description:** Make the Run view's assembly and register panes independently
  scrollable by focus and show each register in hexadecimal, ASCII, and unsigned
  decimal forms.
- **Acceptance:** `Tab`/`Shift+Tab` focus selects which supported pane receives
  `j`/`k` or arrow scrolling; assembly and register offsets are independent and
  bounded; all 32 registers plus `HI`, `LO`, and `PC` expose all three forms;
  printable ASCII is decoded deterministically with nonprintable bytes marked;
  alignment uses the widest ASCII field; compact terminals remain usable;
  help, guide, design, demo, context, tests, host validation, and QNX
  cross-check agree with the behavior.
- **Evidence:** `RunnerView` stores independent bounded assembly/register
  offsets and routes scrolling by focused pane. Wide and 80-column buffer tests
  cover aligned hex/ASCII/unsigned-decimal output; reducer tests cover
  independent movement and both bounds. Help, user guide, design, demo,
  context, and validation records describe the controls and decoding. Format,
  strict Clippy, all 83 host tests, QNX release check, and licensed QNX final
  link pass. The linked AArch64 QNX PIE has SHA-256
  `7ec179df1a3f805215f366a9bff7a117d3afae5952faf74133915f27691a177c`;
  target execution remains to be refreshed in an authenticated session.

### DEMO-001 — Replace the minimal standalone assembly demonstration

- **Category / status:** demonstration / complete (2026-09-20)
- **Dependencies:** BASE-001, TUI-005
- **Description:** Replace the former `examples/countdown.asm` with the richer,
  appropriately named `examples/sample-analysis.asm`, making the visual
  runner's stack, memory, register, branch, function-call, and HI/LO displays
  useful.
- **Acceptance:** The example assembles and halts in the standalone VM; its
  documented final values match execution; TUI/CLI tests and canonical demo,
  ISA, development, validation, README, context, and upload references use the
  new filename and no longer claim the former five-word, nine-step countdown
  behavior; host validation passes.
- **Evidence:** `examples/sample-analysis.asm`; deterministic VM regression in
  `sentinel-core`; updated README, ISA, TUI, demo, development, validation, and
  context documentation. `check` reports 188 bytes; the host CLI halts after
  91 instructions/112 cycles with the documented results; budget 111 fails
  closed; formatting, strict Clippy, all 81 host tests, and the QNX workspace
  release check pass. Refreshing the source fixture on the target was attempted
  but remains unavailable without an authenticated SSH session.

### TUI-001 — Prove the terminal dependency and QNX backend

- **Category / status:** dependency / complete (2026-09-20)
- **Dependencies:** BASE-001
- **Description:** Select and pin a Ratatui release and terminal backend that
  work with the repository's supported host compiler and the QNX custom
  toolchain. The selected combination is Ratatui 0.29.0 with default features
  disabled and Sentinel-32's safe ANSI/`stty` backend.
- **Acceptance:** Record the selected Ratatui/backend versions, feature flags,
  licenses, dependency tree, and Rust minimum; prove a minimal alternate-screen
  application can render, receive keys and resize events, and restore the
  terminal after normal exit and panic on the host; run the host lane; attempt
  the exact QNX release cross-build; if that builds, exercise launch, input,
  resize, and terminal restoration on the Raspberry Pi 5. If no Ratatui backend
  supports QNX, record the result and make the full TUI host-only while retaining
  the existing QNX CLI. Do not guess target support or raise the workspace Rust
  minimum without recording compatibility with the QNX compiler.
- **Evidence:** Host PTY launch/render/input/normal restoration passed. QNX SDP
  8.0 Build 14 with QNX Rust 1.85.1 completed the AArch64 release cross-build.
  `AnsiBackend` buffer and control-character tests pass. The final binary ran
  on QNX 8.0/Raspberry Pi 5 through `ssh -tt`: Assemble and Run rendered,
  batched step/ten-step/mission keys worked, an in-session 80x24 to 70x18
  resize redrew correctly, and `q` restored the cursor, alternate screen, and
  SSH terminal before exit.

### TUI-002 — Extract reusable application sessions

- **Category / status:** architecture / complete (2026-09-20)
- **Dependencies:** TUI-001
- **Description:** Move assembler, VM, mission, and advisory orchestration out
  of CLI formatting paths into typed, bounded sessions that both the existing
  CLI and TUI can drive.
- **Acceptance:** Typed commands and snapshots cover assembly results and
  diagnostics; VM state and step results; mission state, requests, applied
  outputs, rules, faults, and transitions; advisory health, requests, findings,
  provenance, and failures. Sessions expose no TUI types, do not parse printed
  output, preserve cycle/tick limits, return typed errors, and keep provider
  work outside deterministic execution. Existing CLI output and tests remain
  reproducible.
- **Evidence:** Typed `Assembly`, `Machine`, `MissionViewSnapshot`,
  `MissionViewFrame`, `CheckRequest`, and `CheckResponse` drive both interface
  paths without parsing CLI output; domain crates contain no Ratatui types.

### TUI-003 — Implement the shell, navigation, and terminal lifecycle

- **Category / status:** UI / complete (2026-09-20)
- **Dependencies:** TUI-002
- **Description:** Add `sentinel-app tui` with an immediate-mode Ratatui shell
  and a bounded event loop.
- **Acceptance:** The shell provides Assemble, Run, Mission, and Advisory tabs;
  persistent title/status bars; contextual key hints; help and error overlays;
  keyboard-only navigation; focus indication; scrolling; resize handling; and
  a compact layout for small terminals. All exit and error paths restore raw
  mode, cursor state, and the alternate screen. Rendering reads immutable view
  state and cannot mutate VM, mission, safety, or advisory authority. UI tests
  use Ratatui's test backend or equivalent buffer assertions without requiring
  a real terminal.
- **Evidence:** `sentinel-app tui`, central key reducer, four tabs, overlays,
  compact layout, resize polling, RAII terminal session, custom backend tests,
  and host PTY smoke test.

### TUI-004 — Implement the visual assembler workspace

- **Category / status:** UI / complete (2026-09-20)
- **Dependencies:** TUI-003
- **Description:** Turn source assembly and inspection into one visual
  workspace.
- **Acceptance:** The view shows a scrollable source editor/viewer with line
  numbers, selected source path and dirty state; assembly diagnostics linked to
  line and column; symbols and entry point; encoded address, word, and decoded
  instruction; and a summary of byte count and outcome. Assemble/check/reload
  are explicit actions. Editing is in memory, saving is explicit and never
  overwrites after an external file change without a visible conflict. Invalid,
  oversized, and unreadable input produces a bounded visible error without
  crashing or leaving the terminal altered.
- **Evidence:** `AssemblerView`, wide/compact buffer tests, symbol/encoding
  display, bounded editor, explicit save/reload, and external-change test.

### TUI-005 — Implement the visual S32 runner and debugger

- **Category / status:** UI / complete (2026-09-20)
- **Dependencies:** TUI-003, TUI-004
- **Description:** Replace the line-oriented stepper experience with a
  synchronized machine view.
- **Acceptance:** The view highlights the current instruction and shows source
  or disassembly, all 32 registers, `HI`, `LO`, `PC`, machine status, steps,
  cycle use/budget, register deltas, memory writes, stack/data/MMIO regions, and
  the latest trap with stable detail. Keys support one step, a bounded step
  count, run/pause, reset, and quit; optional breakpoints are permitted only if
  they are implemented as a UI pause condition and do not change VM semantics.
  Halt, trap, and budget exhaustion are visually distinct. Every displayed
  state value comes from one coherent post-step snapshot.
- **Evidence:** `RunnerView`, one/ten-step and timed run actions, reset,
  coherent register deltas/writes/status, disassembly highlight, mapping and
  trap/status rendering, and runner session tests.

### TUI-006 — Implement the visual mission runner

- **Category / status:** UI / complete (2026-09-20)
- **Dependencies:** TUI-003, TUI-005
- **Description:** Visualize hardware-inventory and full-scenario missions
  through one runner selected by the validated document schema.
- **Acceptance:** The view combines firmware position and cycle state with
  scenario tick/phase, telemetry, feedback, firmware requests, applied outputs,
  active deterministic rules, faults, hold/abort state, supervisor events, and
  a scrollable transition timeline. Requested and applied values are visually
  separate, safety overrides and rejected requests remain prominent, abort
  remains latched according to the runtime, and the UI cannot write protected
  state or bypass the output decision path. Nominal tank and rocket examples
  remain reproducible.
- **Evidence:** typed mission snapshots for both schemas, separated request and
  applied panes, rule/fault display, bounded frame playback and timeline, and
  the existing tank/rocket integration tests.

### TUI-007 — Implement the visual advisory evaluator

- **Category / status:** UI / complete (2026-09-20)
- **Dependencies:** TUI-003, TUI-002
- **Description:** Present the bounded advisory checker as a clearly
  non-authoritative operator view.
- **Acceptance:** The view shows snapshot and rule-set identities/hashes,
  observation freshness, provider health, backend/model provenance, request
  progress, and findings grouped by `possible_violation`,
  `no_issue_observed`, and `unknown`; each finding exposes only validated rule
  IDs, cited fields, and bounded rationale. A persistent authority banner says
  the operator and deterministic policy own decisions. Provider calls run on a
  bounded worker path so slow, failed, malformed, or unavailable providers do
  not freeze the event loop or affect runner state. Credentials and raw secret
  configuration never render or enter captured UI diagnostics.
- **Evidence:** `AdvisoryView`, locally validated identities, permanent
  authority banner, bounded finding rendering, demo-root config/rule
  resolution, repeated-request configuration reuse, and one-request worker
  channel; provider work never enters the VM or mission runner.

### TUI-008 — Integrate, test, document, and demonstrate the TUI

- **Category / status:** delivery / complete (2026-09-20)
- **Dependencies:** TUI-004, TUI-005, TUI-006, TUI-007
- **Description:** Make the TUI the primary interactive demonstration while
  retaining CLI automation.
- **Acceptance:** Golden buffer tests cover representative wide, compact, empty,
  error, halt, trap, override, abort, advisory failure, and long-content views;
  reducer/session tests cover navigation and bounded actions; terminal cleanup
  is tested for normal and failure exits; the host lane passes; applicable QNX
  checks follow the result of TUI-001. README and demo documentation show the
  key map and assembler, runner, mission, and advisory flows. The demonstration
  clearly labels simulated state, deterministic decisions, and untrusted AI
  findings.
- **Evidence:** `docs/tui-user-guide.md`, the assembler/runner plus detailed
  rocket/tank Mission and Advisory walkthroughs in `docs/demo.md`, README entry
  point, Ratatui buffer/session/backend tests, host PTY smoke, and QNX release
  cross-build and live QNX SSH render/input/resize/restore smoke.

## Retired roadmap

The following unfinished tasks are retired: `BUILD-001`, `BOOT-002`,
`ISA-002`, `VM-001`, `VM-002`, `SCEN-002`, `SCEN-003`, `TWIN-001`, `SAFE-001`
through `SAFE-005`, `MISSION-001`, `AI-001` through `AI-004`, `UI-001` through
`UI-003`, `QNX-001` through `QNX-003`, `TEST-001`, and `DOC-001` through
`DOC-002`. Retiring a task does not delete its implementation or turn partial
work into completed evidence. Any future revival requires a new task with
current dependencies and acceptance criteria.

## Critical path

```text
BASE-001 -> TUI-001 -> TUI-002 -> TUI-003 -> TUI-004 -> TUI-005 -> TUI-006
                                            +---------> TUI-007
TUI-004 + TUI-005 + TUI-006 + TUI-007 -> TUI-008
```

## Recorded assumptions and open questions

- The TUI is the primary human interface; existing CLI commands remain the
  stable automation and diagnosis interface.
- The implementation uses Ratatui 0.29.0 without Crossterm and a repository
  ANSI backend. The QNX build and live SSH terminal behavior are proven for
  the recorded Raspberry Pi 5 image and artifact hash.
- The first assembler view includes editing with explicit save. Multi-file
  project editing, mouse input, syntax plugins, and theme customization are
  outside this plan unless later evidence makes them necessary.
- Advisory provider availability is optional and never gates assembly, VM, or
  mission operation.
