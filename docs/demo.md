# QNX visual and command-line demonstration

The Ratatui interface is the primary interactive demonstration. It runs in a
normal QNX console or an SSH pseudo-terminal and presents the assembler,
instruction runner, deterministic missions, and authority-free advisory review
without replacing their typed runtime boundaries.

## Visual TUI demonstration

Use two SSH terminals. Allocate a pseudo-terminal with `-t`; the TUI uses raw
input and restores the terminal when it exits.

### 1. Prepare the advisory service

In the first SSH terminal, start `llama-server` with the command in
[Start the local AI server](#1-start-the-local-ai-server). Wait until the model
has loaded and the server is listening on port 8080. Leave it running.

The Mission view does not need this server. Only the Advisory view uses it.

### 2. Configure and start the TUI

In the second SSH terminal, configure the rocket advisory inputs **before**
starting the TUI:

```sh
ssh -t qnxuser@qnxpi59.local
cd /data/home/qnxuser/sentinel-32/release

export TERM=xterm-256color
export S32_DEMO_ROOT=/data/home/qnxuser/sentinel-32
export S32_AI_CONFIG_PATH=/data/home/qnxuser/sentinel-32/config/ai-llama-default.env
export S32_AI_CHECK_TIMEOUT_MS=300000
export S32_AI_MAX_OUTPUT_TOKENS=512
export S32_AI_RULES_PATH=/data/home/qnxuser/sentinel-32/examples/ai/rocket-written-rules.json
export S32_TUI_SNAPSHOT_PATH=/data/home/qnxuser/sentinel-32/examples/ai/rocket-pressure-snapshot.json
export S32_TUI_AI_PROFILE=deployment

./sentinel-app tui /data/home/qnxuser/sentinel-32/examples/sample-analysis.asm
```

The exports make the recorded inputs explicit. The current TUI can also resolve
the default config, rocket snapshot, and rocket rules from `S32_DEMO_ROOT` when
`S32_AI_CONFIG_PATH` and `S32_AI_RULES_PATH` are unset. If it reports that
`S32_AI_RULES_PATH` is missing, the deployed executable predates this behavior.

**Shows:** the Assemble tab with numbered source on the left and the entry
address, byte count, encoded words, and decoded instructions on the right. The
status bar reports that the source assembled and the runner was reset. Press
`?` at any point for the complete in-application key reference.

### 3. Demo the assembler and instruction runner

1. In Assemble, press `e`, change a character, then press `Esc`. The path gains
   a `*` dirty marker. Press `a` to assemble the in-memory source. Press `l` to
   discard the edit by reloading the file. `w` is the only save action; this
   demo does not require saving.
2. Press `2` for Run. Press `s` repeatedly to watch stack allocation, `li`
   expansion, and the first memory write. `n` executes up to ten bounded
   instructions; use it to reach the `jal` and then step into the analysis
   loop. The Registers pane shows every value in hexadecimal, decoded ASCII,
   and unsigned decimal. Press `Tab` until Assembly or Registers has the cyan
   border, then use `j`/`k` or the arrow keys to scroll that pane independently.
   `Space` starts or pauses timed execution. The run ends halted after
   91 instructions and 112 cycles with sum `R02=66`, maximum `R03=25`,
   threshold count `R07=3`, average `R08=13`, remainder `R09=1`,
   `HI=1`, `LO=13`, restored `R29=0x20010000`, and `PC=0x00000070`.
   Press `r` to reset it.

### 4. Demo the rocket mission

1. Press `3`. The header initially says `selected=rocket`.
2. Press `m`. The TUI executes the bounded mission and loads 19 recorded
   frames. It does not call the AI provider.
3. Inspect the first frames with right arrow or `j`. The Firmware/phase pane
   shows assembly PCs, instruction/cycle counts, phase, supervisor transition,
   and `hold`/`abort`. Telemetry/feedback shows simulated values. Requests and
   applied outputs are deliberately separate, and the rules pane shows
   `valve_feedback_mismatch` holds during one-tick feedback delays.
4. Press `Space` to play the frames, then press it again to pause. Use left
   arrow or `k` to move backward and `r` to rewind.
5. Move to the last frame. The mission summary reports 248 firmware steps, 391
   virtual cycles, 19 ticks, and final phase `complete`. These are simulated
   deterministic counts, not physical timing or WCET.

### 5. Demo the rocket advisory

1. Press `4`. The left pane identifies the prepared rocket snapshot and
   written-rule hashes and displays its bounded fields, including fuel pressure
   `95000` and `range_clear=false`.
2. Press `a` or `Enter` once. The right pane changes to
   `provider request running` with elapsed time. The local model can take tens
   of seconds; pressing `a` again reports that a request is already running.
3. While it is pending, press `3` and inspect or replay Mission. This
   demonstrates that the provider worker does not freeze deterministic views.
   Press `4` to return to the request.
4. A successful validated response shows the `llama.cpp` backend/model and one
   rule-linked result for each written rule. It should report possible
   violations for the supplied high fuel pressure and uncleared range. Exact
   rationale wording is model-generated and may vary.
5. Point out the permanent
   `ADVISORY ONLY - authority=none - operator and deterministic policy own all decisions`
   banner. The prepared snapshot is advisory input; it is not mission
   telemetry, and its `95000` value does not enter or alter the rocket run.

### 6. Demo the tank mission and advisory

The Advisory view reads its snapshot path when the TUI starts. Press `q`, set
the tank inputs, and relaunch:

```sh
export S32_AI_RULES_PATH=/data/home/qnxuser/sentinel-32/examples/ai/tank-written-rules.json
export S32_TUI_SNAPSHOT_PATH=/data/home/qnxuser/sentinel-32/examples/ai/tank-proposed-action-snapshot.json

./sentinel-app tui /data/home/qnxuser/sentinel-32/examples/sample-analysis.asm
```

1. Press `4`, then `a`. A successful response should identify the prepared
   proposal to open both inlet and outlet valves. The proposal is displayed as
   untrusted advisory data and cannot write either actuator.
2. Press `3`, then `t`. The header changes to `selected=tank`.
3. Press `m`. Inspect or play the 21 simulated seconds. The persistent assembly
   controller fills to `50000`, holds for ten simulated seconds, unloads to
   zero, closes both valves, and halts after 203 instructions and 257 virtual
   cycles.
4. Compare the Advisory proposal with the Mission panes: the proposal does not
   appear as a firmware request or applied output. Mission behavior comes from
   the assembly program and deterministic output path.

### 7. Demo AI failure isolation

Stop `llama-server` in the first terminal with `Ctrl-C`. In the TUI, press `4`
and then `a`. The provider pane reports `UNAVAILABLE` after the configured
connection failure or timeout, while the authority banner remains unchanged.
Press `3`, select rocket or tank with `t`, and press `m`; the deterministic
mission still executes and can be inspected normally.

Press `q` to exit. The alternate screen closes, the cursor returns, and the
shell terminal settings are restored. Clear the overrides with the commands in
[Clear the demo overrides](#12-clear-the-demo-overrides).

The full key map, source-edit behavior, QNX terminal requirements, advisory
variables, and recovery command are in the
[terminal interface guide](tui-user-guide.md).

## Scriptable CLI demonstration

Run these commands after signing in to the QNX target. The deployed tree is
`/data/home/qnxuser/sentinel-32`, and every application command starts in its
`release` directory.

## 1. Start the local AI server

In the first SSH terminal:

```sh
AI_CONFIG=/data/home/qnxuser/sentinel-32/config/ai-llama-default.env
export LLAMA_API_KEY="$(sed -n 's/^S32_LLAMA_API_KEY=//p' "$AI_CONFIG")"

llama-server \
  -m /data/home/qnxuser/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf \
  --device BLAS \
  -ngl 0 \
  -c 4096 \
  --host 0.0.0.0 \
  --port 8080 \
  --alias qwen2.5-1.5b
```

**Shows:** `llama-server` loads the local Qwen model and listens on port 8080.
The API key is read from the protected default configuration instead of being
printed by the command.

## 2. Load the QNX demo configuration

In the second SSH terminal:

```sh
cd /data/home/qnxuser/sentinel-32/release

export S32_AI_CONFIG_PATH=/data/home/qnxuser/sentinel-32/config/ai-llama-default.env
export S32_AI_CHECK_TIMEOUT_MS=300000
export S32_AI_MAX_OUTPUT_TOKENS=512
```

**Shows:** no output. These variables select the absolute QNX configuration
path and give the small local model enough time to answer.

## 3. Display, assemble, and run pure S32 assembly

```sh
cat /data/home/qnxuser/sentinel-32/examples/sample-analysis.asm
```

**Shows:** a standalone S32 program that creates an array on the mapped stack,
calls an analysis function, loops through word loads, computes sum/maximum and
an inclusive threshold count, uses `DIVU` plus `MFLO`/`MFHI` for an integer
average and remainder, restores the stack, and halts. It does not use a
scenario, hardware inventory, AI service, or MMIO.

```sh
./sentinel-app check \
  /data/home/qnxuser/sentinel-32/examples/sample-analysis.asm
```

**Shows:** the assembler validates the source and reports its encoded size and
entry address:

```text
valid: 188 bytes, entry=0x00000000
```

```sh
./sentinel-app assemble \
  /data/home/qnxuser/sentinel-32/examples/sample-analysis.asm
```

**Shows:** 47 real encoded words. Pseudo-instructions are expanded before
encoding: each `li` becomes `lui` plus `ori`, `move` becomes `addu`, `b`
becomes `beq`, and `ret` becomes `jr`. Representative output is:

```text
00000000: 27BDFFE0
00000004: 3C010000
00000008: 3421000C
0000000C: AFA10000
...
00000058: 0C00001C
...
000000B8: 03E00008
```

```sh
./sentinel-app assemble \
  /data/home/qnxuser/sentinel-32/examples/sample-analysis.asm \
  /data/home/qnxuser/sentinel-32/artifacts/sample-analysis.bin
```

**Shows:** `wrote 188 bytes` and creates the raw assembled program at the given
artifact path.

```sh
./sentinel-app run \
  /data/home/qnxuser/sentinel-32/examples/sample-analysis.asm \
  256
```

**Shows:** the reference emulator executes 91 instructions over 112 virtual
cycles. The five loads and five stores cost two cycles each, and `divu` costs
twelve.
The final results match the comments in the source, the stack pointer is
restored, and the PC advances past `halt`:

```text
status=halted steps=91 cycles=112 pc=0x00000070 hi=0x00000001 lo=0x0000000D
R00=0x00000000 R01=0x00000012 R02=0x00000042 R03=0x00000019 ...
R07=0x00000003 R08=0x0000000D R09=0x00000001 ... R29=0x20010000 ...
```

```sh
./sentinel-app step \
  /data/home/qnxuser/sentinel-32/examples/sample-analysis.asm \
  256
```

**Shows:** an interactive `s32>` prompt before any instruction runs. Press
Enter or enter `s` for one instruction, `s 2` for two instructions, `r` for all
registers, `c` to continue to completion, `h` for help, or `q` to stop. Every
executed instruction reports its PC, decoded operation, cycle cost, cumulative
cycles, next PC, status, changed registers, and memory writes:

```text
s32> step=4 pc=0x0000000C instruction=Store { ... } cost=2 cycles=5 next_pc=0x00000010 status=running changes=[] writes=[0x2000FFE0=0C000000]
```

After individual or continued steps, it prints the final register dump
and:

```text
stepper-complete status=halted steps=91 cycles=112 pc=0x00000070
```

## 4. Check the AI service

```sh
./sentinel-app ai-health deployment
```

**Shows:** one `ai-health status=ready` line with the `llama.cpp` build, the
`qwen2.5-1.5b` alias, and the configured GGUF SHA-256.

## 5. Display the rocket advisory input

```sh
cat /data/home/qnxuser/sentinel-32/examples/ai/rocket-pressure-snapshot.json
cat /data/home/qnxuser/sentinel-32/examples/ai/rocket-written-rules.json
```

**Shows:** the demonstration snapshot explicitly supplies fuel pressure
`95000` and `range_clear=false`. The written advisory rules compare pressure
with `90000` and require range clearance. These are prepared demonstration
inputs; they are not measurements produced by the assembly controller.

## 6. Ask for rocket mission advice

```sh
export S32_AI_RULES_PATH=/data/home/qnxuser/sentinel-32/examples/ai/rocket-written-rules.json

./sentinel-app mission-advice deployment \
  /data/home/qnxuser/sentinel-32/examples/ai/rocket-pressure-snapshot.json
```

**Shows:** first,
`ADVISORY-REVIEW authority=none decision_owner=operator_and_deterministic_policy`.
The JSON that follows contains one finding for each written rule. A successful
model response should flag the supplied `telemetry.fuel_pressure=95000` and
`telemetry.range_clear=false` values. Rationale wording is model-generated and
can vary. Do not pipe this command through the mission-output `grep`; doing so
hides the indented JSON rationale lines.

## 7. Run the rocket mission

```sh
./sentinel-app mission-run \
  /data/home/qnxuser/sentinel-32/examples/rocket-launch-default.yaml \
  /data/home/qnxuser/sentinel-32/examples/rocket-controller.asm \
  10000
```

**Shows:** the persistent S32 assembly program loading toward its own `50000`
target, stabilizing, checking interlocks, arming, observing terminal count,
requesting ignition, receiving simulated feedback, and returning outputs to
their safe state. Each `tick=` line includes assembly PCs, instruction and
cycle counts, firmware requests, and deterministically applied outputs.

The nominal sample ends with output equivalent to:

```text
ISSUE-SUMMARY issue_events=5 notice_events=8 hold_ticks=5 abort_ticks=0 issue_rules=[valve_feedback_mismatch] notice_rules=[armed_update_forbidden]
operation=complete firmware_status=halted firmware_steps=248 firmware_cycles=391 scenario_ticks=19 final_phase=complete ...
```

The feedback-mismatch events are visible one-tick simulated feedback delays.
The deterministic rule layer holds progression until feedback agrees. The
`95000` advisory value from step 6 does not appear here because this command
does not consume the prepared AI snapshot.

## 8. Display a proposed tank action

```sh
cat /data/home/qnxuser/sentinel-32/examples/ai/tank-proposed-action-snapshot.json
cat /data/home/qnxuser/sentinel-32/examples/ai/tank-written-rules.json
```

**Shows:** a prepared proposal to open both inlet and outlet valves at a
normalized tank pressure of `50000`, plus the written rules used to review it.
The proposal is data for the advisory checker and cannot write an actuator.

## 9. Ask for tank mission advice

```sh
export S32_AI_RULES_PATH=/data/home/qnxuser/sentinel-32/examples/ai/tank-written-rules.json

./sentinel-app mission-advice deployment \
  /data/home/qnxuser/sentinel-32/examples/ai/tank-proposed-action-snapshot.json
```

**Shows:** an authority-free advisory response that should identify the
simultaneous-open proposal. It does not approve, deny, or execute that action.

## 10. Run the tank mission

```sh
./sentinel-app mission-run \
  /data/home/qnxuser/sentinel-32/examples/lab-scenario.yaml \
  /data/home/qnxuser/sentinel-32/examples/valve-controller.asm \
  10000
```

**Shows:** the assembly-owned sequence filling to `50000`, holding for ten
simulated seconds, emptying to zero, closing both valves, and halting. It ends
with output equivalent to:

```text
operation=complete firmware_status=halted firmware_steps=203 firmware_cycles=257 simulated_seconds=21 final_requests=[actuator.inlet_valve=closed,actuator.outlet_valve=closed]
```

## 11. Show that mission control does not depend on AI

Stop `llama-server` in the first terminal with `Ctrl-C`. Then run in the second
terminal:

```sh
./sentinel-app ai-health deployment
```

**Shows:** a connection or transport error because the advisory service is
unavailable.

```sh
./sentinel-app mission-run \
  /data/home/qnxuser/sentinel-32/examples/rocket-launch-default.yaml \
  /data/home/qnxuser/sentinel-32/examples/rocket-controller.asm \
  10000
```

**Shows:** the deterministic rocket simulation still completes. Provider loss
only removes advisory output; it cannot authorize, deny, interrupt, or alter
mission outputs.

## 12. Clear the demo overrides

```sh
unset S32_AI_CONFIG_PATH
unset S32_AI_CHECK_TIMEOUT_MS
unset S32_AI_MAX_OUTPUT_TOKENS
unset S32_AI_RULES_PATH
unset S32_DEMO_ROOT
unset S32_TUI_SNAPSHOT_PATH
unset S32_TUI_AI_PROFILE
unset LLAMA_API_KEY
```

**Shows:** no output. The shell no longer carries the CLI or TUI demo
overrides. Keep `TERM` set to the value supplied by the SSH client.

This demonstration uses a normalized educational digital twin. AI findings
are untrusted advisory text, the mission runs are deterministic simulations,
measured target timing is observational, and the output is not formal proof or
physical launch guidance.
