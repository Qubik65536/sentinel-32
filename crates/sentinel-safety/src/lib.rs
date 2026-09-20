#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sentinel_scenario::{
    Action, CompiledBundle, ProfileLayer, RuleResponse, RuntimeState, ScalarValue, Severity,
    evaluate_condition,
};

pub const POLICY_VERSION: &str = "sentinel-safety-policy/v0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionKind {
    Accepted,
    Overridden,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecisionReason {
    RequestAllowed,
    RuleInhibit(String),
    SafetyProfile(String),
    SafetyDefault,
    AbortLatched,
    AuthorityUnavailable,
    UnknownActuator,
    InvalidRequest,
    SupervisorOnly,
    MissingRequest,
}

impl DecisionReason {
    pub fn code(&self) -> &'static str {
        match self {
            Self::RequestAllowed => "OUTPUT_ACCEPTED",
            Self::RuleInhibit(_) => "OUTPUT_RULE_INHIBIT",
            Self::SafetyProfile(_) => "OUTPUT_SAFE_PROFILE",
            Self::SafetyDefault => "OUTPUT_SAFE_DEFAULT",
            Self::AbortLatched => "OUTPUT_ABORT_LATCH",
            Self::AuthorityUnavailable => "OUTPUT_AUTHORITY_UNAVAILABLE",
            Self::UnknownActuator => "OUTPUT_UNKNOWN_ACTUATOR",
            Self::InvalidRequest => "OUTPUT_INVALID_REQUEST",
            Self::SupervisorOnly => "OUTPUT_SUPERVISOR_ONLY",
            Self::MissingRequest => "OUTPUT_MISSING_REQUEST",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputDecision {
    pub actuator: String,
    pub requested: Option<ScalarValue>,
    pub effective: Option<ScalarValue>,
    pub kind: DecisionKind,
    pub reason: DecisionReason,
    pub policy_version: &'static str,
}

#[derive(Clone, Debug)]
pub struct GateInput<'a> {
    pub bundle: &'a CompiledBundle,
    pub state: &'a RuntimeState,
    pub active_rules: &'a [String],
    pub requests: &'a BTreeMap<String, ScalarValue>,
    pub current_outputs: &'a BTreeMap<String, ScalarValue>,
    pub ordinary_authority: bool,
    pub supervisor_profile: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GateError {
    UnknownRule(String),
    UnknownProfile(String),
    InvalidAction(String),
    Condition(String),
}

impl fmt::Display for GateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "safety gate error: {self:?}")
    }
}

impl std::error::Error for GateError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplacementDecision {
    Allowed,
    RejectedArmed,
}

#[derive(Clone, Debug, Default)]
pub struct OutputGate {
    preserve_ticks: BTreeMap<String, u32>,
}

impl OutputGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn evaluate(
        &mut self,
        input: &GateInput<'_>,
    ) -> Result<BTreeMap<String, OutputDecision>, GateError> {
        let activated = ActivatedPolicy::from_input(input)?;
        let mut references = input.requests.keys().cloned().collect::<BTreeSet<_>>();
        if !input.ordinary_authority || input.state.abort_latched || activated.requires_safe_state {
            references.extend(
                input
                    .bundle
                    .actuators
                    .keys()
                    .map(|id| format!("actuator.{id}")),
            );
        }

        let mut decisions = BTreeMap::new();
        for reference in references {
            let requested = input.requests.get(&reference).cloned();
            let Some(id) = reference.strip_prefix("actuator.") else {
                decisions.insert(
                    reference.clone(),
                    rejected(reference, requested, DecisionReason::UnknownActuator),
                );
                continue;
            };
            let Some(actuator) = input.bundle.actuators.get(id) else {
                decisions.insert(
                    reference.clone(),
                    rejected(reference, requested, DecisionReason::UnknownActuator),
                );
                continue;
            };
            let forced_reason = if !input.ordinary_authority {
                Some(DecisionReason::AuthorityUnavailable)
            } else if input.state.abort_latched {
                Some(DecisionReason::AbortLatched)
            } else {
                None
            };
            if forced_reason.is_some() || activated.requires_safe_state {
                let (effective, profile) = self.resolve_action(&reference, input, &activated)?;
                let reason = forced_reason.unwrap_or_else(|| {
                    profile.map_or(DecisionReason::SafetyDefault, DecisionReason::SafetyProfile)
                });
                decisions.insert(
                    reference.clone(),
                    OutputDecision {
                        actuator: reference,
                        requested,
                        effective: Some(effective),
                        kind: DecisionKind::Overridden,
                        reason,
                        policy_version: POLICY_VERSION,
                    },
                );
                continue;
            }

            if actuator.privilege == sentinel_scenario::Privilege::Supervisor {
                decisions.insert(
                    reference.clone(),
                    rejected(reference, requested, DecisionReason::SupervisorOnly),
                );
                continue;
            }

            if let Some(rule) = activated.inhibited.get(&reference) {
                decisions.insert(
                    reference.clone(),
                    rejected(
                        reference,
                        requested,
                        DecisionReason::RuleInhibit(rule.clone()),
                    ),
                );
                continue;
            }
            let Some(value) = requested else {
                decisions.insert(
                    reference.clone(),
                    rejected(reference, None, DecisionReason::MissingRequest),
                );
                continue;
            };
            if !valid_value(input.bundle, actuator, &value) {
                decisions.insert(
                    reference.clone(),
                    rejected(reference, Some(value), DecisionReason::InvalidRequest),
                );
                continue;
            }
            self.preserve_ticks.remove(&reference);
            decisions.insert(
                reference.clone(),
                OutputDecision {
                    actuator: reference,
                    requested: Some(value.clone()),
                    effective: Some(value),
                    kind: DecisionKind::Accepted,
                    reason: DecisionReason::RequestAllowed,
                    policy_version: POLICY_VERSION,
                },
            );
        }
        Ok(decisions)
    }

    fn resolve_action(
        &mut self,
        reference: &str,
        input: &GateInput<'_>,
        activated: &ActivatedPolicy,
    ) -> Result<(ScalarValue, Option<String>), GateError> {
        let mut profiles = activated.profiles.clone();
        if input.state.abort_latched {
            profiles.extend(
                input
                    .bundle
                    .safe_states
                    .profiles
                    .iter()
                    .filter(|(_, profile)| profile.layer == ProfileLayer::AbortLatch)
                    .map(|(id, _)| id.clone()),
            );
        }
        if let Some(profile) = input.supervisor_profile {
            if !input.bundle.safe_states.profiles.contains_key(profile) {
                return Err(GateError::UnknownProfile(profile.to_owned()));
            }
            profiles.insert(profile.to_owned());
        }
        for (id, profile) in &input.bundle.safe_states.profiles {
            if profile.layer == ProfileLayer::Hazard {
                let condition = profile
                    .when
                    .as_ref()
                    .ok_or_else(|| GateError::InvalidAction(id.clone()))?;
                if evaluate_condition(condition, input.state)
                    .map_err(|error| GateError::Condition(error.to_string()))?
                {
                    profiles.insert(id.clone());
                }
            }
        }

        let mut ordered = profiles
            .iter()
            .filter_map(|id| {
                input
                    .bundle
                    .safe_states
                    .profiles
                    .get(id)
                    .map(|profile| (id.clone(), profile))
            })
            .filter(|(_, profile)| {
                profile.layer != ProfileLayer::Phase
                    || profile.phase.as_deref() == Some(input.state.phase.as_str())
            })
            .collect::<Vec<_>>();
        ordered.sort_by(|(left_id, left), (right_id, right)| {
            profile_rank(right)
                .cmp(&profile_rank(left))
                .then_with(|| left_id.cmp(right_id))
        });

        if let Some((id, value)) = self.first_resolved(reference, input, &ordered)? {
            return Ok((value, Some(id)));
        }

        if let Some((id, profile)) =
            input
                .bundle
                .safe_states
                .profiles
                .iter()
                .find(|(_, profile)| {
                    profile.layer == ProfileLayer::Phase
                        && profile.phase.as_deref() == Some(input.state.phase.as_str())
                })
        {
            if let Some(value) =
                self.action_value(reference, input, profile.actions.get(reference))?
            {
                return Ok((value, Some(id.clone())));
            }
        }

        let action = input
            .bundle
            .safe_states
            .defaults
            .get(reference)
            .ok_or_else(|| GateError::InvalidAction(reference.to_owned()))?;
        self.action_value(reference, input, Some(action))?
            .map(|value| (value, None))
            .ok_or_else(|| GateError::InvalidAction(reference.to_owned()))
    }

    fn first_resolved(
        &mut self,
        reference: &str,
        input: &GateInput<'_>,
        profiles: &[(String, &sentinel_scenario::SafeProfile)],
    ) -> Result<Option<(String, ScalarValue)>, GateError> {
        for (id, profile) in profiles {
            if let Some(value) =
                self.action_value(reference, input, profile.actions.get(reference))?
            {
                return Ok(Some((id.clone(), value)));
            }
        }
        Ok(None)
    }

    fn action_value(
        &mut self,
        reference: &str,
        input: &GateInput<'_>,
        action: Option<&Action>,
    ) -> Result<Option<ScalarValue>, GateError> {
        match action {
            None => Ok(None),
            Some(Action::Set(value)) => {
                self.preserve_ticks.remove(reference);
                Ok(Some(value.clone()))
            }
            Some(Action::PreserveCurrent(preserve)) => {
                let ticks = self.preserve_ticks.entry(reference.to_owned()).or_default();
                *ticks = ticks.saturating_add(1);
                if *ticks <= preserve.max_ticks {
                    input
                        .current_outputs
                        .get(reference)
                        .cloned()
                        .map(Some)
                        .ok_or_else(|| GateError::InvalidAction(reference.to_owned()))
                } else {
                    Ok(None)
                }
            }
            Some(Action::Deenergize(_)) => Err(GateError::InvalidAction(reference.to_owned())),
        }
    }
}

pub fn replacement_decision(bundle: &CompiledBundle, state: &RuntimeState) -> ReplacementDecision {
    if bundle
        .phases
        .items
        .get(&state.phase)
        .is_some_and(|phase| phase.armed)
    {
        ReplacementDecision::RejectedArmed
    } else {
        ReplacementDecision::Allowed
    }
}

#[derive(Clone, Debug, Default)]
struct ActivatedPolicy {
    profiles: BTreeSet<String>,
    inhibited: BTreeMap<String, String>,
    requires_safe_state: bool,
}

impl ActivatedPolicy {
    fn from_input(input: &GateInput<'_>) -> Result<Self, GateError> {
        let mut policy = Self::default();
        for id in input.active_rules {
            let rule = input
                .bundle
                .rules
                .get(id)
                .ok_or_else(|| GateError::UnknownRule(id.clone()))?;
            match &rule.response {
                RuleResponse::Observe(_) | RuleResponse::Review { .. } => {}
                RuleResponse::Inhibit { targets } => {
                    for target in targets {
                        if target.starts_with("actuator.") {
                            policy.inhibited.insert(target.clone(), id.clone());
                        }
                    }
                }
                RuleResponse::Hold { profile }
                | RuleResponse::Abort { profile }
                | RuleResponse::Force { profile } => {
                    if !input.bundle.safe_states.profiles.contains_key(profile) {
                        return Err(GateError::UnknownProfile(profile.clone()));
                    }
                    policy.profiles.insert(profile.clone());
                    policy.requires_safe_state =
                        matches!(rule.severity, Severity::Hold | Severity::Abort)
                            || matches!(rule.response, RuleResponse::Force { .. });
                }
            }
        }
        if !input.ordinary_authority || input.state.abort_latched {
            policy.requires_safe_state = true;
        }
        Ok(policy)
    }
}

fn profile_rank(profile: &sentinel_scenario::SafeProfile) -> (u8, u8) {
    match profile.layer {
        ProfileLayer::Global => (5, 0),
        ProfileLayer::AbortLatch => (4, 0),
        ProfileLayer::Hazard => (3, profile.priority.unwrap_or_default()),
        ProfileLayer::Phase => (2, 0),
    }
}

fn valid_value(
    bundle: &CompiledBundle,
    actuator: &sentinel_scenario::Actuator,
    value: &ScalarValue,
) -> bool {
    match (actuator.scalar_type.as_str(), value) {
        ("bool", ScalarValue::Bool(_)) => true,
        ("i32", ScalarValue::Signed(number)) => actuator.range.as_ref().is_some_and(|range| {
            range.min <= i64::from(*number) && i64::from(*number) <= range.max
        }),
        ("i32", ScalarValue::Unsigned(number)) if *number <= i32::MAX as u32 => {
            actuator.range.as_ref().is_some_and(|range| {
                range.min <= i64::from(*number) && i64::from(*number) <= range.max
            })
        }
        ("u32", ScalarValue::Unsigned(number)) => actuator.range.as_ref().is_some_and(|range| {
            range.min <= i64::from(*number) && i64::from(*number) <= range.max
        }),
        ("u32", ScalarValue::Signed(number)) if *number >= 0 => {
            actuator.range.as_ref().is_some_and(|range| {
                range.min <= i64::from(*number) && i64::from(*number) <= range.max
            })
        }
        (name, ScalarValue::Enum(variant)) => bundle
            .types
            .get(name)
            .is_some_and(|definition| definition.variants.contains(variant)),
        _ => false,
    }
}

fn rejected(
    actuator: String,
    requested: Option<ScalarValue>,
    reason: DecisionReason,
) -> OutputDecision {
    OutputDecision {
        actuator,
        requested,
        effective: None,
        kind: DecisionKind::Rejected,
        reason,
        policy_version: POLICY_VERSION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_ai_check::{
        CheckRequest, CheckResponse, Finding, FindingProvenance, FindingStatus, Limits,
        Observation, PROMPT_CONTRACT, RESPONSE_SCHEMA, RULE_SET_SCHEMA, RuleSetContent,
        SNAPSHOT_SCHEMA, SnapshotContent, StateSnapshot, StateValue, WrittenRule, WrittenRuleSet,
        parse_and_validate_response,
    };
    use sentinel_scenario::{AllocationMode, Runtime, compile};

    const ROCKET: &str = include_str!("../../../examples/rocket-launch-default.yaml");

    fn bundle() -> CompiledBundle {
        compile(ROCKET, AllocationMode::Clean)
            .unwrap_or_else(|error| panic!("{error}"))
            .bundle
    }

    fn request(value: &str) -> BTreeMap<String, ScalarValue> {
        BTreeMap::from([(
            "actuator.ignition".to_owned(),
            ScalarValue::Enum(value.to_owned()),
        )])
    }

    fn current_outputs(state: &RuntimeState) -> BTreeMap<String, ScalarValue> {
        state
            .values
            .iter()
            .filter(|(reference, _)| reference.starts_with("actuator."))
            .map(|(reference, value)| (reference.clone(), value.clone()))
            .collect()
    }

    fn evaluate(
        gate: &mut OutputGate,
        bundle: &CompiledBundle,
        state: &RuntimeState,
        active_rules: &[&str],
        request_value: &str,
        authority: bool,
    ) -> BTreeMap<String, OutputDecision> {
        let rules = active_rules
            .iter()
            .map(|rule| (*rule).to_owned())
            .collect::<Vec<_>>();
        let requests = request(request_value);
        let current = current_outputs(state);
        gate.evaluate(&GateInput {
            bundle,
            state,
            active_rules: &rules,
            requests: &requests,
            current_outputs: &current,
            ordinary_authority: authority,
            supervisor_profile: None,
        })
        .unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn nominal_request_is_accepted_with_a_stable_reason() {
        let bundle = bundle();
        let state = Runtime::new(bundle.clone()).state().clone();
        let decisions = evaluate(&mut OutputGate::new(), &bundle, &state, &[], "safe", true);
        let decision = &decisions["actuator.ignition"];
        assert_eq!(decision.kind, DecisionKind::Accepted);
        assert_eq!(decision.reason.code(), "OUTPUT_ACCEPTED");
        assert_eq!(
            decision.effective,
            Some(ScalarValue::Enum("safe".to_owned()))
        );
    }

    #[test]
    fn invalid_and_inhibited_requests_fail_closed() {
        let mut bundle = bundle();
        let state = Runtime::new(bundle.clone()).state().clone();
        let invalid = evaluate(
            &mut OutputGate::new(),
            &bundle,
            &state,
            &[],
            "invented",
            true,
        );
        assert_eq!(invalid["actuator.ignition"].kind, DecisionKind::Rejected);
        assert_eq!(
            invalid["actuator.ignition"].reason,
            DecisionReason::InvalidRequest
        );

        let rule = bundle
            .rules
            .get_mut("armed_update_forbidden")
            .expect("armed update rule");
        rule.severity = Severity::Inhibit;
        rule.response = RuleResponse::Inhibit {
            targets: vec!["actuator.ignition".to_owned()],
        };
        let inhibited = evaluate(
            &mut OutputGate::new(),
            &bundle,
            &state,
            &["armed_update_forbidden"],
            "safe",
            true,
        );
        assert_eq!(inhibited["actuator.ignition"].kind, DecisionKind::Rejected);
        assert_eq!(
            inhibited["actuator.ignition"].reason.code(),
            "OUTPUT_RULE_INHIBIT"
        );
    }

    #[test]
    fn pressure_valve_power_interlock_and_staleness_rules_force_safe_output() {
        let bundle = bundle();
        let state = Runtime::new(bundle.clone()).state().clone();
        for rule in [
            "launch_pressure_invalid",
            "fill_ignition_conflict",
            "ignition_outside_launch_phase",
            "valve_feedback_mismatch",
            "electrical_unavailable",
            "launch_interlock_missing",
            "stale_safety_input",
        ] {
            let decisions = evaluate(
                &mut OutputGate::new(),
                &bundle,
                &state,
                &[rule],
                "firing",
                true,
            );
            let decision = &decisions["actuator.ignition"];
            assert_eq!(decision.kind, DecisionKind::Overridden, "{rule}");
            assert_eq!(
                decision.effective,
                Some(ScalarValue::Enum("safe".to_owned())),
                "{rule}"
            );
            assert_eq!(decision.reason.code(), "OUTPUT_SAFE_PROFILE", "{rule}");
        }
    }

    #[test]
    fn abort_latch_and_authority_loss_cannot_be_cleared_by_requests() {
        let bundle = bundle();
        let mut state = Runtime::new(bundle.clone()).state().clone();
        state.abort_latched = true;
        let decisions = evaluate(&mut OutputGate::new(), &bundle, &state, &[], "firing", true);
        let decision = &decisions["actuator.ignition"];
        assert_eq!(decision.kind, DecisionKind::Overridden);
        assert_eq!(decision.reason, DecisionReason::AbortLatched);
        assert_eq!(
            decision.effective,
            Some(ScalarValue::Enum("safe".to_owned()))
        );

        state.abort_latched = false;
        let decisions = evaluate(
            &mut OutputGate::new(),
            &bundle,
            &state,
            &[],
            "firing",
            false,
        );
        assert_eq!(
            decisions["actuator.ignition"].reason,
            DecisionReason::AuthorityUnavailable
        );
    }

    #[test]
    fn safe_state_precedence_is_abort_then_hazard_then_phase_then_default() {
        let mut bundle = bundle();
        bundle
            .safe_states
            .profiles
            .get_mut("critical_pressure")
            .expect("critical profile")
            .actions
            .insert(
                "actuator.vent_valve".to_owned(),
                Action::Set(ScalarValue::Enum("closed".to_owned())),
            );
        let mut state = Runtime::new(bundle.clone()).state().clone();
        state.abort_latched = true;
        let rules = vec!["critical_pressure".to_owned()];
        let requests = BTreeMap::from([(
            "actuator.vent_valve".to_owned(),
            ScalarValue::Enum("closed".to_owned()),
        )]);
        let current = current_outputs(&state);
        let decisions = OutputGate::new()
            .evaluate(&GateInput {
                bundle: &bundle,
                state: &state,
                active_rules: &rules,
                requests: &requests,
                current_outputs: &current,
                ordinary_authority: true,
                supervisor_profile: None,
            })
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            decisions["actuator.vent_valve"].effective,
            Some(ScalarValue::Enum("open".to_owned()))
        );
        assert_eq!(
            decisions["actuator.vent_valve"].reason,
            DecisionReason::AbortLatched
        );
    }

    #[test]
    fn armed_phase_rejects_replacement_and_idle_allows_it() {
        let bundle = bundle();
        let mut state = Runtime::new(bundle.clone()).state().clone();
        assert_eq!(
            replacement_decision(&bundle, &state),
            ReplacementDecision::Allowed
        );
        state.phase = "armed".to_owned();
        assert_eq!(
            replacement_decision(&bundle, &state),
            ReplacementDecision::RejectedArmed
        );
    }

    #[test]
    fn advisory_findings_have_no_input_path_to_output_authority() {
        let compilation =
            compile(ROCKET, AllocationMode::Clean).unwrap_or_else(|error| panic!("{error}"));
        let bundle_hash = compilation.bundle_hash;
        let bundle = compilation.bundle;
        let state = Runtime::new(bundle.clone()).state().clone();
        let mut gate = OutputGate::new();
        let before = evaluate(
            &mut gate,
            &bundle,
            &state,
            &["ignition_outside_launch_phase"],
            "firing",
            true,
        );

        let ignition = match state.values.get("actuator.ignition") {
            Some(ScalarValue::Enum(value)) => value.clone(),
            value => panic!("unexpected ignition state: {value:?}"),
        };

        let snapshot = StateSnapshot::new(SnapshotContent {
            schema: SNAPSHOT_SCHEMA.to_owned(),
            version: 1,
            scenario_id: bundle.scenario_id.clone(),
            scenario_bundle_hash: bundle_hash,
            observation: Observation {
                tick: Some(state.tick),
                unix_time_ms: None,
            },
            fields: BTreeMap::from([("actuator.ignition".to_owned(), StateValue::Text(ignition))]),
            missing_fields: Vec::new(),
        })
        .unwrap_or_else(|error| panic!("{error}"));
        let rules = WrittenRuleSet::new(RuleSetContent {
            schema: RULE_SET_SCHEMA.to_owned(),
            version: 1,
            rules: vec![WrittenRule {
                id: "ignition_advisory".to_owned(),
                deterministic_rule_id: Some("fill_ignition_conflict".to_owned()),
                text: "Compare the ignition state with the written operating rule.".to_owned(),
                required_fields: vec!["actuator.ignition".to_owned()],
            }],
        })
        .unwrap_or_else(|error| panic!("{error}"));
        let check = CheckRequest { snapshot, rules };
        let response = CheckResponse {
            schema: RESPONSE_SCHEMA.to_owned(),
            snapshot_hash: check.snapshot.hash.clone(),
            rule_set_hash: check.rules.hash.clone(),
            provenance: FindingProvenance {
                backend: "fixture".to_owned(),
                model: "untrusted-fixture".to_owned(),
                prompt_contract: PROMPT_CONTRACT.to_owned(),
            },
            findings: vec![Finding {
                rule_id: "ignition_advisory".to_owned(),
                status: FindingStatus::NoIssueObserved,
                cited_fields: vec!["actuator.ignition".to_owned()],
                rationale: "The untrusted checker reports no issue.".to_owned(),
            }],
        };
        let bytes = serde_json::to_vec(&response).unwrap_or_else(|error| panic!("{error}"));
        let _finding = parse_and_validate_response(&bytes, &check, Limits::default())
            .unwrap_or_else(|error| panic!("{error}"));

        let after = evaluate(
            &mut gate,
            &bundle,
            &state,
            &["ignition_outside_launch_phase"],
            "firing",
            true,
        );
        assert_eq!(before, after);
        assert_eq!(after["actuator.ignition"].kind, DecisionKind::Overridden);
        assert_eq!(
            after["actuator.ignition"].effective,
            Some(ScalarValue::Enum("safe".to_owned()))
        );
    }
}
