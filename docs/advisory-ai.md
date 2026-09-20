# Advisory AI rule-check contract

`sentinel-ai-check` implements the provider-neutral `AI-001` boundary. It
compares one bounded, identified state snapshot with one bounded, versioned
written rule set and validates a closed-schema response. This contract is for
operator-facing review only. It has no firmware source, tool invocation,
approval, policy mutation, activation, alarm-suppression, or output-control
field.

## Versioned records

The v0 identifiers are:

```text
snapshot       sentinel.ai-snapshot/v0
written rules sentinel.ai-written-rules/v0
findings       sentinel.ai-findings/v0
prompt         sentinel.ai-rule-check/v0
```

A snapshot contains its schema and positive version, scenario ID, exact
compiled scenario bundle hash, exactly one observation coordinate (`tick` or
`unix_time_ms`), a sorted map of typed fields, and an explicit list of missing
fields. Values are explicitly tagged and closed to booleans, signed integers,
unsigned integers, and strings, so JSON decoding cannot erase numeric type.
Field paths contain exactly a typed namespace and ID, such as
`telemetry.fuel_pressure`.

For a pre-operation mission review, the snapshot may include typed
`proposed.*` fields alongside current telemetry, feedback, phase, and authority
state. Written rules cite those exact proposed-action and current-state fields.
The model compares them and reports findings; it does not execute the action or
produce a proceed decision.

A written rule contains an ID, an optional deterministic scenario-rule ID for
review traceability, bounded human-readable text, and the complete list of
snapshot fields needed to assess it. The rule-set hash is independent of
compiled deterministic policy. The optional link does not import the prose
into policy and does not make a finding an invariant result. Written prose
never becomes executable policy.

A response binds the exact snapshot and rule-set hashes and records backend,
model, and prompt-contract provenance. It returns exactly one finding for each
supplied written rule. A finding contains only its rule ID, one of
`possible_violation`, `no_issue_observed`, or `unknown`, existing cited snapshot
fields, and a bounded rationale. Invented rules, invented fields, duplicate or
missing findings, unknown fields, and additional response properties are
rejected.

## Hashes and bounds

Typed content is serialized as compact UTF-8 JSON. Struct fields retain their
contract order, maps use UTF-8 key order through `BTreeMap`, and arrays retain
semantic order. Hash input is the exact ASCII domain, one zero byte, and those
JSON bytes:

```text
sentinel32:ai-snapshot:v0\0 + snapshot content
sentinel32:ai-written-rules:v0\0 + written-rule-set content
```

The default local limits are 64 KiB each for a snapshot, written rule set, and
response; 512 snapshot fields; 256 rules and findings; and 512 characters per
rationale. Provider adapters may select lower positive limits but may not skip
local validation.

When a written rule requires a field that is absent or explicitly missing, its
finding must be `unknown`; any stronger result is rejected. A stale or
cross-paired snapshot/rule hash rejects the entire response. Timeout, refusal,
transport, malformed-schema, size, hash, rule, citation, and missing-data
failures remain distinct typed errors.

## Authority separation

`sentinel-ai-check` depends on no VM, scenario runtime, safety gate, deployment
gate, or QNX code. `sentinel-safety` has no production dependency on
`sentinel-ai-check`. Integration tests validate advisory findings before and
after an output decision and demonstrate that the deterministic decision is
unchanged. The OpenAI development adapter and authenticated `llama.cpp`
deployment adapter use these exact types and validators. The latter performs
bounded blocking HTTP calls to `/health`, `/props`, and
`/v1/chat/completions`; the former is compile-time optional and uses
`/v1/responses` with strict structured output. Adapter availability or output
can only affect checker status and operator-facing findings. See
`docs/ai-user-guide.md` for operation.
