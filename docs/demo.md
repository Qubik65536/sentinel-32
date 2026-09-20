# QNX command-line demonstration

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

## 3. Check the AI service

```sh
./sentinel-app ai-health deployment
```

**Shows:** one `ai-health status=ready` line with the `llama.cpp` build, the
`qwen2.5-1.5b` alias, and the configured GGUF SHA-256.

## 4. Display the rocket advisory input

```sh
cat /data/home/qnxuser/sentinel-32/examples/ai/rocket-pressure-snapshot.json
cat /data/home/qnxuser/sentinel-32/examples/ai/rocket-written-rules.json
```

**Shows:** the demonstration snapshot explicitly supplies fuel pressure
`95000` and `range_clear=false`. The written advisory rules compare pressure
with `90000` and require range clearance. These are prepared demonstration
inputs; they are not measurements produced by the assembly controller.

## 5. Ask for rocket mission advice

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

## 6. Run the rocket mission

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
`95000` advisory value from step 5 does not appear here because this command
does not consume the prepared AI snapshot.

## 7. Display a proposed tank action

```sh
cat /data/home/qnxuser/sentinel-32/examples/ai/tank-proposed-action-snapshot.json
cat /data/home/qnxuser/sentinel-32/examples/ai/tank-written-rules.json
```

**Shows:** a prepared proposal to open both inlet and outlet valves at a
normalized tank pressure of `50000`, plus the written rules used to review it.
The proposal is data for the advisory checker and cannot write an actuator.

## 8. Ask for tank mission advice

```sh
export S32_AI_RULES_PATH=/data/home/qnxuser/sentinel-32/examples/ai/tank-written-rules.json

./sentinel-app mission-advice deployment \
  /data/home/qnxuser/sentinel-32/examples/ai/tank-proposed-action-snapshot.json
```

**Shows:** an authority-free advisory response that should identify the
simultaneous-open proposal. It does not approve, deny, or execute that action.

## 9. Run the tank mission

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

## 10. Show that mission control does not depend on AI

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

## 11. Clear the demo overrides

```sh
unset S32_AI_CONFIG_PATH
unset S32_AI_CHECK_TIMEOUT_MS
unset S32_AI_MAX_OUTPUT_TOKENS
unset S32_AI_RULES_PATH
unset LLAMA_API_KEY
```

**Shows:** no output. The shell no longer carries the demo overrides.

This demonstration uses a normalized educational digital twin. AI findings
are untrusted advisory text, the mission runs are deterministic simulations,
measured target timing is observational, and the output is not formal proof or
physical launch guidance.
