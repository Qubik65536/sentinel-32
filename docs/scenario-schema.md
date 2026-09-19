# Sentinel scenario schema v0

The smaller hardware-only lab inventory is specified separately in
`docs/hardware-manifest.md`. It must not be confused with this publishable
scenario contract: it contains no phases, safety rules, dynamics, faults, or
output-gating policy.

**Schema identifier:** `sentinel.scenario/v0`
**Status:** Accepted under `SCEN-001` after human review on 2026-09-19.

A scenario is declarative, versioned, bounded, and untrusted. The compiler turns
accepted source into an immutable semantic bundle, a separate presentation
artifact, and generated S32 symbols. Runtime code consumes only validated
compiled artifacts.

## Source data model

The authoring format is the UTF-8 YAML 1.2 JSON-compatible subset. Documents may
contain maps with string keys, sequences, strings, booleans, null, and decimal
integers. Parsers must reject duplicate keys before typed decoding. Floats,
timestamps, binary scalars, custom tags, anchors, aliases, merge keys, multiple
documents, directives, file inclusion, environment expansion, and non-UTF-8
input are forbidden. Unknown fields are errors in every typed object.

Identifiers match `[a-z][a-z0-9_]{0,63}` and are compared byte-for-byte.
Display names are NFC-normalized UTF-8 strings and do not act as identifiers.
Every reference uses a typed namespace such as `telemetry.fuel_pressure` or
`actuator.fill_valve`; implicit cross-namespace lookup is forbidden.

The root object has exactly these fields:

| Field | Required | Type and meaning |
|---|---|---|
| `schema` | yes | literal `sentinel.scenario/v0` |
| `id` | yes | scenario identifier |
| `publication` | yes | positive, monotonically increasing `u32` |
| `tick_ms` | yes | `u32` in `1..=10_000` |
| `metadata` | yes | display name, bounded description, optional tags |
| `types` | no | reusable enum definitions |
| `telemetry` | yes | telemetry-channel map |
| `actuators` | yes | actuator map |
| `feedback` | yes | feedback-channel map |
| `phases` | yes | phase map and initial phase |
| `rules` | yes | typed invariant/interlock map |
| `safe_states` | yes | defaults and profile layers |
| `dynamics` | yes | deterministic tick-update rules |
| `faults` | no | named deterministic fault definitions |
| `layout` | no | presentation-only dashboard data |

All domain collections are maps keyed by ID; entries do not repeat their ID.
`types` maps IDs to `{variants: [id, ...]}`. `phases` is
`{initial: id, items: {id: phase, ...}}`. `safe_states` is
`{defaults: {actuator-ref: action}, profiles: {id: profile}}`. `dynamics` and
`faults` are ID-keyed maps. Empty optional collections normalize to empty maps
rather than null.

V0 uses these concrete YAML forms for fields that have closed alternatives:

- actuator commands are `set` or `{pulse: {duration_ticks: N}}`;
- phase timeouts are `hold`, `abort`, or `{transition: transition_id}`;
- rule scopes are `global` or a nonempty sequence of phase IDs; and
- responses, safe-state actions, dynamic operations, and fault effects are
  externally tagged one-key maps using the alternative names defined below.

`metadata` contains `name` (1..128 scalar values), `description` (0..2048), and
at most 32 tags of 1..32 characters. Tags and all ID-keyed maps are serialized
in ascending UTF-8 byte order.

## Values, types, and units

Runtime scalar types are `bool`, `i32`, `u32`, and a named enum with at most 64
variants. Enum variants use identifier syntax and compile to their declared
zero-based ordinal. Physical values use integer storage and one closed unit:

```text
none                 count                 tick
millisecond          pressure_milli        percent_milli
millivolt            milliampere            temperature_milli_c
```

`pressure_milli`, `percent_milli`, and `temperature_milli_c` are scaled by
1,000 relative to their displayed base unit. No implicit conversion exists.
Both sides of comparison or arithmetic must have the same scalar type and unit.
Numeric ranges use inclusive `min` and `max`. Boolean and enum domains are
implicit and omit `range`. Initial values and every compiled constant must lie
within the declared domain.

Telemetry and feedback entries contain `type`, `unit`, numeric `range` when
applicable, `initial`, and `stale_after_ticks`. Telemetry may also declare
ordered, non-overlapping named bands. Feedback additionally names an optional
actuator and a confirmation condition. Safety-relevant channels must have
positive staleness limits.

An actuator entry contains:

- `type`, `unit`, numeric `range` when applicable, and `initial`;
- `safe_default` within its domain;
- `privilege`: `ordinary` or `supervisor`;
- `reversibility`: `reversible`, `attempt_latched`, or `irreversible`;
- optional typed `feedback` reference and positive `feedback_timeout_ticks`;
- a list of mutually exclusive actuator/value predicates;
- `allow_preserve_current`, default false; and
- command behavior `set` or `pulse`, with a bounded pulse duration for `pulse`.

Every ordinary actuator receives a request MMIO slot. Supervisor actuators are
absent from ordinary firmware capabilities.

## Closed expression grammar

Expressions are single-key map nodes. They cannot call functions, loop, allocate
names, access files, inspect time, or evaluate strings as code.

```yaml
{const: {type: bool, value: true}}
{ref: telemetry.fuel_pressure}
{not: <bool-expr>}
{all: [<bool-expr>, ...]}
{any: [<bool-expr>, ...]}
{compare: {op: ge, left: <scalar-expr>, right: <scalar-expr>}}
{between: {value: <scalar-expr>, min: <scalar-expr>, max: <scalar-expr>}}
{fresh: {ref: telemetry.fuel_pressure, max_age_ticks: 2}}
{phase_is: armed}
{fault_active: pressure_step}
```

Comparison operators are `eq`, `ne`, `lt`, `le`, `gt`, and `ge`; ordering is
unavailable for booleans and enums. `all` and `any` contain 1..64 expressions
and evaluate left-to-right without changing their total result. `between` is
inclusive. `fresh` accepts telemetry or feedback only and is false when missing
or older than its bound. References are read-only snapshots from the start of a
tick. Every expression is statically typed, total, and has a maximum depth of
24 and maximum 256 nodes.

There is no general numeric expression language in v0. Dynamics use the closed
operations below. This keeps overflow and dependency behavior explicit.

## Phases and transitions

`phases.initial` names the initial phase. Each phase defines `entry`,
`completion`, `timeout_ticks`, `on_timeout`, and an ordered transition list.
Entry and completion are boolean expressions. A transition contains a unique
ID, destination phase, guard, and trigger: `automatic`, `supervisor`, `hold`, or
`abort`. At most one automatic transition may be true on a tick; simultaneous
true automatic transitions are a runtime controlled trap and a compile-time
error when statically evident.

The automatic transition graph must be acyclic. A supervisor transition may
start a new attempt from a terminal phase to the initial phase only when marked
`new_attempt: true`; it clears attempt-scoped state through the supervisor path.
Phases mark `armed`, `terminal`, and `abort_terminal` explicitly. An abort
terminal phase has no automatic outgoing transition. Publication must prove
that every reachable nonterminal phase has a timeout or a completion path.

`on_timeout` is one of `hold`, `abort`, or a named supervisor-only transition.
Timeout zero means no timeout and is permitted only for terminal phases.

## Rules and deterministic responses

Each rule has an ID, `condition`, `scope`, `severity`, `response`, and bounded
operator message. The condition states when the unsafe or inhibited situation
exists. Scope is `global` or a nonempty phase list. Severity is `advisory`,
`inhibit`, `hold`, or `abort`.

Responses are closed data:

- `observe`: emit a deterministic result only;
- `inhibit`: reject listed actuator requests or phase transitions;
- `hold`: inhibit progression and resolve the named safe-state profile;
- `abort`: latch abort for the attempt and resolve the named profile; or
- `force`: resolve named actuator values through a safe-state profile; or
- `review`: inhibit the named transition until a supervisor records an
  acknowledgement for the same rule and current attempt.

An `advisory` rule may only `observe`. `review` requires `hold` severity and its
acknowledgement cannot come from firmware, UI state, or AI. Safety-relevant
rules must use a deterministic response. `abort` is attempt-latched and
ordinary firmware cannot clear it. Rule IDs and typed results, rather than
narrative messages, drive policy. AI findings never appear in this grammar.

Compilation requires explicit rules for pressure-band gating, valve feedback
and mutual exclusion, electrical readiness, continuity/readiness/clearance and
remote inhibit, abort latching, controller freshness, telemetry freshness, and
armed-time update prohibition when the scenario declares the referenced
concepts.

## Safe-state resolution

Every actuator has a default action. Profiles provide partial actuator maps and
declare one layer: `global`, `abort_latch`, `hazard`, or `phase`. Hazard profiles
have a unique integer priority from 0 through 255; larger values win. Resolution
order is global, abort latch, matching hazards from high to low priority,
current phase, then actuator default. The first action for an actuator wins.
There is at most one global profile, one abort-latch profile, and one profile
for each phase.

An action is `{set: <typed-value>}`, `{deenergize: true}`, or
`{preserve_current: {max_ticks: N, justification: <string>}}`. `deenergize`
requires a declared deenergized value and compiles to `set`. Preservation is
valid only when the actuator allows it, the bound is `1..=1000`, a nonempty
justification is present, and a lower layer supplies the action used after the
bound. Equal-priority hazards that can match together may not give different
actions for one actuator. Compilation enumerates reachable phase/profile
combinations and rejects missing coverage or conflicts.

## Dynamics and faults

Dynamics execute once per virtual tick in ascending rule ID order. Each rule
has an optional boolean `when`, one target telemetry or feedback channel, and
exactly one operation:

- `set`: assign a typed constant;
- `copy`: assign a same-type, same-unit snapshot reference;
- `add_clamped`: add a signed constant using checked widened arithmetic and
  clamp to the target range;
- `approach`: move toward a reference or constant by at most a positive step;
- `map_enum`: total mapping from every source enum variant to a target value; or
- `confirm_after`: reflect an actuator request after a bounded tick delay.

Each target may be written by at most one matching normal dynamic rule. Reads
come from the start-of-tick snapshot, so ordering cannot create dataflow. The
compiler rejects ambiguous writers and dependency cycles involving delayed
state. Integer overflow before a specified clamp is a compilation or controlled
runtime error, never wraparound.

A fault has an ID, activation mode (`fixture`, `supervisor`, or a bounded tick
window), optional one-shot behavior, and ordered effects. Effects are
`override`, `bias_clamped`, `freeze`, `drop_updates`, and `delay_feedback` on
declared channels. Fault priority is a unique `u8`; higher priority wins.
Conflicting equal-priority effects are rejected. Fault strings and metadata are
inert. Random distributions, wall-clock triggers, and host callbacks are not
supported.

Tick order is: capture inputs; update ages; apply active fault controls; evaluate
dynamics from the snapshot; commit channel updates; evaluate phase transitions
and rules; latch abort/hold; resolve safe-state actions; gate actuator requests;
emit the trace event; increment the tick. Identical bundle, initial state, input
events, and fault schedule must produce identical state and trace events.

## Canonicalization and hashes

Accepted source is converted to a typed tree with all defaults materialized.
Canonical JSON is UTF-8 with no whitespace, keys sorted by UTF-8 bytes, strings
NFC-normalized and JSON-escaped, booleans and null in lowercase, and integers in
minimal decimal form with no negative zero. Arrays retain semantic order; sets
and ID maps are sorted during typed normalization. Duplicate keys are rejected
before this step.

SHA-256 inputs use the exact ASCII domain, one zero byte, then canonical bytes:

```text
source hash       sentinel32:scenario-source:v0\0 + full normalized source
semantic hash     sentinel32:scenario-semantic:v0\0 + source without metadata/layout
bundle hash       sentinel32:scenario-bundle:v0\0 + canonical compiled semantic bundle
presentation hash sentinel32:scenario-presentation:v0\0 + canonical metadata/layout artifact
```

The compilation result retains the full source hash and presentation hash as
provenance. The compiled semantic bundle contains the schema ID, scenario
ID/publication, semantic hash, tick duration, types, channels, phases, rules,
safe-state matrix, dynamics, faults, MMIO map, and compiler-contract version.
It excludes the presentation-sensitive source hash, layout, display metadata,
and authoring comments. Firmware binds scenario ID and the hash of the exact
canonical compiled bundle bytes. Changing only layout or display metadata
changes source/presentation hashes but not semantic/bundle hashes. Published
semantic bundles are immutable.

V0 has no automatic migration. A compiler may read only its exact supported
schema identifier. Migration is an explicit offline command that produces new
source, a new publication number, and a report; it never mutates published data.

## MMIO allocation and generated symbols

All slots are four bytes. Scalar values occupy one slot; enum ordinals are
`u32`; booleans are `0` or `1`. Clean allocation sorts fully qualified channel
IDs by UTF-8 bytes and assigns consecutive slots in the appropriate ISA region:

```text
telemetry  0x40000000    actuator requests 0x50000000
feedback   0x60000000    supervisor state  0x70000000
```

Allocation mode is a compiler option, outside scenario source. Stable mode
accepts a prior published allocation table containing its bundle hash and
qualified-ID/address pairs. Unchanged compatible IDs retain addresses; removed
addresses stay reserved; new IDs take the lowest aligned unused address. A
type, unit, width, or region change is incompatible and requires a new ID or an
explicit major schema migration. Duplicate, unaligned, out-of-region,
unknown-origin, or exhausted allocations fail compilation. Scenario source
cannot request a numeric address.

Generated S32 symbols use uppercase `S32_<NAMESPACE>_<ID>` after replacing each
non-alphanumeric identifier separator with `_`. Collisions after conversion are
errors. The include lists schema ID, scenario ID, bundle hash, and sorted
`.equ NAME, 0xADDRESS` records, with reproducible LF line endings.

## Presentation layout

`layout` is a presentation-only object containing panels and nodes. A panel has
an ID, display title, and ordered node IDs. A node has an ID, a reference to one
declared channel/actuator/rule, integer grid coordinates and size in `0..=4095`,
and an optional display style from the closed set `value`, `gauge`, `state`,
`command`, or `alarm`. At most 64 panels and 1,024 nodes are accepted. Dangling
references, duplicate placement IDs, negative sizes, and unknown styles fail
validation. Layout order, coordinates, labels, and styles have no effect on
compilation, policy, MMIO, or runtime evaluation.

## Limits and diagnostics

Before allocation, parsing is bounded to 1 MiB source, 64-byte IDs, 2,048-byte
descriptions, nesting depth 32, 4,096 map entries, and 16,384 sequence elements.
A scenario may contain at most 1,024 telemetry channels, 1,024 feedback
channels, 512 actuators, 256 phases, 1,024 transitions, 2,048 rules, 512 safe
profiles, 2,048 dynamics, and 512 faults. Compiled artifacts are limited to
8 MiB. All count and size arithmetic is checked.

Diagnostics contain stable code, severity, source path, line/column when
available, and bounded message. Required error families are:

```text
SCHEMA_VERSION  UNKNOWN_FIELD    DUPLICATE_KEY    INVALID_ID
LIMIT_EXCEEDED  TYPE_MISMATCH    UNIT_MISMATCH    VALUE_RANGE
UNKNOWN_REF     REF_KIND         EXPR_DEPTH       EXPR_NOT_TOTAL
PHASE_CYCLE     PHASE_DEAD_END   TRANSITION_AMBIGUOUS
RULE_RESPONSE   RULE_COVERAGE    SAFE_CONFLICT    SAFE_COVERAGE
PRESERVE_UNSAFE DYNAMIC_WRITER   DYNAMIC_CYCLE    DYNAMIC_OVERFLOW
FAULT_CONFLICT  MMIO_ALIGNMENT   MMIO_COLLISION   MMIO_EXHAUSTED
STABLE_INCOMPAT HASH_MISMATCH     PUBLICATION_ORDER
```

No invalid source produces a partial publishable bundle. Diagnostics and preview
allocation have no activation authority.

## Minimal component examples

These fragments illustrate reusable components; omitted root sections remain
required in a full document.

```yaml
telemetry:
  fuel_pressure:
    type: i32
    unit: pressure_milli
    range: {min: 0, max: 120000}
    initial: 0
    stale_after_ticks: 2
    bands:
      ready: {min: 78000, max: 82000}

types:
  fill_valve_state:
    variants: [closed, open]

actuators:
  fill_valve:
    type: fill_valve_state
    unit: none
    initial: closed
    safe_default: closed
    deenergized: closed
    privilege: ordinary
    reversibility: reversible
    feedback: feedback.fill_valve_position
    feedback_timeout_ticks: 3
    mutually_exclusive: []
    allow_preserve_current: false
    command: set

feedback:
  fill_valve_position:
    type: fill_valve_state
    unit: none
    initial: closed
    stale_after_ticks: 2
    actuator: actuator.fill_valve
    confirms: {compare: {op: eq, left: {ref: feedback.fill_valve_position}, right: {ref: actuator.fill_valve}}}
```

```yaml
actuators:
  ignition_bus:
    type: bool
    unit: none
    initial: false
    safe_default: false
    deenergized: false
    privilege: ordinary
    reversibility: attempt_latched
    feedback: feedback.ignition_bus_live
    feedback_timeout_ticks: 1
    mutually_exclusive: []
    allow_preserve_current: false
    command: set

feedback:
  ignition_bus_live:
    type: bool
    unit: none
    initial: false
    stale_after_ticks: 1
    actuator: actuator.ignition_bus
    confirms: {compare: {op: eq, left: {ref: feedback.ignition_bus_live}, right: {ref: actuator.ignition_bus}}}
```

Any incompatible grammar, type, canonicalization, hashing, allocation, policy,
or tick-semantics change requires a new schema identifier and accepted decision
record. Adding meaning to previously rejected input is also incompatible.
