# Safety model

## Scope of claims

Sentinel-32 is a safety-oriented prototype operating only on a software digital twin. Its deterministic mechanisms can produce evidence that a specific firmware image and scenario bundle passed specified checks under recorded bounds. They cannot establish certification, general correctness, freedom from common-mode defects, or safe operation of physical launch hardware.

Bounded exploration is evidence over a finite state/depth budget, not formal proof. Virtual cycle accounting is deterministic within the S32 model; host or target timing measurements are observations, not proven worst-case execution time. The advisory AI checker may identify a possible mismatch between current state and written rules, but its output is untrusted and is neither safety evidence nor an enforcement result.

## Authority model

Firmware writes requests. It cannot directly change the simulated plant, approve itself, grant capabilities, publish scenarios, clear safety latches, or bypass the output gate. The independent output gate combines current telemetry, invariant results, compiled policy, firmware approval state, and abort state before accepting a request.

System policy may reduce a firmware-requested capability set but never expand it. Evidence is authoritative only for the exact firmware bytes, manifest, compiled scenario hash, ISA/policy/verifier versions, and validation bounds recorded in the report.

The written AI rule set is a human-readable review aid. It is versioned and hashed independently from compiled scenario policy. A deterministic invariant remains necessary even when a written rule describes the same hazard. `no_issue_observed` cannot authorize a transition, silence a deterministic alarm, or count as a passed safety check. `possible_violation` and `unknown` may be shown to an operator, but they cannot directly change output state.

## Initial launch invariants

At least these immutable policy families must be enforced outside firmware:

1. Ignition requires the configured fuel and oxidizer pressure bands.
2. Ignition requires confirmed valve positions and mutual exclusion between incompatible fill and ignition states.
3. Ignition requires primary or approved backup electrical readiness plus controller power.
4. Ignition requires continuity, flight-system readiness, range clearance, pad clearance, and absence of remote inhibit.
5. Abort is latched for the current launch attempt and ordinary firmware cannot clear it.
6. A missed controller deadline or stale heartbeat removes ordinary output authority and resolves a safe state.
7. Stale safety-relevant telemetry blocks dependent transitions and commands.
8. Firmware or scenario replacement is forbidden after the launch system becomes armed.

Exact expressions, severity, precedence, and counterexample semantics remain work for `SCEN-001` and `SAFE-001`. They must be typed scenario policy rather than identifiers hard-coded into the generic VM.

## Safe-state resolution

Resolution is deterministic and phase aware. The intended precedence is:

1. global emergency and supervisor overrides;
2. active latched-abort policy;
3. matching hazard policies by explicit priority;
4. current-phase policy;
5. actuator default safe action.

Every safety-relevant actuator needs an explicit resolved action for every reachable emergency profile. `preserve_current` is valid only when the schema explicitly permits and justifies it. Equal-priority contradictions, missing coverage, dangling references, unit errors, or non-total expressions prevent publication.

## Initial hazards and containment

| Hazard or fault | Detection owner | Required containment |
|---|---|---|
| Illegal instruction or invalid control flow | VM | Trap candidate; active image unchanged |
| Protected or unauthorized memory access | VM/capability policy | Trap and record address/reason |
| Infinite loop or budget excess | VM | Stop candidate; safe state if active |
| Firmware or scenario byte corruption | Hash/deployment gate | Refuse activation/use |
| Controller hang or stale heartbeat | Watchdog | Remove output authority and resolve safe state |
| Critical pressure or invalid launch band | Safety monitor | Block ignition; hold or latch abort by policy |
| Valve command/feedback mismatch | Transition monitor | Hold or abort after configured timeout |
| Electrical, continuity, clearance, or readiness loss | Safety monitor | Block arming/ignition; hold or abort by phase policy |
| AI checker, network, or UI loss | Process supervision | Active deterministic control continues; checker status becomes unavailable |
| Malformed, stale, or misleading AI finding | Schema/hash checks and authority separation | Reject or label the finding; deterministic control and evidence remain unchanged |
| Update requested while armed | Deployment gate | Refuse update and retain current pair |

## Last-known-good and rollback

Activation is transactional across a compatible firmware/scenario pair. The last-known-good pair remains immutable until a replacement has complete, hash-bound evidence and is accepted. Rollback cannot make an incompatible scenario/firmware pairing, clear the attempt abort latch, or bypass current policy.

## Evidence minimum

A validation report includes source and bytecode hashes, source and compiled scenario hashes, manifest, ISA/policy/verifier versions, tests and explored-state counts, bounds, first or minimal counterexample when available, maximum observed virtual cycles, timing observations labeled as such, shadow comparison, and deterministic reason codes.
