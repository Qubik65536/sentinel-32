# Architecture

## Status

This document records the version-0 boundaries. `sentinel-core` and
`sentinel-app` now exist; the remaining crates are introduced only with
functional content and their corresponding tasks.

## Trust and data flow

```mermaid
flowchart LR
    Human[Human or Scenario Studio] -->|untrusted source| SC[Scenario compiler]
    Human -->|untrusted firmware| AV[Assembler and verifier]
    SC -->|immutable bundle + hash| AV
    AV -->|candidate + bound evidence| Shadow[Shadow controller]
    Shadow --> Gate[Deployment gate]
    Gate --> Active[Active controller]
    Active -->|requests only| Output[Output gate]
    SC --> Twin[Scenario runtime / digital twin]
    Twin -->|telemetry| Active
    Twin --> Monitor[Safety monitor]
    Active --> Monitor
    Monitor -->|override / safe state| Output
    Output --> Twin
    Twin -->|bounded state snapshot| AIC[Advisory AI rule checker]
    Rules[Versioned written AI rules] --> AIC
    AIC -->|untrusted findings| Observe[Operator display and log]
```

AI findings, user input, scenario source, firmware source, and the network are outside the trusted computing base. The AI checker has no edge to the assembler, verifier, safety monitor, deployment gate, or output gate. The initial trusted computing base is the S32 decoder/interpreter, memory and capability enforcement, scenario compiler and canonicalization, invariant evaluator, safe-state resolver, output and deployment gates, watchdog, hashing/evidence binding, and the QNX adapter used by those components.

## Crate map

| Crate | Responsibility | Key restriction |
|---|---|---|
| `sentinel-core` | ISA types, assembler, VM, traps, cycles, traces | No AI, UI, network, QNX, or unsafe code |
| `sentinel-scenario` | Schema, validation, canonicalization, MMIO allocation, runtime bundle and deterministic dynamics | No hard-coded rocket semantics in generic machinery |
| `sentinel-safety` | Invariants, safe-state resolution, static checks, exploration, evidence, lifecycle decisions | Narrative and AI scores cannot authorize anything |
| `sentinel-protocol` | Versioned bounded service messages | No pointers or platform handles |
| `sentinel-qnx` | Timing, scheduling, IPC, health and narrowly scoped FFI | Own nearly all target-specific unsafe code |
| `sentinel-ai-check` | Snapshot/rule/finding types, fixtures, test-only OpenAI adapter, local `llama.cpp` adapter, evaluation metadata | Advisory only; no firmware generation, activation, policy, or output-control path |
| `sentinel-app` | Process entry points, orchestration, NDJSON, dashboard and Studio | UI failure cannot affect essential control |

`sentinel-core` and `sentinel-app` are implemented workspace members and pass
the QNX cross-build. The core currently contains ISA types, canonical
decode/encode, cycle metadata, and the assembler. The other rows remain planned
boundaries.

## Runtime separation

The host-native milestone may use modules or threads. The target architecture separates the safety monitor/output gate, active controller, shadow controller, scenario runtime, compiler/bundle store, verifier/deployment gate, and bounded snapshot publisher into QNX processes. For the hackathon deployment, the advisory checker client and `llama.cpp` server run together on the local companion host; the server accepts only loopback connections. The target sends the companion only versioned, size-bounded state snapshots, and loss of that transport has no control effect. Initial QNX priority intent, pending target measurement, is: safety and output gating; scenario clock and active controller; trace and snapshot transport; shadow controller; verifier.

The AI check is asynchronous with respect to control. The checker receives a size-bounded, schema-validated snapshot plus exact written-rule version and hash. Its structured result links each finding to rule IDs and snapshot fields, and may be displayed or logged only. Timeouts, malformed results, model loss, or stopping the entire checker cannot interrupt the active controller or suppress deterministic alarms.

OpenAI is a development test backend used to exercise the same checker contract and evaluate fixtures. It is not a deployment fallback. The hackathon runtime backend is a pinned local GGUF model served by `llama.cpp`; model file hash, `llama.cpp` version, launch arguments, prompt contract, and rule-set hash are provenance rather than safety evidence.

## S32 machine boundary

S32 has 32 general registers (`R0` is hardwired to zero), `HI`, `LO`, `PC`, fixed 32-bit little-endian instructions, no delay slots, deterministic virtual cycles, and a sparse 32-bit address space split into program, data, stack, telemetry, commands, feedback, supervisor, and protected safety regions. The accepted exact encoding is in `docs/s32-isa.md`; decode, encode, assembly, and cycle metadata are implemented in `sentinel-core`.

## Scenario boundary

The scenario compiler owns typed channels, phases, transitions, invariants, safe-state policies, dynamics, faults, stable MMIO allocation, generated symbols, canonical serialization, and hashing. Runtime code consumes a compiled immutable bundle. Publication and firmware activation are forbidden while the simulation is armed.

## Lifecycle boundary

A proposal becomes a candidate after parsing and assembly. A candidate may enter shadow only with deterministic validation evidence bound to its bytes and the compiled scenario hash. Activation is a separate deterministic decision. Active and last-known-good state are retained transactionally; rollback requires hash-compatible artifacts and cannot clear a launch-attempt abort latch.
