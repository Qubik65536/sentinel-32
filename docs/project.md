# Project scope

## Purpose

Sentinel-32 demonstrates how untrusted embedded firmware can be constrained, verified, observed, and supervised before and during execution. The initial deployment target is a Raspberry Pi 5 running QNX OS 8.0; portable components are developed and tested on an ordinary host.

The first scenario is a fully simulated rocket ground-launch sequencer. The reusable product is the scenario-driven execution and assurance system, not a hard-coded rocket controller.

## Goals

- Define a deterministic, fixed-width 32-bit S32 ISA, assembler, interpreter, cycle model, and replayable trace.
- Compile typed declarative scenarios into immutable runtime bundles, deterministic MMIO symbols, policies, and digital-twin dynamics.
- Keep safety invariants, safe states, authorization, deployment gates, and output gates outside firmware.
- Reject malformed programs, invalid control flow, forbidden memory access, capability excess, excessive stack use, and budget violations.
- Maintain proposed, candidate, shadow, active, and last-known-good firmware/scenario lifecycle states with hash-bound evidence.
- Provide fault injection, counterexamples, runtime supervision, and visible containment.
- Run one advisory AI safety checker that compares a bounded current-state snapshot with a versioned written rule set and reports rule-linked findings.
- Use OpenAI only for checker testing and evaluation; use a local `llama.cpp` service for the hackathon deployment.
- Offer a text/NDJSON observability path, browser operations dashboard, and declarative Scenario Studio.
- Run essential components as isolated QNX processes while keeping core logic portable.

## Non-goals

- Controlling a physical rocket, real propellant hardware, GPIO, sensors, servos, or displays.
- Claiming certification, production readiness, formal proof, or proven worst-case execution time.
- AI generation or repair of firmware, scenarios, policies, tests, deployment decisions, or control commands.
- Treating model findings, generated tests, or redundant copies of one interpreter as safety evidence by themselves.
- Allowing executable Rust, JavaScript, shell, native modules, or other arbitrary code in scenario definitions.
- Making active control depend on AI, the UI, a network, or the validation service.

## Primary users

Hackathon judges, embedded and systems developers, safety engineers, and learners studying low-level execution, validation, and containment.

## Terminology

- **S32:** Sentinel-32's project-specific MIPS-inspired ISA. It is not MIPS-compatible.
- **Firmware proposal:** Untrusted source and metadata supplied by a human or deterministic fixture.
- **Scenario source:** Declarative authoring input; untrusted until compilation succeeds.
- **Compiled scenario:** Immutable canonical runtime bundle with an identity and hash.
- **Candidate:** Assembled firmware being evaluated without output authority.
- **Shadow:** Validated candidate executing against mirrored inputs without output authority.
- **Active:** The firmware/scenario pair currently authorized by the deployment gate.
- **Last-known-good:** Previous compatible active pair retained for rollback.
- **Output request:** A firmware MMIO write that the independent output gate may accept, override, or reject.
- **Evidence:** Deterministic validation results bound to exact firmware and scenario hashes.
- **Written AI rule set:** Versioned, hash-identified natural-language rules used only by the advisory AI checker; it does not replace compiled scenario policy.
- **AI safety finding:** An untrusted advisory result that names a written rule, identifies supporting state fields, and reports `possible_violation`, `no_issue_observed`, or `unknown`.
- **Attempt:** One simulated launch lifecycle; abort remains latched until a supervisor begins another attempt.
- **Safe state:** A phase- and hazard-aware policy result, resolved from declarative rules rather than one hard-coded vector.

## Success boundary

The minimum demonstration requires a QNX-built binary executed on the Raspberry Pi 5, deterministic S32 execution and replay, the default declarative launch scenario, external safety invariants, safe and unsafe validation cases, lifecycle gating and rollback, an advisory local-`llama.cpp` rule check, OpenAI-backed checker tests, failure isolation, and usable evidence display. Each claim is limited to the exact versions, hashes, coverage, platform, and observed results recorded in its evidence.
