# Scenario schema working contract

**Status:** requirements baseline only. `SCEN-001` owns the exact schema and canonical serialization.

A scenario is declarative, versioned, typed, and untrusted. YAML may be an authoring format; the compiler produces a deterministic canonical representation and immutable runtime bundle. Scenario files cannot contain executable language fragments, scripts, dynamic modules, file inclusion, environment expansion, or provider instructions.

## Required concepts

- Scenario identity, display metadata, schema version, and publication version.
- Typed telemetry with units, ranges, initial value, staleness, bands, and display hints.
- Actuators with state/value domain, command semantics, reversibility/privilege, initial state, feedback, timeout, mutual exclusion, and safe-state requirement.
- Phases, permitted transitions, typed entry/completion conditions, timeout behavior, and attempt-terminal states.
- Total typed invariant/interlock expressions and deterministic inhibit/hold/abort/force/review responses.
- Global, latch, hazard, phase, and default safe-state policies with explicit priorities and full emergency coverage.
- Deterministic dynamics and fault models driven by ticks rather than wall-clock time.
- Stable generated MMIO assignments, S32 symbol output, and dashboard layout metadata kept semantically separate.

## Compilation rules

Compilation must reject unknown or duplicate identifiers, dangling feedback/references, incompatible units, invalid ranges/bands, undeclared transitions, prohibited cycles, non-total expressions, conflicting policy actions, missing safety coverage, unsafe use of `preserve_current`, invalid dynamics, nondeterministic constructs, and unsupported schema versions.

Canonicalization must define ordering, integer representation, strings/Unicode, absent versus default fields, and rejection of duplicate map keys before a hash algorithm is selected. Semantically identical accepted input must compile identically. Display layout changes must not silently change policy identity; `SCEN-001` must decide and document which metadata participates in each hash.

MMIO allocation must be deterministic and aligned within the fixed S32 region classes. Stable allocation mode must preserve existing symbol addresses when compatible channels are added. Publication yields an immutable versioned bundle and generated include file. Active firmware names the exact scenario ID and compiled hash.

## Publication and lifecycle

Drafts may be edited and simulated. Publication requires schema validation, semantic validation, safe-state coverage, successful compilation, and hashing. Published bundles are immutable. Scenario publication or activation is forbidden while a launch attempt is armed. Activation is transactional with compatible firmware and retains a last-known-good pair.

## SCEN-001 open decisions

The exact authoring grammar; typed expression AST; unit vocabulary; canonical JSON profile; hash algorithm and domain separation; ID normalization; source-versus-compiled hash semantics; schema migration policy; stable-address collision handling; safe-state profile enumeration; and limits on counts, nesting, strings, and numeric values all require explicit specification and tests.
