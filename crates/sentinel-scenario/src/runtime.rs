use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use crate::compiler::CompiledBundle;
use crate::model::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TickInput {
    pub channel_updates: BTreeMap<String, ScalarValue>,
    pub actuator_requests: BTreeMap<String, ScalarValue>,
    pub active_faults: BTreeSet<String>,
    pub transition: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeState {
    pub tick: u64,
    pub phase: String,
    pub phase_ticks: u32,
    pub values: BTreeMap<String, ScalarValue>,
    pub ages: BTreeMap<String, u32>,
    pub active_faults: BTreeSet<String>,
    pub abort_latched: bool,
    pub hold: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TickResult {
    pub tick: u64,
    pub phase_before: String,
    pub phase_after: String,
    pub active_rules: Vec<String>,
    pub active_faults: Vec<String>,
    pub changed_values: BTreeMap<String, ScalarValue>,
    pub abort_latched: bool,
    pub hold: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeError {
    UnknownInput(String),
    InvalidValue(String),
    ArithmeticOverflow(String),
    TransitionAmbiguous(Vec<String>),
    UnknownTransition(String),
    Expression(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "scenario runtime error: {self:?}")
    }
}

impl std::error::Error for RuntimeError {}

pub fn evaluate_condition(
    expression: &Expression,
    state: &RuntimeState,
) -> Result<bool, RuntimeError> {
    eval_bool(
        expression,
        &state.values,
        &state.ages,
        &state.phase,
        &state.active_faults,
    )
}

#[derive(Clone, Debug)]
pub struct Runtime {
    bundle: CompiledBundle,
    state: RuntimeState,
    delay_queues: BTreeMap<String, VecDeque<ScalarValue>>,
    fired_one_shot: BTreeSet<String>,
}

impl Runtime {
    pub fn new(bundle: CompiledBundle) -> Self {
        let mut values = BTreeMap::new();
        let mut ages = BTreeMap::new();
        for (id, channel) in &bundle.telemetry {
            let reference = format!("telemetry.{id}");
            values.insert(reference.clone(), channel.initial.clone());
            ages.insert(reference, 0);
        }
        for (id, channel) in &bundle.feedback {
            let reference = format!("feedback.{id}");
            values.insert(reference.clone(), channel.initial.clone());
            ages.insert(reference, 0);
        }
        for (id, actuator) in &bundle.actuators {
            values.insert(format!("actuator.{id}"), actuator.initial.clone());
        }
        let phase = bundle.phases.initial.clone();
        Self {
            bundle,
            state: RuntimeState {
                tick: 0,
                phase,
                phase_ticks: 0,
                values,
                ages,
                active_faults: BTreeSet::new(),
                abort_latched: false,
                hold: false,
            },
            delay_queues: BTreeMap::new(),
            fired_one_shot: BTreeSet::new(),
        }
    }

    pub fn state(&self) -> &RuntimeState {
        &self.state
    }

    pub fn bundle(&self) -> &CompiledBundle {
        &self.bundle
    }

    pub fn tick(&mut self, input: &TickInput) -> Result<TickResult, RuntimeError> {
        let before = self.state.clone();
        let phase_before = before.phase.clone();
        let mut values = before.values.clone();
        let mut ages = before.ages.clone();
        for age in ages.values_mut() {
            *age = age.saturating_add(1);
        }

        for (reference, value) in &input.channel_updates {
            if !reference.starts_with("telemetry.") && !reference.starts_with("feedback.") {
                return Err(RuntimeError::UnknownInput(reference.clone()));
            }
            self.validate_runtime_value(reference, value)?;
            if !values.contains_key(reference) {
                return Err(RuntimeError::UnknownInput(reference.clone()));
            }
            values.insert(reference.clone(), value.clone());
            ages.insert(reference.clone(), 0);
        }
        for (reference, value) in &input.actuator_requests {
            if !reference.starts_with("actuator.") || !values.contains_key(reference) {
                return Err(RuntimeError::UnknownInput(reference.clone()));
            }
            self.validate_runtime_value(reference, value)?;
            values.insert(reference.clone(), value.clone());
        }

        let active_faults = self.resolve_faults(input);
        for id in &active_faults {
            let Some(fault) = self.bundle.faults.get(id) else {
                continue;
            };
            for effect in &fault.effects {
                if matches!(
                    effect,
                    FaultEffect::Freeze { .. } | FaultEffect::DropUpdates { .. }
                ) {
                    let target = fault_target(effect);
                    if let Some(value) = before.values.get(target) {
                        values.insert(target.to_owned(), value.clone());
                    }
                    if let Some(age) = before.ages.get(target) {
                        ages.insert(target.to_owned(), age.saturating_add(1));
                    }
                }
            }
        }
        let snapshot = values.clone();
        let mut proposed = BTreeMap::new();
        let dynamics = self.bundle.dynamics.clone();
        for (id, dynamic) in &dynamics {
            if dynamic.when.as_ref().is_some_and(|condition| {
                !eval_bool(condition, &snapshot, &ages, &before.phase, &active_faults)
                    .unwrap_or(false)
            }) {
                continue;
            }
            if let Some(value) = self.evaluate_dynamic(id, dynamic, &snapshot, input)? {
                proposed.insert(dynamic.target.clone(), value);
            }
        }

        self.apply_faults(&active_faults, &snapshot, &mut proposed)?;
        for (reference, value) in proposed {
            if values.get(&reference) != Some(&value) {
                values.insert(reference.clone(), value);
            }
            ages.insert(reference, 0);
        }

        let mut next_phase = before.phase.clone();
        let phase = self
            .bundle
            .phases
            .items
            .get(&before.phase)
            .cloned()
            .ok_or_else(|| {
                RuntimeError::Expression("current phase is absent from bundle".to_owned())
            })?;

        let mut hold = false;
        let mut abort_latched = before.abort_latched;
        let mut active_rules = Vec::new();
        for (id, rule) in &self.bundle.rules {
            if rule_in_scope(rule, &before.phase)
                && eval_bool(
                    &rule.condition,
                    &values,
                    &ages,
                    &before.phase,
                    &active_faults,
                )?
            {
                active_rules.push(id.clone());
                match rule.severity {
                    Severity::Hold => hold = true,
                    Severity::Abort => abort_latched = true,
                    Severity::Advisory | Severity::Inhibit => {}
                }
            }
        }

        let mut selected: Option<Transition> = None;
        if let Some(requested) = &input.transition {
            let transition = phase
                .transitions
                .iter()
                .find(|transition| transition.id == *requested)
                .cloned()
                .ok_or_else(|| RuntimeError::UnknownTransition(requested.clone()))?;
            if transition.trigger == TransitionTrigger::Automatic {
                return Err(RuntimeError::UnknownTransition(requested.clone()));
            }
            if eval_bool(
                &transition.guard,
                &values,
                &ages,
                &before.phase,
                &active_faults,
            )? {
                match transition.trigger {
                    TransitionTrigger::Hold => hold = true,
                    TransitionTrigger::Abort => abort_latched = true,
                    TransitionTrigger::Automatic | TransitionTrigger::Supervisor => {}
                }
                selected = Some(transition);
            }
        } else if !hold && !abort_latched {
            let complete = eval_bool(
                &phase.completion,
                &values,
                &ages,
                &before.phase,
                &active_faults,
            )?;
            let mut automatic = Vec::new();
            for transition in &phase.transitions {
                if complete
                    && transition.trigger == TransitionTrigger::Automatic
                    && eval_bool(
                        &transition.guard,
                        &values,
                        &ages,
                        &before.phase,
                        &active_faults,
                    )?
                {
                    automatic.push(transition.clone());
                }
            }
            if automatic.len() > 1 {
                let mut ids = automatic
                    .iter()
                    .map(|transition| transition.id.clone())
                    .collect::<Vec<_>>();
                ids.sort();
                return Err(RuntimeError::TransitionAmbiguous(ids));
            }
            selected = automatic.pop();
        }

        if selected.is_none()
            && phase.timeout_ticks > 0
            && before.phase_ticks.saturating_add(1) >= phase.timeout_ticks
        {
            match &phase.on_timeout {
                TimeoutAction::Builtin(TimeoutBuiltin::Hold) => hold = true,
                TimeoutAction::Builtin(TimeoutBuiltin::Abort) => abort_latched = true,
                TimeoutAction::Transition { transition } => {
                    let target = phase
                        .transitions
                        .iter()
                        .find(|candidate| {
                            candidate.id == *transition
                                && candidate.trigger == TransitionTrigger::Supervisor
                        })
                        .cloned()
                        .ok_or_else(|| RuntimeError::UnknownTransition(transition.clone()))?;
                    selected = Some(target);
                }
            }
        }

        let new_attempt = selected
            .as_ref()
            .is_some_and(|transition| transition.new_attempt);
        if let Some(transition) = &selected {
            next_phase.clone_from(&transition.to);
        }
        if next_phase != before.phase {
            let target = self.bundle.phases.items.get(&next_phase).ok_or_else(|| {
                RuntimeError::Expression("transition target is absent from bundle".to_owned())
            })?;
            if !eval_bool(&target.entry, &values, &ages, &next_phase, &active_faults)? {
                return Err(RuntimeError::Expression(format!(
                    "entry condition for phase `{next_phase}` is false"
                )));
            }
        }
        if new_attempt {
            abort_latched = false;
            hold = false;
            self.delay_queues.clear();
            self.fired_one_shot.clear();
        }

        let phase_ticks = if next_phase == before.phase {
            before.phase_ticks.saturating_add(1)
        } else {
            0
        };
        self.state = RuntimeState {
            tick: before.tick.saturating_add(1),
            phase: next_phase.clone(),
            phase_ticks,
            values,
            ages,
            active_faults: active_faults.clone(),
            abort_latched,
            hold,
        };
        let changed_values = self
            .state
            .values
            .iter()
            .filter(|(reference, value)| before.values.get(*reference) != Some(*value))
            .map(|(reference, value)| (reference.clone(), value.clone()))
            .collect();
        Ok(TickResult {
            tick: before.tick,
            phase_before,
            phase_after: next_phase,
            active_rules,
            active_faults: active_faults.into_iter().collect(),
            changed_values,
            abort_latched,
            hold,
        })
    }

    fn resolve_faults(&mut self, input: &TickInput) -> BTreeSet<String> {
        let mut active = BTreeSet::new();
        for (id, fault) in &self.bundle.faults {
            let selected = match &fault.activation {
                FaultActivation::Mode(FaultMode::Fixture | FaultMode::Supervisor) => {
                    input.active_faults.contains(id)
                }
                FaultActivation::Window { window } => {
                    (window.start_tick..=window.end_tick).contains(&self.state.tick)
                }
            };
            if selected && !(fault.one_shot && self.fired_one_shot.contains(id)) {
                active.insert(id.clone());
                if fault.one_shot {
                    self.fired_one_shot.insert(id.clone());
                }
            }
        }
        active
    }

    fn evaluate_dynamic(
        &mut self,
        id: &str,
        dynamic: &DynamicRule,
        snapshot: &BTreeMap<String, ScalarValue>,
        input: &TickInput,
    ) -> Result<Option<ScalarValue>, RuntimeError> {
        let current = snapshot
            .get(&dynamic.target)
            .ok_or_else(|| RuntimeError::UnknownInput(dynamic.target.clone()))?;
        let value = match &dynamic.operation {
            DynamicOperation::Set(value) => value.clone(),
            DynamicOperation::Copy(reference) => snapshot
                .get(reference)
                .cloned()
                .ok_or_else(|| RuntimeError::UnknownInput(reference.clone()))?,
            DynamicOperation::AddClamped(amount) => {
                self.clamp_numeric(&dynamic.target, numeric(current)? + i64::from(*amount))?
            }
            DynamicOperation::Approach(approach) => {
                let target = if let Some(reference) = &approach.reference {
                    snapshot
                        .get(reference)
                        .ok_or_else(|| RuntimeError::UnknownInput(reference.clone()))?
                } else {
                    approach
                        .value
                        .as_ref()
                        .ok_or_else(|| RuntimeError::InvalidValue(dynamic.target.clone()))?
                };
                let current_number = numeric(current)?;
                let target_number = numeric(target)?;
                let step = i64::from(approach.step);
                let moved = if current_number < target_number {
                    current_number.saturating_add(step).min(target_number)
                } else {
                    current_number.saturating_sub(step).max(target_number)
                };
                self.clamp_numeric(&dynamic.target, moved)?
            }
            DynamicOperation::MapEnum(mapping) => {
                let source = snapshot
                    .get(&mapping.source)
                    .ok_or_else(|| RuntimeError::UnknownInput(mapping.source.clone()))?;
                let ScalarValue::Enum(variant) = source else {
                    return Err(RuntimeError::InvalidValue(mapping.source.clone()));
                };
                mapping
                    .values
                    .get(variant)
                    .cloned()
                    .ok_or_else(|| RuntimeError::InvalidValue(mapping.source.clone()))?
            }
            DynamicOperation::ConfirmAfter(confirm) => {
                let requested = input
                    .actuator_requests
                    .get(&confirm.actuator)
                    .or_else(|| snapshot.get(&confirm.actuator))
                    .cloned()
                    .ok_or_else(|| RuntimeError::UnknownInput(confirm.actuator.clone()))?;
                let queue = self
                    .delay_queues
                    .entry(format!("dynamic:{id}"))
                    .or_default();
                queue.push_back(requested);
                if queue.len() <= confirm.delay_ticks as usize {
                    return Ok(None);
                }
                queue
                    .pop_front()
                    .ok_or_else(|| RuntimeError::Expression(id.to_owned()))?
            }
        };
        self.validate_runtime_value(&dynamic.target, &value)?;
        Ok(Some(value))
    }

    fn apply_faults(
        &mut self,
        active: &BTreeSet<String>,
        snapshot: &BTreeMap<String, ScalarValue>,
        proposed: &mut BTreeMap<String, ScalarValue>,
    ) -> Result<(), RuntimeError> {
        let mut faults = active
            .iter()
            .filter_map(|id| self.bundle.faults.get(id).map(|fault| (id, fault)))
            .collect::<Vec<_>>();
        faults.sort_by_key(|(_, fault)| std::cmp::Reverse(fault.priority));
        let mut controlled = BTreeSet::new();
        for (id, fault) in faults {
            for effect in &fault.effects {
                let target = fault_target(effect);
                if !controlled.insert(target.to_owned()) {
                    continue;
                }
                match effect {
                    FaultEffect::Override { value, .. } => {
                        proposed.insert(target.to_owned(), value.clone());
                    }
                    FaultEffect::BiasClamped { amount, .. } => {
                        let base = proposed
                            .get(target)
                            .or_else(|| snapshot.get(target))
                            .ok_or_else(|| RuntimeError::UnknownInput(target.to_owned()))?;
                        let value =
                            self.clamp_numeric(target, numeric(base)? + i64::from(*amount))?;
                        proposed.insert(target.to_owned(), value);
                    }
                    FaultEffect::Freeze { .. } | FaultEffect::DropUpdates { .. } => {
                        proposed.remove(target);
                    }
                    FaultEffect::DelayFeedback { ticks, .. } => {
                        let Some(value) = proposed.remove(target) else {
                            continue;
                        };
                        let queue = self
                            .delay_queues
                            .entry(format!("fault:{id}:{target}"))
                            .or_default();
                        queue.push_back(value);
                        if queue.len() > *ticks as usize {
                            if let Some(delayed) = queue.pop_front() {
                                proposed.insert(target.to_owned(), delayed);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn clamp_numeric(&self, reference: &str, value: i64) -> Result<ScalarValue, RuntimeError> {
        let (scalar_type, range) = self.domain(reference)?;
        let range = range.ok_or_else(|| RuntimeError::InvalidValue(reference.to_owned()))?;
        let value = value.clamp(range.min, range.max);
        match scalar_type {
            "i32" => i32::try_from(value)
                .map(ScalarValue::Signed)
                .map_err(|_| RuntimeError::ArithmeticOverflow(reference.to_owned())),
            "u32" => u32::try_from(value)
                .map(ScalarValue::Unsigned)
                .map_err(|_| RuntimeError::ArithmeticOverflow(reference.to_owned())),
            _ => Err(RuntimeError::InvalidValue(reference.to_owned())),
        }
    }

    fn validate_runtime_value(
        &self,
        reference: &str,
        value: &ScalarValue,
    ) -> Result<(), RuntimeError> {
        let (scalar_type, range) = self.domain(reference)?;
        let valid = match (scalar_type, value) {
            ("bool", ScalarValue::Bool(_)) => true,
            ("i32", ScalarValue::Signed(number)) => range.is_some_and(|range| {
                range.min <= i64::from(*number) && i64::from(*number) <= range.max
            }),
            ("i32", ScalarValue::Unsigned(number)) if *number <= i32::MAX as u32 => range
                .is_some_and(|range| {
                    range.min <= i64::from(*number) && i64::from(*number) <= range.max
                }),
            ("u32", ScalarValue::Unsigned(number)) => range.is_some_and(|range| {
                range.min <= i64::from(*number) && i64::from(*number) <= range.max
            }),
            ("u32", ScalarValue::Signed(number)) if *number >= 0 => range.is_some_and(|range| {
                range.min <= i64::from(*number) && i64::from(*number) <= range.max
            }),
            (name, ScalarValue::Enum(variant)) => self
                .bundle
                .types
                .get(name)
                .is_some_and(|definition| definition.variants.contains(variant)),
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(RuntimeError::InvalidValue(reference.to_owned()))
        }
    }

    fn domain(&self, reference: &str) -> Result<(&str, Option<&NumericRange>), RuntimeError> {
        let Some((namespace, id)) = reference.split_once('.') else {
            return Err(RuntimeError::UnknownInput(reference.to_owned()));
        };
        match namespace {
            "telemetry" => self
                .bundle
                .telemetry
                .get(id)
                .map(|channel| (channel.scalar_type.as_str(), channel.range.as_ref())),
            "feedback" => self
                .bundle
                .feedback
                .get(id)
                .map(|channel| (channel.scalar_type.as_str(), channel.range.as_ref())),
            "actuator" => self
                .bundle
                .actuators
                .get(id)
                .map(|actuator| (actuator.scalar_type.as_str(), actuator.range.as_ref())),
            _ => None,
        }
        .ok_or_else(|| RuntimeError::UnknownInput(reference.to_owned()))
    }
}

fn fault_target(effect: &FaultEffect) -> &str {
    match effect {
        FaultEffect::Override { target, .. }
        | FaultEffect::BiasClamped { target, .. }
        | FaultEffect::Freeze { target }
        | FaultEffect::DropUpdates { target }
        | FaultEffect::DelayFeedback { target, .. } => target,
    }
}

fn numeric(value: &ScalarValue) -> Result<i64, RuntimeError> {
    match value {
        ScalarValue::Signed(value) => Ok(i64::from(*value)),
        ScalarValue::Unsigned(value) => Ok(i64::from(*value)),
        _ => Err(RuntimeError::InvalidValue(
            "expected numeric value".to_owned(),
        )),
    }
}

fn rule_in_scope(rule: &Rule, phase: &str) -> bool {
    match &rule.scope {
        RuleScope::Global(_) => true,
        RuleScope::Phases(phases) => phases.iter().any(|candidate| candidate == phase),
    }
}

fn eval_bool(
    expression: &Expression,
    values: &BTreeMap<String, ScalarValue>,
    ages: &BTreeMap<String, u32>,
    phase: &str,
    active_faults: &BTreeSet<String>,
) -> Result<bool, RuntimeError> {
    match eval(expression, values, ages, phase, active_faults)? {
        ScalarValue::Bool(value) => Ok(value),
        _ => Err(RuntimeError::Expression(
            "expected boolean expression".to_owned(),
        )),
    }
}

fn eval(
    expression: &Expression,
    values: &BTreeMap<String, ScalarValue>,
    ages: &BTreeMap<String, u32>,
    phase: &str,
    active_faults: &BTreeSet<String>,
) -> Result<ScalarValue, RuntimeError> {
    match expression {
        Expression::Const(constant) => Ok(constant.value.clone()),
        Expression::Ref(reference) => values
            .get(reference)
            .cloned()
            .ok_or_else(|| RuntimeError::Expression(reference.clone())),
        Expression::Not(inner) => Ok(ScalarValue::Bool(!eval_bool(
            inner,
            values,
            ages,
            phase,
            active_faults,
        )?)),
        Expression::All(items) => {
            for item in items {
                if !eval_bool(item, values, ages, phase, active_faults)? {
                    return Ok(ScalarValue::Bool(false));
                }
            }
            Ok(ScalarValue::Bool(true))
        }
        Expression::Any(items) => {
            for item in items {
                if eval_bool(item, values, ages, phase, active_faults)? {
                    return Ok(ScalarValue::Bool(true));
                }
            }
            Ok(ScalarValue::Bool(false))
        }
        Expression::Compare(compare) => {
            let left = eval(&compare.left, values, ages, phase, active_faults)?;
            let right = eval(&compare.right, values, ages, phase, active_faults)?;
            Ok(ScalarValue::Bool(compare_values(
                compare.op, &left, &right,
            )?))
        }
        Expression::Between(between) => {
            let value = numeric(&eval(&between.value, values, ages, phase, active_faults)?)?;
            let min = numeric(&eval(&between.min, values, ages, phase, active_faults)?)?;
            let max = numeric(&eval(&between.max, values, ages, phase, active_faults)?)?;
            Ok(ScalarValue::Bool(min <= value && value <= max))
        }
        Expression::Fresh(fresh) => Ok(ScalarValue::Bool(
            ages.get(&fresh.r#ref)
                .is_some_and(|age| *age <= fresh.max_age_ticks),
        )),
        Expression::PhaseIs(expected) => Ok(ScalarValue::Bool(expected == phase)),
        Expression::FaultActive(fault) => Ok(ScalarValue::Bool(active_faults.contains(fault))),
    }
}

fn compare_values(
    op: CompareOp,
    left: &ScalarValue,
    right: &ScalarValue,
) -> Result<bool, RuntimeError> {
    match op {
        CompareOp::Eq => Ok(left == right),
        CompareOp::Ne => Ok(left != right),
        CompareOp::Lt | CompareOp::Le | CompareOp::Gt | CompareOp::Ge => {
            let left = numeric(left)?;
            let right = numeric(right)?;
            Ok(match op {
                CompareOp::Lt => left < right,
                CompareOp::Le => left <= right,
                CompareOp::Gt => left > right,
                CompareOp::Ge => left >= right,
                CompareOp::Eq | CompareOp::Ne => false,
            })
        }
    }
}
