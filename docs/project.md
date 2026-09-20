# Project scope

## Purpose

Sentinel-32 demonstrates how untrusted embedded firmware can be constrained, verified, observed, and supervised before and during execution. The initial deployment target is a Raspberry Pi 5 running QNX OS 8.0; portable components are developed and tested on an ordinary host.

The first scenario is a fully simulated rocket ground-launch sequencer. The reusable product is the scenario-driven execution and assurance system, not a hard-coded rocket controller.

## Goals

- Define a deterministic, fixed-width 32-bit S32 ISA, assembler, interpreter, and cycle model.
- Compile typed declarative scenarios into immutable runtime bundles, deterministic MMIO symbols, policies, and digital-twin dynamics.
- Keep safety invariants, safe states, authorization, and output decisions outside firmware.
- Reject malformed programs, forbidden memory access, capability excess, and budget violations in the implemented laboratory boundary.
- Make assembly, machine execution, mission state, deterministic output decisions, and advisory findings understandable in one terminal interface.
- Run one advisory AI safety checker that compares a bounded current-state snapshot with a versioned written rule set and reports rule-linked findings.
- Use OpenAI only for checker testing and evaluation; use an authenticated `llama.cpp` deployment server for the hackathon deployment.
- Offer a Ratatui terminal interface while retaining command-line workflows for automation and diagnosis.
- Keep core logic portable and run the same TUI through host or QNX terminal
  primitives without adding UI dependencies to domain crates.

## Control design principles

- **Operator-triggered program:** An operator starts one S32 program for one
  operation. That single machine invocation retains state and controls the
  operation until it completes, traps, or exhausts its mandatory cycle budget.
  No host, simulator, UI, or network service repeatedly restarts the program to
  advance its sequence.
- **Hardware-only YAML:** A `sentinel.hardware/v0` document declares only which
  hardware registers exist, their types, ranges, units, and reset values. It
  contains no thresholds, waits, phases, transitions, control sequence, or
  actuator actions. Those operational decisions belong to S32 assembly.
- **Physical response remains external:** The hardware model may update
  read-only telemetry and feedback while firmware runs. It models physical
  response and never chooses controller state or writes actuator requests.
- **Mission:** An operator-visible pairing of a declared hardware inventory or
  full scenario with S32 firmware. `mission-run` is the common execution
  interface; tank and rocket are examples rather than runner types.
- **Mission advisory:** A bounded comparison of an identified mission snapshot
  with its configured versioned written rules, requested with
  `mission-advice` before the operator considers execution. It informs the
  operator and never grants or removes deterministic output authority.

## Non-goals

- Controlling a physical rocket, real propellant hardware, GPIO, sensors, servos, or displays.
- Claiming certification, production readiness, formal proof, or proven worst-case execution time.
- AI generation or repair of firmware, scenarios, policies, tests, deployment decisions, or control commands.
- Treating model findings, generated tests, or redundant copies of one interpreter as safety evidence by themselves.
- Allowing executable Rust, JavaScript, shell, native modules, or other arbitrary code in scenario definitions.
- Making deterministic mission execution depend on AI, the UI, a network, or a validation service.

## Primary users

Hackathon judges, embedded and systems developers, safety engineers, and learners studying low-level execution, validation, and containment.

## Terminology

- **S32:** Sentinel-32's project-specific MIPS-inspired ISA. It is not MIPS-compatible.
- **Firmware proposal:** Untrusted source and metadata supplied by a human or deterministic fixture.
- **Scenario source:** Declarative authoring input; untrusted until compilation succeeds.
- **Compiled scenario:** Immutable canonical runtime bundle with an identity and hash.
- **Output request:** A firmware MMIO write that the independent output gate may accept, override, or reject.
- **Evidence:** Recorded deterministic validation results bound to identified firmware and scenario artifacts where the implemented workflow produces them.
- **Written AI rule set:** Versioned, hash-identified natural-language rules used only by the advisory AI checker; it does not replace compiled scenario policy.
- **AI safety finding:** An untrusted advisory result that names a written rule, identifies supporting state fields, and reports `possible_violation`, `no_issue_observed`, or `unknown`.
- **Attempt:** One simulated launch lifecycle; abort remains latched until a supervisor begins another attempt.
- **Safe state:** A phase- and hazard-aware policy result, resolved from declarative rules rather than one hard-coded vector.

## Success boundary

The minimum demonstration is a Ratatui application that visually assembles and
inspects S32 source, steps and runs the deterministic VM, runs the tank and
rocket missions with visible telemetry and output decisions, and displays
bounded advisory findings without granting them authority. Existing CLI
workflows remain available. Ratatui 0.29.0 with the safe ANSI backend passes the
licensed QNX release cross-build and is intended for an allocated SSH terminal.
Each claim remains limited to the exact versions, hashes, coverage, platform,
and observed results recorded for it.
