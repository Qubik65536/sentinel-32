# Sentinel-32 terminal interface guide

The `sentinel-app tui` command opens the visual Sentinel-32 laboratory. It
combines the assembler, S32 machine runner, mission playback, and advisory
review in one keyboard-operated terminal. It runs on Linux and on QNX 8.0 in
an SSH pseudo-terminal. The deterministic runtime remains independent of the
interface and the advisory provider.

## Start it

From a host checkout:

```sh
cargo run -p sentinel-app -- tui examples/sample-analysis.asm
```

From the deployed QNX tree, first connect with terminal allocation enabled:

```sh
ssh -t qnxuser@qnxpi59.local
cd /data/home/qnxuser/sentinel-32/release
export TERM=xterm-256color
export S32_DEMO_ROOT=/data/home/qnxuser/sentinel-32
./sentinel-app tui /data/home/qnxuser/sentinel-32/examples/sample-analysis.asm
```

The QNX implementation uses Ratatui with Sentinel-32's safe ANSI backend. It
depends only on standard terminal escape sequences and the QNX `stty` utility;
it contains no QNX FFI or `unsafe` code. SSH must allocate a pseudo-terminal,
so use `ssh -t` when launching the application directly in one command. A
terminal of at least 60 columns by 16 rows is recommended. Resize is detected
while the application is running.

If a shell is interrupted while it owns the terminal, restore it with:

```sh
stty sane
printf '\033[0m\033[?25h\033[?1049l'
```

## Global keys

| Key | Action |
|---|---|
| `1`, `2`, `3`, `4` | Open Assemble, Run, Mission, or Advisory |
| `Tab`, `Shift+Tab` | Move pane focus |
| `j`, `k` or arrow keys | Move or scroll in the active view |
| `Ctrl+P` | Enter an assembly source path; `Enter` loads it |
| `?` | Open contextual help |
| `Esc` | Close help, a path prompt, or source editing |
| `q` | Exit when an editor or dialog does not own input |

The status line explains the latest action. The key line at the bottom shows
the shortcuts for the current view. Terminals smaller than the supported
layout show a compact warning while retaining help and quit handling.

## Assemble

The Assemble view opens the source path supplied on the command line. It shows
numbered source, diagnostics, the entry address and byte count, and each
encoded word beside its decoded instruction.

| Key | Action |
|---|---|
| `a` | Assemble the current in-memory source and reset Run on success |
| `e` | Enter the bounded in-memory editor |
| `w` | Save explicitly |
| `l` | Reload the source from disk and assemble it |

While editing, printable ASCII inserts at the cursor, arrow keys move,
`Backspace` deletes, `Enter` creates a line, and `Esc` returns to view mode. A
`*` after the path marks unsaved changes. Save refuses to overwrite a file that
changed on disk since it was loaded and reports a visible conflict. The editor
does not silently save on exit.

## Run

The Run view uses the last successfully assembled program. It keeps the
instruction list, program counter, all 32 registers, `HI`, `LO`, status, steps,
cycles, memory mappings, latest register changes, and writes in one coherent
post-step display.

| Key | Action |
|---|---|
| `s` or `Enter` | Execute one instruction |
| `n` | Execute up to ten instructions, stopping on halt or trap |
| `c` or `Space` | Run or pause bounded execution |
| `r` | Reset from the current successful assembly |
| `j`, `k` or arrows | Scroll disassembly |

Running stops on `halt`, a typed trap, or the configured cycle budget. Changed
registers carry a `*` marker as well as color, so the display remains useful in
a monochrome terminal.

## Mission

The Mission view runs the existing typed mission harness and plays back its
bounded frames. It keeps requested outputs separate from deterministically
applied outputs and shows firmware PCs, instruction and cycle counts, phase,
telemetry, active rules, faults, hold/abort state, and summary events.

| Key | Action |
|---|---|
| `t` | Select the tank or rocket example |
| `m` or `Enter` | Execute the selected bounded mission |
| `Space` | Play or pause recorded frames |
| arrows or `j`, `k` | Move one frame |
| `r` | Return to the first frame |

Mission state is simulated. The TUI cannot write protected state, accept an
output directly, or bypass the deterministic decision path.

## Advisory

The Advisory view loads the configured snapshot and written rule set, starts
one provider request on a worker thread, and displays validated identities,
provenance, status, cited fields, and rule-linked findings. Its permanent
banner says `ADVISORY ONLY - authority=none`; findings cannot start, stop, or
alter a mission.

Configure the deployment adapter as described in
[the AI guide](ai-user-guide.md). For an explicit recorded demonstration, set
the snapshot and rule set before launching the TUI:

```sh
export S32_AI_CONFIG_PATH=/data/home/qnxuser/sentinel-32/config/ai-llama-default.env
export S32_AI_RULES_PATH=/data/home/qnxuser/sentinel-32/examples/ai/rocket-written-rules.json
export S32_TUI_SNAPSHOT_PATH=/data/home/qnxuser/sentinel-32/examples/ai/rocket-pressure-snapshot.json
export S32_TUI_AI_PROFILE=deployment
```

These variables are explicit demo overrides. If they are absent, the TUI
resolves the deployment config, rocket snapshot, and matching rocket rules
under `S32_DEMO_ROOT`, the repository tree, or the standard QNX deployment
root. Set both snapshot and rule paths when demonstrating another mission.

Press `a` or `Enter` in Advisory to request a check. Slow or failed provider
work leaves Assemble, Run, and Mission responsive. The UI never displays the
API key or raw authorization configuration. `S32_TUI_AI_PROFILE` defaults to
`deployment`; the only alternative adapter is the feature-gated development
test profile.

## Asset lookup

The optional `S32_DEMO_ROOT` points the TUI at a tree containing `examples/`.
Without it, the application searches the current repository/package layout and
then `/data/home/qnxuser/sentinel-32`. Supplying the assembly path explicitly is
recommended for recorded demonstrations.

The CLI commands remain available for scripts, redirected evidence, and
diagnosis. The TUI consumes typed assembly, VM, mission, and advisory results;
it does not scrape or reinterpret human-readable CLI output.
