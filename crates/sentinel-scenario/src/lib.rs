#![forbid(unsafe_code)]

mod compiler;
mod model;
mod runtime;

pub use compiler::{
    AllocationEntry, AllocationKind, AllocationMode, Compilation, CompileError, CompiledBundle,
    Diagnostic, HardwareRegister, HardwareSource, PriorAllocation, compile, compile_hardware,
};
pub use model::*;
pub use runtime::{Runtime, RuntimeError, RuntimeState, TickInput, TickResult};

pub const SCHEMA_ID: &str = "sentinel.scenario/v0";
pub const COMPILER_CONTRACT: &str = "sentinel-scenario-compiler/v0";

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;

    const FIXTURE: &str = include_str!("test-scenario.yaml");
    const HARDWARE_FIXTURE: &str = include_str!("../../../examples/lab-scenario.yaml");

    fn compiled() -> Compilation {
        compile(FIXTURE, AllocationMode::Clean).unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn compiles_hardware_only_manifest_to_mmio_symbols() {
        let compilation =
            compile_hardware(HARDWARE_FIXTURE).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(compilation.bundle.scenario_id, "tank_hardware");
        assert_eq!(
            compilation
                .bundle
                .mmio
                .iter()
                .map(|entry| (entry.qualified_id.as_str(), entry.address))
                .collect::<Vec<_>>(),
            vec![
                ("actuator.inlet_valve", 0x5000_0000),
                ("actuator.outlet_valve", 0x5000_0004),
                ("feedback.inlet_valve_position", 0x6000_0000),
                ("feedback.outlet_valve_position", 0x6000_0004),
                ("telemetry.tank_pressure", 0x4000_0000),
            ]
        );
        assert!(
            compilation
                .symbols
                .contains(".equ S32_ACTUATOR_INLET_VALVE, 0x50000000")
        );
    }

    #[test]
    fn compiles_fixture_with_stable_hashes_mmio_and_symbols() {
        let first = compiled();
        let second = compiled();
        assert_eq!(first.source_hash, second.source_hash);
        assert_eq!(first.semantic_hash, second.semantic_hash);
        assert_eq!(first.bundle_hash, second.bundle_hash);
        assert_eq!(
            first.bundle_hash,
            "829b9b71ccdd14eb8f4c86b02f10ef87be5be841d685b178dfc9282d20bb357d"
        );
        assert_eq!(first.canonical_bundle, second.canonical_bundle);
        assert_eq!(
            first
                .bundle
                .mmio
                .iter()
                .map(|entry| (entry.qualified_id.as_str(), entry.address))
                .collect::<Vec<_>>(),
            vec![
                ("actuator.valve", 0x5000_0000),
                ("feedback.valve_position", 0x6000_0000),
                ("telemetry.counter", 0x4000_0000),
            ]
        );
        assert_eq!(
            first.symbols,
            "# schema: sentinel.scenario/v0\n# scenario: lab_counter\n# bundle-sha256: 829b9b71ccdd14eb8f4c86b02f10ef87be5be841d685b178dfc9282d20bb357d\n.equ S32_ACTUATOR_VALVE, 0x50000000\n.equ S32_FEEDBACK_VALVE_POSITION, 0x60000000\n.equ S32_TELEMETRY_COUNTER, 0x40000000\n"
        );
    }

    #[test]
    fn presentation_only_changes_preserve_semantic_and_bundle_identity() {
        let first = compiled();
        let changed = FIXTURE
            .replace("Deterministic lab counter", "Deterministic counter display")
            .replace("      x: 0", "      x: 1");
        let second =
            compile(&changed, AllocationMode::Clean).unwrap_or_else(|error| panic!("{error}"));
        assert_ne!(first.source_hash, second.source_hash);
        assert_ne!(first.presentation_hash, second.presentation_hash);
        assert_eq!(first.semantic_hash, second.semantic_hash);
        assert_eq!(first.bundle_hash, second.bundle_hash);
    }

    #[test]
    fn semantic_change_changes_bundle_identity() {
        let first = compiled();
        let changed = FIXTURE.replace("add_clamped: 1", "add_clamped: 2");
        let second =
            compile(&changed, AllocationMode::Clean).unwrap_or_else(|error| panic!("{error}"));
        assert_ne!(first.semantic_hash, second.semantic_hash);
        assert_ne!(first.bundle_hash, second.bundle_hash);
    }

    #[test]
    fn stable_allocation_reuses_compatible_addresses_and_checks_hash() {
        let first = compiled();
        let prior = PriorAllocation {
            scenario_id: first.bundle.scenario_id.clone(),
            publication: first.bundle.publication,
            bundle_hash: first.bundle_hash.clone(),
            entries: first.bundle.mmio.clone(),
        };
        let next_source = FIXTURE.replace("publication: 1", "publication: 2");
        let stable = compile(
            &next_source,
            AllocationMode::Stable {
                prior: &prior,
                expected_bundle_hash: &first.bundle_hash,
            },
        )
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(stable.bundle.mmio, first.bundle.mmio);
        let error = compile(
            &next_source,
            AllocationMode::Stable {
                prior: &prior,
                expected_bundle_hash: &"0".repeat(64),
            },
        )
        .expect_err("mismatched prior hash must fail");
        assert_eq!(error.diagnostics[0].code, "HASH_MISMATCH");
    }

    #[test]
    fn rejects_duplicate_unknown_malformed_and_injection_shaped_input() {
        let duplicate = FIXTURE.replacen("id: lab_counter", "id: lab_counter\nid: duplicate", 1);
        let duplicate_error =
            compile(&duplicate, AllocationMode::Clean).expect_err("duplicate key must fail");
        assert_eq!(duplicate_error.diagnostics[0].code, "DUPLICATE_KEY");

        let unknown = FIXTURE.replacen("tick_ms: 100", "tick_ms: 100\nscript: rm -rf /", 1);
        let unknown_error = compile(&unknown, AllocationMode::Clean)
            .expect_err("unknown executable-looking field must fail");
        assert_eq!(unknown_error.diagnostics[0].code, "UNKNOWN_FIELD");

        let tagged = FIXTURE.replacen("name: Deterministic", "name: !shell Deterministic", 1);
        assert!(compile(&tagged, AllocationMode::Clean).is_err());

        let inert = FIXTURE.replace(
            "A generic valve and counter fixture for compiler and runtime checks.",
            "$(touch /tmp/never) is inert display text.",
        );
        assert!(compile(&inert, AllocationMode::Clean).is_ok());
    }

    #[test]
    fn rejects_schema_type_graph_writer_safe_state_and_rule_coverage_errors() {
        let cases = [
            (
                FIXTURE.replace("sentinel.scenario/v0", "sentinel.scenario/v9"),
                "SCHEMA_VERSION",
            ),
            (
                FIXTURE.replacen("type: valve_state", "type: bool", 1),
                "TYPE_MISMATCH",
            ),
            (
                FIXTURE.replace(
                    "      transitions: []",
                    "      transitions:\n        - id: restart_cycle\n          to: idle\n          trigger: automatic\n          new_attempt: false\n          guard: {const: {type: bool, value: true}}",
                ),
                "PHASE_CYCLE",
            ),
            (
                FIXTURE.replace(
                    "  confirm_valve:\n",
                    "  duplicate_counter:\n    target: telemetry.counter\n    set: 0\n  confirm_valve:\n",
                ),
                "DYNAMIC_WRITER",
            ),
            (
                FIXTURE.replace(
                    "  defaults:\n    actuator.valve: {set: closed}",
                    "  defaults: {}",
                ),
                "SAFE_COVERAGE",
            ),
            (
                FIXTURE.replacen(
                    "fresh: {ref: telemetry.counter",
                    "fresh: {ref: feedback.valve_position",
                    1,
                ),
                "RULE_COVERAGE",
            ),
        ];
        for (source, expected) in cases {
            let error = compile(&source, AllocationMode::Clean)
                .expect_err("invalid scenario must fail without a bundle");
            assert!(
                error
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == expected),
                "missing {expected}: {:?}",
                error.diagnostics
            );
        }
    }

    #[test]
    fn deterministic_runtime_advances_dynamics_phase_and_faults() {
        let compilation = compiled();
        let mut left = Runtime::new(compilation.bundle.clone());
        let mut right = Runtime::new(compilation.bundle);
        let input = TickInput {
            actuator_requests: BTreeMap::from([(
                "actuator.valve".to_owned(),
                ScalarValue::Enum("open".to_owned()),
            )]),
            ..TickInput::default()
        };
        for _ in 0..3 {
            assert_eq!(
                left.tick(&input).unwrap_or_else(|error| panic!("{error}")),
                right.tick(&input).unwrap_or_else(|error| panic!("{error}"))
            );
            assert_eq!(left.state(), right.state());
        }
        assert_eq!(left.state().phase, "complete");
        assert_eq!(
            left.state().values.get("telemetry.counter"),
            Some(&ScalarValue::Signed(3))
        );
        assert_eq!(
            left.state().values.get("feedback.valve_position"),
            Some(&ScalarValue::Enum("open".to_owned()))
        );

        let fault = TickInput {
            active_faults: BTreeSet::from(["counter_override".to_owned()]),
            ..TickInput::default()
        };
        let result = left.tick(&fault).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            left.state().values.get("telemetry.counter"),
            Some(&ScalarValue::Signed(99))
        );
        assert_eq!(result.active_rules, vec!["counter_high"]);
    }

    #[test]
    fn runtime_executes_every_dynamic_operation_family() {
        let base = compiled().bundle;

        let mut set_bundle = base.clone();
        set_bundle.dynamics = BTreeMap::from([(
            "set_counter".to_owned(),
            DynamicRule {
                when: None,
                target: "telemetry.counter".to_owned(),
                operation: DynamicOperation::Set(ScalarValue::Signed(7)),
            },
        )]);
        let mut set_runtime = Runtime::new(set_bundle);
        set_runtime
            .tick(&TickInput::default())
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            set_runtime.state().values.get("telemetry.counter"),
            Some(&ScalarValue::Signed(7))
        );

        let mut approach_bundle = base.clone();
        approach_bundle.dynamics = BTreeMap::from([(
            "approach_counter".to_owned(),
            DynamicRule {
                when: None,
                target: "telemetry.counter".to_owned(),
                operation: DynamicOperation::Approach(Approach {
                    reference: None,
                    value: Some(ScalarValue::Signed(6)),
                    step: 2,
                }),
            },
        )]);
        let mut approach_runtime = Runtime::new(approach_bundle);
        approach_runtime
            .tick(&TickInput::default())
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            approach_runtime.state().values.get("telemetry.counter"),
            Some(&ScalarValue::Signed(2))
        );

        let open = TickInput {
            actuator_requests: BTreeMap::from([(
                "actuator.valve".to_owned(),
                ScalarValue::Enum("open".to_owned()),
            )]),
            ..TickInput::default()
        };
        let mut copy_bundle = base.clone();
        copy_bundle.dynamics = BTreeMap::from([(
            "copy_valve".to_owned(),
            DynamicRule {
                when: None,
                target: "feedback.valve_position".to_owned(),
                operation: DynamicOperation::Copy("actuator.valve".to_owned()),
            },
        )]);
        let mut copy_runtime = Runtime::new(copy_bundle);
        copy_runtime
            .tick(&open)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            copy_runtime.state().values.get("feedback.valve_position"),
            Some(&ScalarValue::Enum("open".to_owned()))
        );

        let mut map_bundle = base;
        map_bundle.dynamics = BTreeMap::from([(
            "map_valve".to_owned(),
            DynamicRule {
                when: None,
                target: "feedback.valve_position".to_owned(),
                operation: DynamicOperation::MapEnum(MapEnum {
                    source: "actuator.valve".to_owned(),
                    values: BTreeMap::from([
                        ("closed".to_owned(), ScalarValue::Enum("closed".to_owned())),
                        ("open".to_owned(), ScalarValue::Enum("open".to_owned())),
                    ]),
                }),
            },
        )]);
        let mut map_runtime = Runtime::new(map_bundle);
        map_runtime
            .tick(&open)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            map_runtime.state().values.get("feedback.valve_position"),
            Some(&ScalarValue::Enum("open".to_owned()))
        );
    }

    #[test]
    fn runtime_applies_fault_priority_freeze_bias_delay_and_one_shot() {
        let base = compiled().bundle;
        for (effect, expected) in [
            (
                FaultEffect::BiasClamped {
                    target: "telemetry.counter".to_owned(),
                    amount: 2,
                },
                ScalarValue::Signed(3),
            ),
            (
                FaultEffect::Freeze {
                    target: "telemetry.counter".to_owned(),
                },
                ScalarValue::Signed(0),
            ),
            (
                FaultEffect::DropUpdates {
                    target: "telemetry.counter".to_owned(),
                },
                ScalarValue::Signed(0),
            ),
        ] {
            let mut bundle = base.clone();
            bundle.faults = BTreeMap::from([(
                "controlled".to_owned(),
                Fault {
                    activation: FaultActivation::Mode(FaultMode::Fixture),
                    one_shot: false,
                    priority: 1,
                    effects: vec![effect],
                },
            )]);
            let mut runtime = Runtime::new(bundle);
            runtime
                .tick(&TickInput {
                    active_faults: BTreeSet::from(["controlled".to_owned()]),
                    ..TickInput::default()
                })
                .unwrap_or_else(|error| panic!("{error}"));
            assert_eq!(
                runtime.state().values.get("telemetry.counter"),
                Some(&expected)
            );
        }

        let mut delayed_bundle = base.clone();
        delayed_bundle.faults = BTreeMap::from([(
            "delay".to_owned(),
            Fault {
                activation: FaultActivation::Mode(FaultMode::Fixture),
                one_shot: false,
                priority: 1,
                effects: vec![FaultEffect::DelayFeedback {
                    target: "feedback.valve_position".to_owned(),
                    ticks: 1,
                }],
            },
        )]);
        let mut delayed = Runtime::new(delayed_bundle);
        let input = TickInput {
            actuator_requests: BTreeMap::from([(
                "actuator.valve".to_owned(),
                ScalarValue::Enum("open".to_owned()),
            )]),
            active_faults: BTreeSet::from(["delay".to_owned()]),
            ..TickInput::default()
        };
        delayed
            .tick(&input)
            .unwrap_or_else(|error| panic!("{error}"));
        delayed
            .tick(&input)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            delayed.state().values.get("feedback.valve_position"),
            Some(&ScalarValue::Enum("closed".to_owned()))
        );
        delayed
            .tick(&input)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            delayed.state().values.get("feedback.valve_position"),
            Some(&ScalarValue::Enum("open".to_owned()))
        );

        let mut one_shot_bundle = base;
        one_shot_bundle.faults = BTreeMap::from([(
            "once".to_owned(),
            Fault {
                activation: FaultActivation::Mode(FaultMode::Fixture),
                one_shot: true,
                priority: 1,
                effects: vec![FaultEffect::Override {
                    target: "telemetry.counter".to_owned(),
                    value: ScalarValue::Signed(50),
                }],
            },
        )]);
        let mut once = Runtime::new(one_shot_bundle);
        let input = TickInput {
            active_faults: BTreeSet::from(["once".to_owned()]),
            ..TickInput::default()
        };
        once.tick(&input).unwrap_or_else(|error| panic!("{error}"));
        once.tick(&input).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            once.state().values.get("telemetry.counter"),
            Some(&ScalarValue::Signed(51))
        );
    }
}
