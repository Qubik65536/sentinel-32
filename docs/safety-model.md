# Safety model

## Scope of claims

Sentinel-32 is a safety-oriented prototype operating only on a software digital twin. Its deterministic tests and runs describe a specific firmware image and scenario bundle under recorded bounds. They cannot establish certification, general correctness, freedom from common-mode defects, or safe operation of physical launch hardware.

The project does not currently perform bounded state-space exploration or claim
formal proof. Virtual cycle accounting is deterministic within the S32 model;
host or target timing measurements are observations, not proven worst-case
execution time. The advisory AI checker may identify a possible mismatch
between current state and written rules, but its output is untrusted and is
neither safety evidence nor an enforcement result.

## Authority model

Firmware writes requests. It cannot directly change the simulated plant, grant
capabilities, clear safety latches, or bypass output decisions. The independent
safety component combines current telemetry, invariant results, compiled
policy, firmware authority state, and abort state before accepting a request.

System policy may reduce a firmware-requested capability set but never expand
it. Recorded results apply only to the exact inputs, versions, and bounds named
with them.

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

The default launch expressions, severities, and responses are compiled from
`examples/rocket-launch-default.yaml`; no rocket identifier is hard-coded in
the VM, scenario runtime, or output gate. `sentinel-safety` consumes the active
typed rule IDs reported by the runtime. Pressure-band edges, feedback
agreement, power combinations, readiness inputs, staleness, abort persistence,
authority loss, and armed-time replacement have explicit host truth-table
tests. Automated counterexample production is outside the current roadmap.

## Safe-state resolution

Resolution is deterministic and phase aware. The intended precedence is:

1. global emergency and supervisor overrides;
2. active latched-abort policy;
3. matching hazard policies by explicit priority;
4. current-phase policy;
5. actuator default safe action.

Every safety-relevant actuator needs an explicit resolved action for every reachable emergency profile. `preserve_current` is valid only when the schema explicitly permits and justifies it. The output gate counts consecutive preserved ticks and falls through to the next policy layer when the bound expires. Equal-priority contradictions, missing coverage, dangling references, unit errors, or non-total expressions prevent compilation.

An ordinary request is `accepted`, `overridden`, or `rejected`. Accepted values
retain the firmware request. Active hold, abort, force, lost-authority, or
latched-abort state resolves a typed safe action and overrides the request. An
inhibit response rejects its named request. Unknown actuators, invalid typed
values, and ordinary requests to supervisor actuators are rejected. The stable
reason codes are `OUTPUT_ACCEPTED`, `OUTPUT_RULE_INHIBIT`,
`OUTPUT_SAFE_PROFILE`, `OUTPUT_SAFE_DEFAULT`, `OUTPUT_ABORT_LATCH`,
`OUTPUT_AUTHORITY_UNAVAILABLE`, `OUTPUT_UNKNOWN_ACTUATOR`,
`OUTPUT_INVALID_REQUEST`, `OUTPUT_SUPERVISOR_ONLY`, and
`OUTPUT_MISSING_REQUEST`.

The gate API contains no AI finding or score. An unavailable controller removes
ordinary authority and resolves safe outputs. Armed-time firmware/scenario
replacement is a separate deterministic decision based on the compiled current
phase; advisory findings cannot allow it.

## Initial hazards and containment

| Hazard or fault | Detection owner | Required containment |
|---|---|---|
| Illegal instruction or invalid control flow | VM | Trap the current run without applying another instruction |
| Protected or unauthorized memory access | VM/capability policy | Trap and record address/reason |
| Infinite loop or budget excess | VM | Stop the bounded run |
| Critical pressure or invalid launch band | Safety monitor | Block ignition; hold or latch abort by policy |
| Valve command/feedback mismatch | Transition monitor | Hold or abort after configured timeout |
| Electrical, continuity, clearance, or readiness loss | Safety monitor | Block arming/ignition; hold or abort by phase policy |
| AI checker, network, or UI loss | Authority separation | Deterministic VM, mission, and output-decision APIs remain independent; checker status becomes unavailable |
| Malformed, stale, or misleading AI finding | Schema/hash checks and authority separation | Reject or label the finding; deterministic state remains unchanged |

The former last-known-good lifecycle, rollback, evidence-report, watchdog, and
bounded-exploration designs are retired from the active roadmap. Their absence
limits the safety claims of this laboratory and must remain visible in product
documentation.
