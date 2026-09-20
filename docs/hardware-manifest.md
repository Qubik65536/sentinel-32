# Hardware manifest v0

`sentinel.hardware/v0` is the small lab-runner inventory format. It declares
which MMIO registers exist. It does not declare phases, transitions, rules,
dynamics, faults, timing, or actions. S32 firmware controls the order of
actuator requests when it runs.

This inventory-only boundary is a design principle rather than a convention.
A hardware manifest must not contain an operation trigger, pressure target,
hold duration, loop, sequence, or completion condition. An operator triggers
the assembled S32 program once, and that program remains active until the whole
operation completes or bounded execution stops it.

The root keys are `schema`, `id`, `publication`, `types`, `telemetry`,
`actuators`, and `feedback`. Unknown keys are rejected. `types` maps enum names
to ordered `variants`; the zero-based variant position is its MMIO integer.
Each register has `type`, `unit`, optional numeric `range`, and `initial`.
`initial` is the register's reset value, not a scheduled action.

The compiler allocates telemetry from `0x40000000`, actuator-request registers
from `0x50000000`, and feedback from `0x60000000`. Firmware receives read-only
telemetry and feedback capabilities and write-only actuator-request
capabilities. A request remains subject to the independent deterministic safety
and output-gating boundary; the lab runner does not grant direct physical output
authority.

`examples/lab-scenario.yaml` is the canonical hardware-only example. The full
`sentinel.scenario/v0` document used by compiler tests is an internal crate
fixture and is not shipped as a runnable example.

The app's `tank-run` demonstration supplies a deterministic lab plant outside
the YAML contract. The operator starts one persistent firmware invocation.
Every completed inlet/outlet command pair advances the plant by one simulated
second while that invocation remains active. An open inlet raises pressure by
10000 milli-units, an open outlet lowers it by 10000, and closed valves retain
pressure. These are simulated physical responses, not controller actions. The
pressure target, ten-second counter, sequencing, and every inlet/outlet request
remain in `examples/valve-controller.s32`.
