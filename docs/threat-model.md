# Threat model

## Protected assets

- Integrity and availability of active control and the safety/output gates.
- Integrity, identity, and compatibility of firmware, scenario bundles, manifests, and evidence.
- Confidentiality of test API credentials and deployment details.
- Accuracy and reproducibility of traces, validation results, and operator state.
- Availability of the last-known-good rollback pair.

## Untrusted inputs and actors

AI safety findings, human-authored assembly, written AI rules, scenario source and metadata, dashboard requests, network data, fixtures, transferred artifacts, and external process messages are untrusted. A checker backend may return malformed, oversized, stale, incomplete, refused, prompt-influenced, or incorrect findings. A controller may be buggy or malicious. Storage or transfer may corrupt artifacts. Processes may hang, crash, or send stale/replayed messages.

## Trust boundaries

The test-only OpenAI adapter is network-capable and outside both the real-time path and safety authority. The deployment `llama.cpp` service is authenticated, nonessential, network separated, and also outside the trusted computing base. Its plaintext HTTP port is restricted to the trusted lab network. Neither backend exists in the assembler, validation, deployment, safety-monitor, or output-control path. The dashboard and Scenario Studio can request compilation and display results but cannot activate firmware or directly write plant outputs. The compiler/verifier boundary accepts bounded data and produces immutable identified artifacts. QNX IPC messages are versioned, length checked, state checked, and do not carry pointers or platform handles.

## Required controls

- Strict size, UTF-8, version, enum, identifier, and extra-field validation at all parsing boundaries.
- Scenario expressions use a closed typed grammar with no arbitrary code, file access, expansion, native loading, or dynamic evaluation.
- Written rules and string-valued state fields are delimited as inert data; model tools are unavailable.
- Capabilities are allowlisted and intersected with system policy.
- Hashes bind evidence, active artifacts, and scenario/firmware compatibility; stale hashes fail closed.
- Deployment requires a separate deterministic gate and is locked while armed.
- The output gate checks every request against current policy and cannot be bypassed by firmware MMIO.
- Watchdogs, cycle budgets, memory permissions, and process isolation limit hangs and invalid access.
- Test credentials remain only in the OpenAI test process environment, are redacted from errors, and never enter IPC, fixtures, traces, evidence, panic text, or UI events.
- The `llama.cpp` credential remains in process environments, is redacted from diagnostics, and is not placed in the launcher command line or repository.
- The deployment model path, SHA-256, and server endpoint are validated; the server requires an API key on the restricted lab network and cannot fall back to another provider.
- Active control continues when the checker, validation, UI, DNS, TLS, or networking fails.

## Key abuse cases

| Abuse case | Control and expected result |
|---|---|
| A written rule or state string tells the model to ignore its contract | Treat all supplied text as delimited data; strict response parsing and authority separation contain the result |
| State snapshot and rule-set hashes do not match the finding | Reject the stale or cross-paired finding before display |
| Checker reports `no_issue_observed` during a real violation | Deterministic invariant and output gate remain authoritative; evaluation records a false negative |
| Proposal requests undeclared channel access | Capability intersection/static validation rejects it |
| Candidate claims it passed validation | Narrative has no authority; deployment gate accepts only locally generated hash-bound evidence |
| Old valid evidence is paired with new bytes | Firmware/scenario hash mismatch rejects activation |
| Studio publishes an incomplete safe-state matrix | Semantic compilation fails; no bundle is produced |
| Replay or out-of-order lifecycle message | Protocol version/state machine rejects transition |
| UI or AI checker attempts output control | No protocol capability or process authority exists for that path |
| Credential appears in an error | Redaction and adversarial tests fail the change before completion |

## Residual risks

The trusted implementation can contain defects, identical redundant interpreters share common-mode faults, hash algorithms and storage can be misused, bounded exploration misses unvisited states, and QNX process separation does not itself establish correctness. Target timing, priority inversion, IPC behavior, crash recovery, and device-image configuration remain unverified until QNX milestones run on the Raspberry Pi 5.
