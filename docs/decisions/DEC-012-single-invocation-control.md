# DEC-012: Operations use one persistent firmware invocation

**Status:** Accepted on 2026-09-19 by operator direction.

## Decision

An operator triggers an S32 control program once for one operation. That same
machine instance retains registers and control flow, samples changing hardware,
issues requests, loops through the operation, and halts only when the task is
complete or bounded execution stops it. The host, UI, network, and simulator
must not repeatedly restart firmware to advance its control sequence.

The external hardware model may update read-only telemetry and feedback while
the machine runs. It models physical response only; it does not select firmware
states or issue actuator requests. Operation thresholds, waits, sequencing, and
completion decisions belong to assembly. A mandatory virtual-cycle budget still
bounds every invocation and fails closed when exhausted.

## Consequences

- Registers and memory can retain controller state for the whole operation.
- `HALT` means the operation has finished, rather than the end of one polling
  tick.
- Tests must prove that one entry at `start` reaches final safe requests without
  a firmware restart.
- Hardware YAML remains an inventory of registers, types, ranges, and reset
  values; it contains no controller sequence.
