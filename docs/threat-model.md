# Threat model

## Protected assets

- Integrity and availability of bounded VM and mission execution and deterministic output decisions.
- Integrity and identity of firmware, scenario bundles, manifests, and recorded results.
- Confidentiality of test API credentials and deployment details.
- Accuracy and reproducibility of validation results and operator state.

## Untrusted inputs and actors

AI safety findings, human-authored assembly, written AI rules, scenario source
and metadata, terminal input, network data, fixtures, and transferred artifacts
are untrusted. A checker backend may return malformed, oversized, stale,
incomplete, refused, prompt-influenced, or incorrect findings. Firmware may be
buggy or malicious. Storage or transfer may corrupt artifacts, and provider
calls may hang or fail.

## Trust boundaries

The test-only OpenAI adapter is network-capable and outside both the real-time path and safety authority. The QNX-hosted `llama.cpp` service is authenticated, nonessential, and also outside the trusted computing base. The checker uses loopback; the server's plaintext externally bound port is restricted to the trusted lab network. Neither backend exists in the assembler, validation, safety-monitor, or output-control path. The Ratatui application can request compilation and display or advance bounded laboratory execution, but rendering has no direct memory, policy, or output authority. Untrusted source, terminal input, filenames, scenario text, provider output, and error text are bounded before entering view state; control characters must not be emitted as terminal commands. The compiler boundary accepts bounded data and produces immutable identified artifacts.

## Required controls

- Strict size, UTF-8, version, enum, identifier, and extra-field validation at all parsing boundaries.
- Scenario expressions use a closed typed grammar with no arbitrary code, file access, expansion, native loading, or dynamic evaluation.
- Written rules and string-valued state fields are delimited as inert data; model tools are unavailable.
- Capabilities are allowlisted and intersected with system policy.
- Hashes identify scenario and advisory inputs; stale advisory hashes fail closed.
- The output gate checks every request against current policy and cannot be bypassed by firmware MMIO.
- Cycle budgets and memory permissions limit firmware hangs and invalid access.
- Test credentials remain only in the OpenAI test process environment, are redacted from errors, and never enter IPC, fixtures, traces, evidence, panic text, or UI events.
- The `llama.cpp` credential remains in process environments, is redacted from diagnostics, and is not placed in the launcher command line or repository.
- The deployment model path, SHA-256, and server endpoint are validated; the server requires an API key on the restricted lab network and cannot fall back to another provider.
- VM and mission state cannot be authorized or altered by checker output, DNS, TLS, or networking failures.

## Key abuse cases

| Abuse case | Control and expected result |
|---|---|
| A written rule or state string tells the model to ignore its contract | Treat all supplied text as delimited data; strict response parsing and authority separation contain the result |
| State snapshot and rule-set hashes do not match the finding | Reject the stale or cross-paired finding before display |
| Checker reports `no_issue_observed` during a real violation | Deterministic invariant and output gate remain authoritative; evaluation records a false negative |
| Firmware requests undeclared channel access | Capability validation rejects it |
| Scenario has an incomplete safe-state matrix | Semantic compilation fails; no bundle is produced |
| UI or AI checker attempts output control | No protocol capability or process authority exists for that path |
| Credential appears in an error | Redaction and adversarial tests fail the change before completion |

## Residual risks

The trusted implementation can contain defects, hash algorithms and storage can
be misused, and deterministic test cases do not cover all reachable behavior.
The current roadmap does not add bounded exploration, lifecycle/rollback,
watchdog supervision, or QNX process isolation. The ANSI backend cross-builds
for QNX, but behavior still depends on the operator's terminal emulator and an
allocated pseudo-terminal; terminal loss can disrupt presentation without
granting output authority.
