# Repository context

Last updated: 2026-09-19 during the `AI-002`/`AI-003` provider integration.

## Current implementation

Control operations follow two accepted design principles: an operator starts
one persistent S32 invocation that runs until completion or bounded failure,
and `sentinel.hardware/v0` YAML contains hardware inventory only. Operational
thresholds, waits, sequencing, loops, and actuator requests belong to assembly.

The repository has a Rust workspace. `sentinel-core` implements
typed S32 decode/encode, the source assembler, validated manifests and sparse
memory, and complete S32 v0 reference execution with permissions, capabilities,
atomic traps, and deterministic cycle budgets. `sentinel-scenario` implements
bounded strict YAML parsing, typed validation, canonical bundle compilation,
stable MMIO allocation, generated symbols, and a generic deterministic
runtime. The app exposes VM and scenario commands, including an end-to-end
firmware harness with YAML-allocated MMIO. `BOOT-002`, `ISA-002`,
`VM-001`, and `SCEN-002` remain formally blocked by the missing target-side
evidence dependency in `BUILD-001`; `SCEN-002` also awaits review of its schema
clarification.

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
QNX run.

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
- `docs/architecture.md`: intended boundaries and lifecycle.
- `docs/development.md`: host/QNX workflow and `BUILD-001` audit/blocker.
- `docs/configuration.md`: intended configuration for core services and the advisory checker backends.
- `docs/advisory-ai.md`: exact provider-neutral snapshot, written-rule, finding, hash, limit, and authority contract.
- `docs/ai-user-guide.md`: deployment-server, QNX client, rocket sample, OpenAI test, and failure-isolation procedure.
- `docs/s32-isa.md`: accepted ISA v0 contract; implemented incrementally under `ISA-002` and `VM-001`.
- `docs/scenario-schema.md`: accepted schema v0 contract and the concrete forms implemented by `SCEN-002`.
- `crates/sentinel-core`: portable S32 ISA, assembler, VM, memory, traps, cycles, and tests.
- `crates/sentinel-scenario`: strict parser, compiler, canonical artifacts, MMIO symbols, and deterministic runtime.
- `crates/sentinel-safety`: deterministic safe-state resolution and output request decisions.
- `crates/sentinel-ai-check`: provider-neutral advisory contract, validators, `llama.cpp` client, and feature-gated OpenAI test client.
- `crates/sentinel-app`: host/QNX CLI for VM and scenario workflows.
- `examples/countdown.asm`: source-level assembler smoke example.
- `examples/lab-scenario.yaml`: hardware-only tank-pressure and two-valve MMIO inventory with no controller actions.
- `crates/sentinel-scenario/src/test-scenario.yaml`: internal full-schema compiler fixture.
- `examples/valve-controller.asm`: commented firmware that fills to a pressure target, holds ten simulated seconds, unloads, and closes both valves.
- `examples/rocket-launch-default.yaml`: full declarative normalized rocket launch-pad scenario.
- `examples/rocket-controller.asm`: persistent S32 controller for the nominal
  normalized rocket run; firmware owns thresholds, waits, interlock checks,
  actuator sequencing, ignition feedback, shutdown, and its abort path.
- `docs/safety-model.md`: claims, invariant families, containment, evidence.
- `docs/threat-model.md`: assets, untrusted boundaries, abuse cases, controls.
- `docs/validation.md`: validation layers and current results.
- `docs/decisions/`: accepted bootstrap architecture decisions and revisit triggers.
- `docs/decisions/DEC-012-single-invocation-control.md`: operator-started firmware remains active until its bounded operation completes.

## Verified environment

Host checks pass under upstream Rust 1.98.1 on `x86_64-unknown-linux-gnu`.
QNX SDP 8.0 Build 14 and linked toolchain `qnx800` produced the prior AArch64
QNX 8.0 release binary. The prior scenario-enabled artifact had SHA-256
`50a2c8546e1f256f85d7429b7f1b3e68409559fbc13cdcab6b88cb6824146aae`.
The five-crate rocket/safety/advisory and persistent assembly-runner update
passes the host lane. The QNX target libraries and updated app objects compile,
but the latest final link was blocked by a local QNX license-lock timeout, so
that older hash does not identify the current source.
The operator reports successful earlier Raspberry Pi 5 execution; exact target
image, commands, output, exit status, and execution of the current artifact
remain to be captured before `BUILD-001` is complete.
The documented QNX deployment root is `/data/home/qnxuser/sentinel-32`, with
release, example, and artifact subdirectories. `scripts/qnx-pi-upload.sh`
transfers an already-built release and fixtures to `qnxuser@qnxpi59.local`;
`scripts/qnx-build-upload.sh` runs the full host lane, host and QNX release
builds, scenario artifact generation, and that upload in one command. Target
testing remains a separate operator SSH session.

## Next work

Run the configured `llama-server`, capture its version and approved GGUF hash,
then execute the AI health/check and failure-isolation procedure on QNX.
Review the SCEN-002 runtime clarification and SAFE-001 trusted safety behavior.
Then resolve the QNX license lock, link the current workspace, run its VM and
scenario checks on the Raspberry Pi 5, and capture the evidence in
`docs/development.md`. The next unimplemented critical-path work is `SAFE-002`
or `VM-002`; live provider evaluation remains under `AI-002` through `AI-004`.

## Working tree note

The AI provider batch began from commit `d561adb`; preserve unrelated work.
