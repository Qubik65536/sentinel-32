# Ratatui interface design

## Purpose

The Sentinel-32 TUI is the primary interactive view of the implemented
laboratory. It makes assembly, machine execution, mission behavior,
deterministic output decisions, and advisory findings visible in one terminal.
It does not add validation, safety, deployment, or output authority.

The existing CLI remains available for scripts, regression checks, redirected
output, and target diagnosis.

## Dependency and terminal backend

The application pins Ratatui 0.29.0 with default features disabled. Its
dependency graph is pure Rust for the features used here, declares Rust 1.74,
remains compatible with the workspace's Rust 1.85 minimum, and cross-builds with QNX SDP 8.0 Build
14 and the QNX-modified Rust 1.85.1 compiler. The crate is MIT licensed.

Sentinel-32 implements Ratatui's `Backend` trait directly using bounded ANSI
output, standard I/O, and the `stty` utility. It does not depend on Crossterm,
Termion, libc terminal bindings, QNX FFI, or `unsafe` code. The backend enters
the alternate screen, hides the cursor, records the exact prior terminal mode,
and restores all three on normal and error unwinding through RAII. It polls
`stty size` for resize changes and uses an 80-by-24 fallback if size reporting
is unavailable. QNX operation requires a console or SSH pseudo-terminal.

Ratatui uses immediate rendering: every frame is derived from application view
state. Terminal event collection comes from the selected backend. Sentinel-32
uses one central event collector and translates input into bounded, typed
commands for the focused view.

References:

- <https://ratatui.rs/>
- <https://ratatui.rs/concepts/event-handling/>
- <https://docs.rs/ratatui/latest/ratatui/>

## Application structure

```text
terminal events                         advisory worker
      |                                      |
      v                                      v
+-------------+     typed commands     +-------------+
| TUI reducer | ---------------------> | app sessions|
| + view state| <--------------------- | and snapshots|
+-------------+   bounded results      +-------------+
      |
      v
Ratatui frame
```

The reducer owns navigation, focus, selection, scroll offsets, editor state,
dialogs, and transient messages. Application sessions own assembly, VM,
mission, and advisory orchestration. Rendering is a pure projection of a
coherent snapshot plus view state. It cannot call providers, step the VM, tick
a mission, save files, or alter output decisions.

The event loop may coalesce redraw requests but does not drop typed execution
results. Provider work uses a bounded request/result channel. Only one advisory
request may be active in the initial version; cancellation discards its eventual
result by request ID and grants no authority.

## Global layout

```text
+ Sentinel-32 | Assemble | Run | Mission | Advisory ---- profile/status ----+
|                                                                          |
|                           active view                                    |
|                                                                          |
+ status/error message ----------------------------------------------------+
| Tab view  Shift+Tab focus  ? help  Ctrl+P files  q quit                  |
+--------------------------------------------------------------------------+
```

The title bar always identifies the active view and current file or mission.
The status line shows the latest bounded result, never a credential or raw
provider configuration. The key bar changes with focus. A modal help overlay
lists global and view-specific commands. Error overlays preserve the prior
usable screen and can be dismissed without changing domain state.

At wide widths, views use adjacent panes. At medium widths, secondary panes
become selectable tabs within the view. Below the documented minimum size, the
TUI shows a small-terminal message and retains quit/help/resize handling rather
than attempting a broken layout.

## Assemble view

```text
+ source: examples/sample-analysis.asm * -------+ diagnostics --------------------+
|  1  .entry start                        | error/warning, line:column       |
|  2  start:                              |                                 |
|  3 >    addiu sp, sp, -32               + symbols ------------------------+
|  4      li r1, 12                       | start  0x00000000                |
|  5      sw r1, 0(sp)                    | analyze_samples  0x00000070      |
+-----------------------------------------+---------------------------------+
| address     word       decoded instruction                                |
| 00000000    27BDFFE0   addiu sp, sp, -32                                  |
+ bytes=... entry=... outcome=valid ----------------------------------------+
```

The source pane supports viewing and bounded in-memory editing. Check/assemble,
reload, and save are explicit commands. The dirty marker is always visible.
Save compares the current file identity/metadata with the load snapshot and
opens a conflict dialog if it changed externally. Diagnostics select their
source location. Encoded rows select the corresponding source location when
the assembler exposes that mapping; otherwise the UI labels the absence rather
than guessing.

## Run view

```text
+ assembly/disassembly -------------------+ registers ----------------------+
| 00000000  addi r1, r0, 3                | r00 00000000  r01 00000003 *    |
|>00000004  addi r1, r1, -1               | ...                             |
| 00000008  bne r1, r0, loop              | r31 00000000  hi 00000000       |
|                                         | lo 00000000  pc 00000004        |
+ memory / mappings ----------------------+ latest step --------------------+
| program rx 00000000..                   | cost 1  cycles 2/9              |
| stack   rw 20000000..                   | r1: 3 -> 2                      |
| writes: none                            | status: running                 |
+ s step  n step-count  Space run/pause  r reset  b breakpoint ------------+
```

Changed values receive both a style and a textual marker so color is not the
only signal. The instruction cursor, register/memory delta, cycle charge, and
status all come from the same completed step. Run mode repeatedly issues
bounded steps through the same session API and remains interruptible. A
breakpoint, if implemented, pauses before a matching PC and does not modify VM
memory or instruction semantics.

## Mission view

```text
+ firmware -------------------------------+ mission ------------------------+
|>pc 00000034  sw r3, 0(r4)               | tick 8  phase loading -> hold   |
| steps 117  cycles 181/5000               | hold yes  abort no              |
+ telemetry / feedback -------------------+ requests / applied -------------+
| fuel_pressure       50000                | main_valve  open -> CLOSED      |
| oxidizer_pressure   49999 !              | reason OUTPUT_RULE_INHIBIT      |
+ rules and faults -----------------------+ timeline -----------------------+
| ISSUE pressure_pair_ready                | 006 loading                     |
| fault none                               | 007 loading                     |
|                                         | 008 hold                        |
+-------------------------------------------------------------------------+
```

Firmware requests and applied outputs are separate columns. Overrides and
rejections show their stable reason codes. Hold, abort, stale telemetry, active
faults, and supervisor events remain visible even when focus moves. The mission
timeline is bounded in memory and scrollable. The view supports load, start,
step frame/tick where the session permits, run/pause, and reset/new attempt
through validated APIs; it cannot directly edit telemetry or protected state.

## Advisory view

```text
+ snapshot -------------------------------+ written rules -----------------+
| id/hash ...  tick 8  fresh              | ruleset/hash ...                |
| scenario/hash ...                       | R-01 pressure agreement         |
+ provider -------------------------------+ findings -----------------------+
| llama.cpp  model/hash ...               | POSSIBLE VIOLATION  R-01       |
| health ready  request complete  31.6 s  | fields: fuel, oxidizer          |
|                                         | rationale: ...                  |
+ ADVISORY ONLY — operator and deterministic policy own all decisions -------+
```

The authority banner is always visible. The view shows validated snapshot,
rule-set, provider, model, prompt-contract, and timing provenance when present.
Findings are grouped by outcome and cannot reference unvalidated rule IDs or
fields. Timeout, refusal, incomplete, malformed, unavailable, and cancelled
states are distinct. Raw credentials, authorization headers, and unrestricted
provider bodies are never rendered.

## Initial key model

| Key | Global action |
|---|---|
| `1`..`4` | Select Assemble, Run, Mission, or Advisory |
| `Tab` / `Shift+Tab` | Move focus within the active view |
| Arrow keys / `j` `k` | Move selection or scroll focused pane |
| `Enter` | Activate the focused bounded action |
| `Esc` | Close overlay or cancel the current UI mode |
| `?` | Toggle contextual help |
| `Ctrl+P` | Open the bounded file selector |
| `q` | Quit when no editor/dialog owns text input |

View-specific shortcuts appear in the bottom key bar. Destructive-looking
actions such as reset, discard, overwrite after conflict, and quit with dirty
source require a dialog. Assembly, stepping, mission start, and advisory check
are direct actions because they operate inside the software laboratory.

## Visual and content rules

- Use labels, icons available in plain ASCII, and style together; never encode
  status with color alone.
- Reserve red for traps, aborts, rejected output, and invalid input; yellow for
  holds, overrides, stale data, and advisory unknown; green for completed valid
  operations. Themes must retain readable monochrome output.
- Escape control characters and bound every string, list, timeline, and error
  before storing it in view state.
- Keep numeric values aligned. Display addresses and machine words as fixed
  eight-digit hexadecimal; display cycles as used/budget.
- Preserve selection by stable identity where possible when lists refresh.
- Never label bounded runs or tests as proof, and never style AI findings as
  deterministic alarms or approvals.

## Implementation record

`TUI-001` through `TUI-008` are implemented in `sentinel-app`. Buffer tests
exercise all four main views and the compact layout; session tests exercise
assembly, stepping, mission snapshots, external save conflicts, key parsing,
and ANSI rendering. The complete validation record, including the host lane
and QNX build command, is maintained in `docs/validation.md` and
`docs/development.md`. Operator instructions are in `docs/tui-user-guide.md`.
