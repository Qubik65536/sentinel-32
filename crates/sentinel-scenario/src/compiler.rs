use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser};
use yaml_rust2::scanner::{Marker, TScalarStyle};

use crate::model::*;
use crate::{COMPILER_CONTRACT, SCHEMA_ID};

const MAX_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_MAP_ENTRIES: usize = 4096;
const MAX_SEQUENCE_ENTRIES: usize = 16_384;
const MAX_DEPTH: usize = 32;
const MAX_ARTIFACT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}: {}: {}",
            self.line, self.column, self.code, self.message
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileError {
    pub diagnostics: Vec<Diagnostic>,
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index > 0 {
                formatter.write_str("\n")?;
            }
            write!(formatter, "{diagnostic}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CompileError {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AllocationKind {
    Telemetry,
    ActuatorRequest,
    Feedback,
    Supervisor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AllocationEntry {
    pub qualified_id: String,
    pub address: u32,
    pub kind: AllocationKind,
    pub scalar_type: String,
    pub unit: Unit,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PriorAllocation {
    pub scenario_id: String,
    pub publication: u32,
    pub bundle_hash: String,
    pub entries: Vec<AllocationEntry>,
}

pub enum AllocationMode<'a> {
    Clean,
    Stable {
        prior: &'a PriorAllocation,
        expected_bundle_hash: &'a str,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledBundle {
    pub schema: String,
    pub compiler_contract: String,
    pub scenario_id: String,
    pub publication: u32,
    pub semantic_hash: String,
    pub tick_ms: u32,
    pub types: BTreeMap<String, EnumDefinition>,
    pub telemetry: BTreeMap<String, Channel>,
    pub actuators: BTreeMap<String, Actuator>,
    pub feedback: BTreeMap<String, FeedbackChannel>,
    pub phases: Phases,
    pub rules: BTreeMap<String, Rule>,
    pub safe_states: SafeStates,
    pub dynamics: BTreeMap<String, DynamicRule>,
    pub faults: BTreeMap<String, Fault>,
    pub mmio: Vec<AllocationEntry>,
}

#[derive(Clone, Debug)]
pub struct Compilation {
    pub source: ScenarioSource,
    pub bundle: CompiledBundle,
    pub source_hash: String,
    pub semantic_hash: String,
    pub bundle_hash: String,
    pub presentation_hash: String,
    pub canonical_source: Vec<u8>,
    pub canonical_bundle: Vec<u8>,
    pub symbols: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareRegister {
    #[serde(rename = "type")]
    pub scalar_type: String,
    pub unit: Unit,
    #[serde(default)]
    pub range: Option<NumericRange>,
    pub initial: ScalarValue,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareSource {
    pub schema: String,
    pub id: String,
    pub publication: u32,
    #[serde(default)]
    pub types: BTreeMap<String, EnumDefinition>,
    #[serde(default)]
    pub telemetry: BTreeMap<String, HardwareRegister>,
    #[serde(default)]
    pub actuators: BTreeMap<String, HardwareRegister>,
    #[serde(default)]
    pub feedback: BTreeMap<String, HardwareRegister>,
}

pub fn compile_hardware(source_text: &str) -> Result<Compilation, CompileError> {
    let json_source = parse_source_json(source_text)?;
    let hardware: HardwareSource =
        serde_json::from_value(json_source).map_err(json_decode_error)?;
    if hardware.schema != "sentinel.hardware/v0" {
        return Err(one("SCHEMA_VERSION", "expected `sentinel.hardware/v0`"));
    }
    let registers = |items: &BTreeMap<String, HardwareRegister>, feedback: bool| {
        items
            .iter()
            .map(|(id, register)| {
                let mut value = json!({
                    "type": register.scalar_type,
                    "unit": register.unit,
                    "range": register.range,
                    "initial": register.initial,
                    "stale_after_ticks": 4294967295_u32
                });
                if feedback {
                    value["actuator"] = JsonValue::Null;
                    value["confirms"] = json!({"const": {"type": "bool", "value": true}});
                } else {
                    value["bands"] = json!({});
                }
                (id.clone(), value)
            })
            .collect::<serde_json::Map<_, _>>()
    };
    let actuators = hardware
        .actuators
        .iter()
        .map(|(id, register)| {
            (
                id.clone(),
                json!({
                    "type": register.scalar_type,
                    "unit": register.unit,
                    "range": register.range,
                    "initial": register.initial,
                    "safe_default": register.initial,
                    "deenergized": register.initial,
                    "privilege": "ordinary",
                    "reversibility": "reversible",
                    "feedback": null,
                    "feedback_timeout_ticks": null,
                    "mutually_exclusive": [],
                    "allow_preserve_current": false,
                    "command": "set"
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let mut rules = serde_json::Map::new();
    for reference in hardware
        .telemetry
        .keys()
        .map(|id| format!("telemetry.{id}"))
        .chain(hardware.feedback.keys().map(|id| format!("feedback.{id}")))
    {
        let id = format!("{}_freshness", reference.replace('.', "_"));
        rules.insert(id, json!({
            "condition": {"not": {"fresh": {"ref": reference, "max_age_ticks": 4294967295_u32}}},
            "scope": "global",
            "severity": "advisory",
            "response": {"observe": true},
            "message": "Generated hardware-manifest freshness observation."
        }));
    }
    let defaults = hardware
        .actuators
        .iter()
        .map(|(id, register)| (format!("actuator.{id}"), json!({"set": register.initial})))
        .collect::<serde_json::Map<_, _>>();
    let synthetic = json!({
        "schema": SCHEMA_ID,
        "id": hardware.id,
        "publication": hardware.publication,
        "tick_ms": 1,
        "metadata": {"name": "Hardware manifest", "description": "", "tags": []},
        "types": hardware.types,
        "telemetry": registers(&hardware.telemetry, false),
        "actuators": actuators,
        "feedback": registers(&hardware.feedback, true),
        "phases": {"initial": "hardware", "items": {"hardware": {
            "entry": {"const": {"type": "bool", "value": true}},
            "completion": {"const": {"type": "bool", "value": true}},
            "timeout_ticks": 0,
            "on_timeout": "hold",
            "transitions": [],
            "armed": false,
            "terminal": true,
            "abort_terminal": false
        }}},
        "rules": rules,
        "safe_states": {"defaults": defaults, "profiles": {}},
        "dynamics": {},
        "faults": {},
        "layout": null
    });
    let source = serde_json::to_string(&synthetic).map_err(internal_serialization)?;
    compile(&source, AllocationMode::Clean)
}

pub fn compile(source_text: &str, mode: AllocationMode<'_>) -> Result<Compilation, CompileError> {
    let source = parse_source(source_text)?;
    let mut diagnostics = Vec::new();
    validate_source(&source, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(CompileError { diagnostics });
    }

    let full_value = normalized_json(&source).map_err(internal_serialization)?;
    let canonical_source = canonical_json(&full_value);
    let source_hash = hash("sentinel32:scenario-source:v0", &canonical_source);

    let mut semantic_value = full_value.clone();
    if let JsonValue::Object(object) = &mut semantic_value {
        object.remove("metadata");
        object.remove("layout");
    }
    let semantic_hash = hash(
        "sentinel32:scenario-semantic:v0",
        &canonical_json(&semantic_value),
    );

    let presentation = serde_json::json!({
        "metadata": &source.metadata,
        "layout": &source.layout,
    });
    let presentation_hash = hash(
        "sentinel32:scenario-presentation:v0",
        &canonical_json(&normalize_json_value(presentation)),
    );

    let mmio = allocate(&source, mode)?;
    let mut compiled_safe_states = source.safe_states.clone();
    materialize_safe_actions(&source, &mut compiled_safe_states);
    let bundle = CompiledBundle {
        schema: SCHEMA_ID.to_owned(),
        compiler_contract: COMPILER_CONTRACT.to_owned(),
        scenario_id: source.id.clone(),
        publication: source.publication,
        semantic_hash: semantic_hash.clone(),
        tick_ms: source.tick_ms,
        types: source.types.clone(),
        telemetry: source.telemetry.clone(),
        actuators: source.actuators.clone(),
        feedback: source.feedback.clone(),
        phases: source.phases.clone(),
        rules: source.rules.clone(),
        safe_states: compiled_safe_states,
        dynamics: source.dynamics.clone(),
        faults: source.faults.clone(),
        mmio,
    };
    let bundle_value = normalized_json(&bundle).map_err(internal_serialization)?;
    let canonical_bundle = canonical_json(&bundle_value);
    if canonical_bundle.len() > MAX_ARTIFACT_BYTES {
        return Err(one("LIMIT_EXCEEDED", "compiled bundle exceeds 8 MiB"));
    }
    let bundle_hash = hash("sentinel32:scenario-bundle:v0", &canonical_bundle);
    let symbols = generate_symbols(&source.id, &bundle_hash, &bundle.mmio)?;
    Ok(Compilation {
        source,
        bundle,
        source_hash,
        semantic_hash,
        bundle_hash,
        presentation_hash,
        canonical_source,
        canonical_bundle,
        symbols,
    })
}

fn materialize_safe_actions(source: &ScenarioSource, safe_states: &mut SafeStates) {
    for (reference, action) in safe_states.defaults.iter_mut().chain(
        safe_states
            .profiles
            .values_mut()
            .flat_map(|profile| profile.actions.iter_mut()),
    ) {
        if matches!(action, Action::Deenergize(true)) {
            if let Some(value) = reference
                .strip_prefix("actuator.")
                .and_then(|id| source.actuators.get(id))
                .and_then(|actuator| actuator.deenergized.clone())
            {
                *action = Action::Set(value);
            }
        }
    }
}

fn parse_source(source: &str) -> Result<ScenarioSource, CompileError> {
    let json = parse_source_json(source)?;
    let mut typed: ScenarioSource = serde_json::from_value(json).map_err(json_decode_error)?;
    normalize_display_strings(&mut typed);
    Ok(typed)
}

fn parse_source_json(source: &str) -> Result<JsonValue, CompileError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(one("LIMIT_EXCEEDED", "scenario source exceeds 1 MiB"));
    }
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('%')
            || trimmed == "---"
            || trimmed == "..."
            || trimmed.starts_with("!!")
            || trimmed.starts_with('!')
            || trimmed.contains("<<:")
        {
            return Err(CompileError {
                diagnostics: vec![Diagnostic {
                    code: "UNKNOWN_FIELD",
                    line: index + 1,
                    column: line.len() - trimmed.len() + 1,
                    message:
                        "YAML directives, tags, merge keys, and document markers are forbidden"
                            .to_owned(),
                }],
            });
        }
    }
    parse_yaml_json(source)
}

fn normalize_display_strings(source: &mut ScenarioSource) {
    source.metadata.name = source.metadata.name.nfc().collect();
    source.metadata.description = source.metadata.description.nfc().collect();
    for tag in &mut source.metadata.tags {
        *tag = tag.nfc().collect();
    }
    for rule in source.rules.values_mut() {
        rule.message = rule.message.nfc().collect();
    }
    for action in source.safe_states.defaults.values_mut().chain(
        source
            .safe_states
            .profiles
            .values_mut()
            .flat_map(|profile| profile.actions.values_mut()),
    ) {
        if let Action::PreserveCurrent(preserve) = action {
            preserve.justification = preserve.justification.nfc().collect();
        }
    }
    if let Some(layout) = &mut source.layout {
        for panel in layout.panels.values_mut() {
            panel.title = panel.title.nfc().collect();
        }
    }
}

fn json_decode_error(error: serde_json::Error) -> CompileError {
    let message = error.to_string();
    let code = if message.contains("unknown field") {
        "UNKNOWN_FIELD"
    } else {
        "TYPE_MISMATCH"
    };
    one(code, &message)
}

#[derive(Default)]
struct YamlEvents {
    events: Vec<(Event, Marker)>,
}

impl MarkedEventReceiver for YamlEvents {
    fn on_event(&mut self, event: Event, marker: Marker) {
        self.events.push((event, marker));
    }
}

enum Frame {
    Sequence(Vec<JsonValue>),
    Mapping {
        values: serde_json::Map<String, JsonValue>,
        pending_key: Option<String>,
    },
}

fn parse_yaml_json(source: &str) -> Result<JsonValue, CompileError> {
    let mut receiver = YamlEvents::default();
    Parser::new_from_str(source)
        .load(&mut receiver, false)
        .map_err(|error| one("TYPE_MISMATCH", &error.to_string()))?;
    let mut stack = Vec::new();
    let mut root = None;
    let mut map_entries = 0_usize;
    let mut sequence_entries = 0_usize;
    for (event, marker) in receiver.events {
        match event {
            Event::MappingStart(anchor, tag) => {
                reject_anchor_tag(anchor, tag.is_some(), marker)?;
                if stack.len() >= MAX_DEPTH {
                    return Err(at("LIMIT_EXCEEDED", marker, "YAML nesting exceeds 32"));
                }
                stack.push(Frame::Mapping {
                    values: serde_json::Map::new(),
                    pending_key: None,
                });
            }
            Event::SequenceStart(anchor, tag) => {
                reject_anchor_tag(anchor, tag.is_some(), marker)?;
                if stack.len() >= MAX_DEPTH {
                    return Err(at("LIMIT_EXCEEDED", marker, "YAML nesting exceeds 32"));
                }
                stack.push(Frame::Sequence(Vec::new()));
            }
            Event::MappingEnd => {
                let Some(Frame::Mapping {
                    values,
                    pending_key,
                }) = stack.pop()
                else {
                    return Err(at("TYPE_MISMATCH", marker, "unexpected YAML mapping end"));
                };
                if pending_key.is_some() {
                    return Err(at("TYPE_MISMATCH", marker, "YAML map key has no value"));
                }
                attach_json(
                    JsonValue::Object(values),
                    marker,
                    &mut stack,
                    &mut root,
                    &mut map_entries,
                    &mut sequence_entries,
                )?;
            }
            Event::SequenceEnd => {
                let Some(Frame::Sequence(values)) = stack.pop() else {
                    return Err(at("TYPE_MISMATCH", marker, "unexpected YAML sequence end"));
                };
                attach_json(
                    JsonValue::Array(values),
                    marker,
                    &mut stack,
                    &mut root,
                    &mut map_entries,
                    &mut sequence_entries,
                )?;
            }
            Event::Scalar(value, style, anchor, tag) => {
                reject_anchor_tag(anchor, tag.is_some(), marker)?;
                let value = scalar_json(value, style, marker)?;
                attach_json(
                    value,
                    marker,
                    &mut stack,
                    &mut root,
                    &mut map_entries,
                    &mut sequence_entries,
                )?;
            }
            Event::Alias(_) => {
                return Err(at("UNKNOWN_FIELD", marker, "YAML aliases are forbidden"));
            }
            Event::Nothing
            | Event::StreamStart
            | Event::StreamEnd
            | Event::DocumentStart
            | Event::DocumentEnd => {}
        }
    }
    root.ok_or_else(|| one("TYPE_MISMATCH", "YAML document is empty"))
}

fn reject_anchor_tag(anchor: usize, tagged: bool, marker: Marker) -> Result<(), CompileError> {
    if anchor != 0 || tagged {
        Err(at(
            "UNKNOWN_FIELD",
            marker,
            "YAML anchors, aliases, and tags are forbidden",
        ))
    } else {
        Ok(())
    }
}

fn attach_json(
    value: JsonValue,
    marker: Marker,
    stack: &mut [Frame],
    root: &mut Option<JsonValue>,
    map_entries: &mut usize,
    sequence_entries: &mut usize,
) -> Result<(), CompileError> {
    let Some(parent) = stack.last_mut() else {
        if root.replace(value).is_some() {
            return Err(at("TYPE_MISMATCH", marker, "multiple YAML root values"));
        }
        return Ok(());
    };
    match parent {
        Frame::Sequence(values) => {
            *sequence_entries = sequence_entries.saturating_add(1);
            if *sequence_entries > MAX_SEQUENCE_ENTRIES {
                return Err(at(
                    "LIMIT_EXCEEDED",
                    marker,
                    "YAML sequence entries exceed 16384",
                ));
            }
            values.push(value);
        }
        Frame::Mapping {
            values,
            pending_key,
        } => {
            if let Some(key) = pending_key.take() {
                *map_entries = map_entries.saturating_add(1);
                if *map_entries > MAX_MAP_ENTRIES {
                    return Err(at("LIMIT_EXCEEDED", marker, "YAML map entries exceed 4096"));
                }
                if values.insert(key, value).is_some() {
                    return Err(at("DUPLICATE_KEY", marker, "duplicate YAML map key"));
                }
            } else if let JsonValue::String(key) = value {
                *pending_key = Some(key);
            } else {
                return Err(at(
                    "TYPE_MISMATCH",
                    marker,
                    "all YAML map keys must be strings",
                ));
            }
        }
    }
    Ok(())
}

fn scalar_json(
    value: String,
    style: TScalarStyle,
    marker: Marker,
) -> Result<JsonValue, CompileError> {
    if style != TScalarStyle::Plain {
        return Ok(JsonValue::String(value));
    }
    match value.as_str() {
        "null" => Ok(JsonValue::Null),
        "true" => Ok(JsonValue::Bool(true)),
        "false" => Ok(JsonValue::Bool(false)),
        _ => {
            if let Ok(value) = value.parse::<i64>() {
                return Ok(JsonValue::Number(value.into()));
            }
            if value
                .as_bytes()
                .first()
                .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'+' || *byte == b'-')
            {
                return Err(at(
                    "TYPE_MISMATCH",
                    marker,
                    "only decimal integers are permitted",
                ));
            }
            Ok(JsonValue::String(value))
        }
    }
}

fn at(code: &'static str, marker: Marker, message: &str) -> CompileError {
    CompileError {
        diagnostics: vec![Diagnostic {
            code,
            line: marker.line() + 1,
            column: marker.col() + 1,
            message: message.to_owned(),
        }],
    }
}

fn validate_source(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    if source.schema != SCHEMA_ID {
        push(diagnostics, "SCHEMA_VERSION", "unsupported scenario schema");
    }
    validate_id(&source.id, diagnostics);
    if source.publication == 0 {
        push(
            diagnostics,
            "PUBLICATION_ORDER",
            "publication must be positive",
        );
    }
    if !(1..=10_000).contains(&source.tick_ms) {
        push(diagnostics, "VALUE_RANGE", "tick_ms must be in 1..=10000");
    }
    validate_metadata(&source.metadata, diagnostics);
    validate_count(source.telemetry.len(), 1024, "telemetry", diagnostics);
    validate_count(source.feedback.len(), 1024, "feedback", diagnostics);
    validate_count(source.actuators.len(), 512, "actuators", diagnostics);
    validate_count(source.phases.items.len(), 256, "phases", diagnostics);
    validate_count(source.rules.len(), 2048, "rules", diagnostics);
    validate_count(
        source.safe_states.profiles.len(),
        512,
        "safe profiles",
        diagnostics,
    );
    validate_count(source.dynamics.len(), 2048, "dynamics", diagnostics);
    validate_count(source.faults.len(), 512, "faults", diagnostics);

    for (id, definition) in &source.types {
        validate_id(id, diagnostics);
        if definition.variants.is_empty() || definition.variants.len() > 64 {
            push(
                diagnostics,
                "LIMIT_EXCEEDED",
                "enum must have 1..=64 variants",
            );
        }
        let mut variants = BTreeSet::new();
        for variant in &definition.variants {
            validate_id(variant, diagnostics);
            if !variants.insert(variant) {
                push(diagnostics, "DUPLICATE_KEY", "enum variant is duplicated");
            }
        }
    }
    for id in source
        .telemetry
        .keys()
        .chain(source.actuators.keys())
        .chain(source.feedback.keys())
    {
        validate_id(id, diagnostics);
    }
    for (id, channel) in &source.telemetry {
        validate_channel(id, channel, source, diagnostics);
    }
    for (id, channel) in &source.feedback {
        validate_feedback(id, channel, source, diagnostics);
    }
    for (id, actuator) in &source.actuators {
        validate_actuator(id, actuator, source, diagnostics);
    }
    validate_phases(source, diagnostics);
    validate_rules(source, diagnostics);
    validate_rule_coverage(source, diagnostics);
    validate_safe_states(source, diagnostics);
    validate_dynamics(source, diagnostics);
    validate_faults(source, diagnostics);
    validate_layout(source, diagnostics);
}

fn validate_metadata(metadata: &Metadata, diagnostics: &mut Vec<Diagnostic>) {
    let name_len = metadata.name.chars().count();
    if !(1..=128).contains(&name_len) || metadata.description.chars().count() > 2048 {
        push(
            diagnostics,
            "LIMIT_EXCEEDED",
            "metadata text exceeds its bound",
        );
    }
    if metadata.tags.len() > 32
        || metadata
            .tags
            .iter()
            .any(|tag| !(1..=32).contains(&tag.chars().count()))
    {
        push(
            diagnostics,
            "LIMIT_EXCEEDED",
            "metadata tags exceed their bounds",
        );
    }
}

fn validate_channel(
    id: &str,
    channel: &Channel,
    source: &ScenarioSource,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_domain(
        &channel.scalar_type,
        channel.unit,
        channel.range.as_ref(),
        &channel.initial,
        source,
        diagnostics,
    );
    if channel.stale_after_ticks == 0 {
        push(
            diagnostics,
            "VALUE_RANGE",
            &format!("telemetry.{id} has zero staleness"),
        );
    }
    let mut bands = channel.bands.iter().collect::<Vec<_>>();
    bands.sort_by_key(|(_, band)| band.min);
    let mut previous_end = None;
    for (band_id, band) in bands {
        validate_id(band_id, diagnostics);
        if band.min > band.max || previous_end.is_some_and(|end| band.min <= end) {
            push(
                diagnostics,
                "VALUE_RANGE",
                "telemetry bands overlap or are invalid",
            );
        }
        previous_end = Some(band.max);
    }
}

fn validate_feedback(
    id: &str,
    channel: &FeedbackChannel,
    source: &ScenarioSource,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_domain(
        &channel.scalar_type,
        channel.unit,
        channel.range.as_ref(),
        &channel.initial,
        source,
        diagnostics,
    );
    if channel.stale_after_ticks == 0 {
        push(
            diagnostics,
            "VALUE_RANGE",
            &format!("feedback.{id} has zero staleness"),
        );
    }
    if let Some(actuator) = &channel.actuator {
        check_reference(source, actuator, Some("actuator"), diagnostics);
        if let Some(actuator) = actuator
            .strip_prefix("actuator.")
            .and_then(|id| source.actuators.get(id))
        {
            if actuator.scalar_type != channel.scalar_type || actuator.unit != channel.unit {
                push(
                    diagnostics,
                    "TYPE_MISMATCH",
                    "feedback and actuator domains differ",
                );
            }
        }
    }
    if let Some(expression) = &channel.confirms {
        require_bool(expression, source, diagnostics);
    }
}

fn validate_actuator(
    id: &str,
    actuator: &Actuator,
    source: &ScenarioSource,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_domain(
        &actuator.scalar_type,
        actuator.unit,
        actuator.range.as_ref(),
        &actuator.initial,
        source,
        diagnostics,
    );
    validate_value(
        &actuator.scalar_type,
        actuator.range.as_ref(),
        &actuator.safe_default,
        source,
        diagnostics,
    );
    if let Some(value) = &actuator.deenergized {
        validate_value(
            &actuator.scalar_type,
            actuator.range.as_ref(),
            value,
            source,
            diagnostics,
        );
    }
    if let Some(feedback) = &actuator.feedback {
        check_reference(source, feedback, Some("feedback"), diagnostics);
        if let Some(channel) = feedback
            .strip_prefix("feedback.")
            .and_then(|id| source.feedback.get(id))
        {
            if actuator.scalar_type != channel.scalar_type || actuator.unit != channel.unit {
                push(
                    diagnostics,
                    "TYPE_MISMATCH",
                    "actuator and feedback domains differ",
                );
            }
        }
        if actuator
            .feedback_timeout_ticks
            .is_none_or(|value| value == 0)
        {
            push(
                diagnostics,
                "VALUE_RANGE",
                &format!("actuator.{id} needs a positive feedback timeout"),
            );
        }
    }
    for expression in &actuator.mutually_exclusive {
        require_bool(expression, source, diagnostics);
    }
    if let CommandBehavior::Pulse { pulse } = &actuator.command {
        if pulse.duration_ticks == 0 {
            push(
                diagnostics,
                "VALUE_RANGE",
                "pulse duration must be positive",
            );
        }
    }
}

fn validate_domain(
    scalar_type: &str,
    unit: Unit,
    range: Option<&NumericRange>,
    initial: &ScalarValue,
    source: &ScenarioSource,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let numeric = matches!(scalar_type, "i32" | "u32");
    if numeric != range.is_some() {
        push(
            diagnostics,
            "TYPE_MISMATCH",
            "numeric domains require range; bool/enum domains forbid it",
        );
    }
    if !numeric && unit != Unit::None {
        push(
            diagnostics,
            "UNIT_MISMATCH",
            "bool and enum domains require unit none",
        );
    }
    if let Some(range) = range {
        if range.min > range.max {
            push(
                diagnostics,
                "VALUE_RANGE",
                "numeric range minimum exceeds maximum",
            );
        }
    }
    validate_value(scalar_type, range, initial, source, diagnostics);
}

fn validate_value(
    scalar_type: &str,
    range: Option<&NumericRange>,
    value: &ScalarValue,
    source: &ScenarioSource,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let numeric = match (scalar_type, value) {
        ("bool", ScalarValue::Bool(_)) => return,
        ("i32", ScalarValue::Signed(value)) => Some(i64::from(*value)),
        ("i32", ScalarValue::Unsigned(value)) if *value <= i32::MAX as u32 => {
            Some(i64::from(*value))
        }
        ("u32", ScalarValue::Unsigned(value)) => Some(i64::from(*value)),
        ("u32", ScalarValue::Signed(value)) if *value >= 0 => Some(i64::from(*value)),
        (name, ScalarValue::Enum(variant)) if source.types.contains_key(name) => {
            if !source.types[name].variants.contains(variant) {
                push(diagnostics, "VALUE_RANGE", "unknown enum variant");
            }
            return;
        }
        (name, _)
            if !matches!(name, "bool" | "i32" | "u32") && !source.types.contains_key(name) =>
        {
            push(diagnostics, "TYPE_MISMATCH", "unknown scalar type");
            return;
        }
        _ => {
            push(
                diagnostics,
                "TYPE_MISMATCH",
                "value does not match scalar type",
            );
            return;
        }
    };
    if let (Some(number), Some(range)) = (numeric, range) {
        if !(range.min..=range.max).contains(&number) {
            push(
                diagnostics,
                "VALUE_RANGE",
                "numeric value is outside its domain",
            );
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExprType {
    scalar: String,
    unit: Option<Unit>,
}

fn expression_type(
    expression: &Expression,
    source: &ScenarioSource,
    depth: usize,
    nodes: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ExprType> {
    *nodes = nodes.saturating_add(1);
    if depth > 24 || *nodes > 256 {
        push(
            diagnostics,
            "EXPR_DEPTH",
            "expression exceeds depth or node limit",
        );
        return None;
    }
    match expression {
        Expression::Const(constant) => {
            validate_value(
                &constant.scalar_type,
                None,
                &constant.value,
                source,
                diagnostics,
            );
            Some(ExprType {
                scalar: constant.scalar_type.clone(),
                unit: None,
            })
        }
        Expression::Ref(reference) => reference_type(source, reference, diagnostics),
        Expression::Not(inner) => {
            require_bool_inner(inner, source, depth + 1, nodes, diagnostics);
            bool_type()
        }
        Expression::All(items) | Expression::Any(items) => {
            if items.is_empty() || items.len() > 64 {
                push(
                    diagnostics,
                    "EXPR_NOT_TOTAL",
                    "all/any require 1..=64 operands",
                );
            }
            for item in items {
                require_bool_inner(item, source, depth + 1, nodes, diagnostics);
            }
            bool_type()
        }
        Expression::Compare(compare) => {
            let left = expression_type(&compare.left, source, depth + 1, nodes, diagnostics);
            let right = expression_type(&compare.right, source, depth + 1, nodes, diagnostics);
            if let (Some(left), Some(right)) = (left, right) {
                compatible_types(&left, &right, diagnostics);
                if !matches!(compare.op, CompareOp::Eq | CompareOp::Ne)
                    && (left.scalar == "bool" || source.types.contains_key(&left.scalar))
                {
                    push(
                        diagnostics,
                        "TYPE_MISMATCH",
                        "bool and enum values are not ordered",
                    );
                }
            }
            bool_type()
        }
        Expression::Between(between) => {
            let value = expression_type(&between.value, source, depth + 1, nodes, diagnostics);
            let min = expression_type(&between.min, source, depth + 1, nodes, diagnostics);
            let max = expression_type(&between.max, source, depth + 1, nodes, diagnostics);
            if let (Some(value), Some(min), Some(max)) = (value, min, max) {
                compatible_types(&value, &min, diagnostics);
                compatible_types(&value, &max, diagnostics);
                if value.scalar == "bool" || source.types.contains_key(&value.scalar) {
                    push(
                        diagnostics,
                        "TYPE_MISMATCH",
                        "between requires numeric values",
                    );
                }
            }
            bool_type()
        }
        Expression::Fresh(fresh) => {
            let namespace = fresh.r#ref.split_once('.').map(|pair| pair.0);
            if !matches!(namespace, Some("telemetry" | "feedback")) {
                push(
                    diagnostics,
                    "REF_KIND",
                    "fresh requires telemetry or feedback",
                );
            }
            check_reference(source, &fresh.r#ref, None, diagnostics);
            bool_type()
        }
        Expression::PhaseIs(phase) => {
            if !source.phases.items.contains_key(phase) {
                push(diagnostics, "UNKNOWN_REF", "unknown phase in expression");
            }
            bool_type()
        }
        Expression::FaultActive(fault) => {
            if !source.faults.contains_key(fault) {
                push(diagnostics, "UNKNOWN_REF", "unknown fault in expression");
            }
            bool_type()
        }
    }
}

fn bool_type() -> Option<ExprType> {
    Some(ExprType {
        scalar: "bool".to_owned(),
        unit: Some(Unit::None),
    })
}

fn require_bool(
    expression: &Expression,
    source: &ScenarioSource,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut nodes = 0;
    require_bool_inner(expression, source, 0, &mut nodes, diagnostics);
}

fn require_bool_inner(
    expression: &Expression,
    source: &ScenarioSource,
    depth: usize,
    nodes: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if expression_type(expression, source, depth, nodes, diagnostics)
        .is_some_and(|kind| kind.scalar != "bool")
    {
        push(diagnostics, "TYPE_MISMATCH", "expression must be boolean");
    }
}

fn compatible_types(left: &ExprType, right: &ExprType, diagnostics: &mut Vec<Diagnostic>) {
    if left.scalar != right.scalar {
        push(
            diagnostics,
            "TYPE_MISMATCH",
            "expression operand types differ",
        );
    }
    if let (Some(left), Some(right)) = (left.unit, right.unit) {
        if left != right {
            push(
                diagnostics,
                "UNIT_MISMATCH",
                "expression operand units differ",
            );
        }
    }
}

fn reference_type(
    source: &ScenarioSource,
    reference: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ExprType> {
    let Some((namespace, id)) = reference.split_once('.') else {
        push(
            diagnostics,
            "UNKNOWN_REF",
            "reference must be namespace-qualified",
        );
        return None;
    };
    match namespace {
        "telemetry" => source.telemetry.get(id).map(|channel| ExprType {
            scalar: channel.scalar_type.clone(),
            unit: Some(channel.unit),
        }),
        "feedback" => source.feedback.get(id).map(|channel| ExprType {
            scalar: channel.scalar_type.clone(),
            unit: Some(channel.unit),
        }),
        "actuator" => source.actuators.get(id).map(|actuator| ExprType {
            scalar: actuator.scalar_type.clone(),
            unit: Some(actuator.unit),
        }),
        _ => {
            push(diagnostics, "REF_KIND", "unknown reference namespace");
            return None;
        }
    }
    .or_else(|| {
        push(
            diagnostics,
            "UNKNOWN_REF",
            "referenced object does not exist",
        );
        None
    })
}

fn check_reference(
    source: &ScenarioSource,
    reference: &str,
    expected_namespace: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some((namespace, _)) = reference.split_once('.') {
        if expected_namespace.is_some_and(|expected| namespace != expected) {
            push(diagnostics, "REF_KIND", "reference has the wrong namespace");
        }
    }
    let _ = reference_type(source, reference, diagnostics);
}

fn validate_phases(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    if !source.phases.items.contains_key(&source.phases.initial) {
        push(diagnostics, "UNKNOWN_REF", "initial phase does not exist");
    }
    let mut transition_ids = BTreeSet::new();
    let mut transition_count = 0;
    let mut graph: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, phase) in &source.phases.items {
        validate_id(id, diagnostics);
        require_bool(&phase.entry, source, diagnostics);
        require_bool(&phase.completion, source, diagnostics);
        transition_count += phase.transitions.len();
        if phase.timeout_ticks == 0 && !phase.terminal {
            push(
                diagnostics,
                "PHASE_DEAD_END",
                "nonterminal phase requires a timeout",
            );
        }
        if phase.abort_terminal
            && phase
                .transitions
                .iter()
                .any(|transition| transition.trigger == TransitionTrigger::Automatic)
        {
            push(
                diagnostics,
                "PHASE_DEAD_END",
                "abort terminal has an automatic transition",
            );
        }
        if phase.abort_terminal && !phase.terminal {
            push(
                diagnostics,
                "PHASE_DEAD_END",
                "abort_terminal phase must also be terminal",
            );
        }
        let mut constant_true = 0;
        for transition in &phase.transitions {
            validate_id(&transition.id, diagnostics);
            if !transition_ids.insert(transition.id.as_str()) {
                push(
                    diagnostics,
                    "DUPLICATE_KEY",
                    "transition ID is not globally unique",
                );
            }
            if !source.phases.items.contains_key(&transition.to) {
                push(
                    diagnostics,
                    "UNKNOWN_REF",
                    "transition destination does not exist",
                );
            }
            require_bool(&transition.guard, source, diagnostics);
            if transition.trigger == TransitionTrigger::Automatic {
                graph.entry(id).or_default().push(&transition.to);
                if expression_is_true(&transition.guard) {
                    constant_true += 1;
                }
            }
            if transition.new_attempt
                && !(transition.trigger == TransitionTrigger::Supervisor
                    && phase.terminal
                    && transition.to == source.phases.initial)
            {
                push(
                    diagnostics,
                    "PHASE_DEAD_END",
                    "new_attempt transition is invalid",
                );
            }
        }
        if constant_true > 1 {
            push(
                diagnostics,
                "TRANSITION_AMBIGUOUS",
                "multiple automatic guards are statically true",
            );
        }
        if let TimeoutAction::Transition { transition } = &phase.on_timeout {
            if !phase.transitions.iter().any(|candidate| {
                candidate.id == *transition && candidate.trigger == TransitionTrigger::Supervisor
            }) {
                push(
                    diagnostics,
                    "UNKNOWN_REF",
                    "timeout names no supervisor transition in its phase",
                );
            }
        }
    }
    validate_count(transition_count, 1024, "transitions", diagnostics);
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for phase in source.phases.items.keys() {
        if has_cycle(phase, &graph, &mut visiting, &mut visited) {
            push(
                diagnostics,
                "PHASE_CYCLE",
                "automatic transition graph contains a cycle",
            );
            break;
        }
    }
}

fn has_cycle<'a>(
    phase: &'a str,
    graph: &BTreeMap<&'a str, Vec<&'a str>>,
    visiting: &mut BTreeSet<&'a str>,
    visited: &mut BTreeSet<&'a str>,
) -> bool {
    if visiting.contains(phase) {
        return true;
    }
    if !visited.insert(phase) {
        return false;
    }
    visiting.insert(phase);
    let cycle = graph.get(phase).is_some_and(|targets| {
        targets
            .iter()
            .any(|target| has_cycle(target, graph, visiting, visited))
    });
    visiting.remove(phase);
    cycle
}

fn expression_is_true(expression: &Expression) -> bool {
    matches!(expression, Expression::Const(TypedConstant { scalar_type, value: ScalarValue::Bool(true) }) if scalar_type == "bool")
}

fn validate_rules(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    for (id, rule) in &source.rules {
        validate_id(id, diagnostics);
        require_bool(&rule.condition, source, diagnostics);
        if rule.message.chars().count() > 512 {
            push(
                diagnostics,
                "LIMIT_EXCEEDED",
                "rule message exceeds 512 characters",
            );
        }
        match &rule.scope {
            RuleScope::Global(_) => {}
            RuleScope::Phases(phases) => {
                if phases.is_empty() {
                    push(diagnostics, "RULE_COVERAGE", "phase rule scope is empty");
                }
                for phase in phases {
                    if !source.phases.items.contains_key(phase) {
                        push(diagnostics, "UNKNOWN_REF", "rule scope names unknown phase");
                    }
                }
            }
        }
        let response_ok = matches!(
            (&rule.severity, &rule.response),
            (Severity::Advisory, RuleResponse::Observe(true))
                | (Severity::Inhibit, RuleResponse::Inhibit { .. })
                | (Severity::Hold, RuleResponse::Hold { .. })
                | (Severity::Hold, RuleResponse::Force { .. })
                | (Severity::Hold, RuleResponse::Review { .. })
                | (Severity::Abort, RuleResponse::Abort { .. })
                | (Severity::Abort, RuleResponse::Force { .. })
        );
        if !response_ok {
            push(
                diagnostics,
                "RULE_RESPONSE",
                "severity and response are incompatible",
            );
        }
        match &rule.response {
            RuleResponse::Hold { profile }
            | RuleResponse::Abort { profile }
            | RuleResponse::Force { profile } => {
                if !source.safe_states.profiles.contains_key(profile) {
                    push(
                        diagnostics,
                        "UNKNOWN_REF",
                        "rule names unknown safe-state profile",
                    );
                }
            }
            RuleResponse::Review { transition } => {
                if !source
                    .phases
                    .items
                    .values()
                    .flat_map(|phase| &phase.transitions)
                    .any(|candidate| &candidate.id == transition)
                {
                    push(
                        diagnostics,
                        "UNKNOWN_REF",
                        "review names unknown transition",
                    );
                }
            }
            RuleResponse::Inhibit { targets } => {
                for target in targets {
                    let actuator = target
                        .strip_prefix("actuator.")
                        .is_some_and(|id| source.actuators.contains_key(id));
                    let transition = source
                        .phases
                        .items
                        .values()
                        .flat_map(|phase| &phase.transitions)
                        .any(|candidate| candidate.id == *target);
                    if !actuator && !transition {
                        push(diagnostics, "UNKNOWN_REF", "inhibit target does not exist");
                    }
                }
            }
            RuleResponse::Observe(_) => {}
        }
    }
}

#[derive(Default)]
struct ExpressionCoverage<'a> {
    references: BTreeSet<&'a str>,
    fresh: BTreeSet<&'a str>,
    phases: BTreeSet<&'a str>,
}

fn validate_rule_coverage(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    let mut coverage = ExpressionCoverage::default();
    for rule in source.rules.values() {
        collect_coverage(&rule.condition, &mut coverage);
    }
    for (id, channel) in &source.telemetry {
        let reference = format!("telemetry.{id}");
        if !coverage.fresh.contains(reference.as_str()) {
            push(
                diagnostics,
                "RULE_COVERAGE",
                "telemetry channel has no explicit freshness rule",
            );
        }
        if !channel.bands.is_empty() && !coverage.references.contains(reference.as_str()) {
            push(
                diagnostics,
                "RULE_COVERAGE",
                "banded telemetry channel is absent from all rules",
            );
        }
    }
    for id in source.feedback.keys() {
        let reference = format!("feedback.{id}");
        if !coverage.fresh.contains(reference.as_str()) {
            push(
                diagnostics,
                "RULE_COVERAGE",
                "feedback channel has no explicit freshness rule",
            );
        }
    }
    for (id, actuator) in &source.actuators {
        if let Some(feedback) = &actuator.feedback {
            let actuator_reference = format!("actuator.{id}");
            if !coverage.references.contains(actuator_reference.as_str())
                || !coverage.references.contains(feedback.as_str())
            {
                push(
                    diagnostics,
                    "RULE_COVERAGE",
                    "feedback-linked actuator has no rule comparing command and feedback",
                );
            }
        }
        if !actuator.mutually_exclusive.is_empty() {
            let reference = format!("actuator.{id}");
            if !coverage.references.contains(reference.as_str()) {
                push(
                    diagnostics,
                    "RULE_COVERAGE",
                    "mutually exclusive actuator is absent from all rules",
                );
            }
        }
    }
    for (id, phase) in &source.phases.items {
        if phase.armed && !coverage.phases.contains(id.as_str()) {
            push(
                diagnostics,
                "RULE_COVERAGE",
                "armed phase has no explicit phase-scoped update rule",
            );
        }
    }
    if source
        .phases
        .items
        .values()
        .any(|phase| phase.abort_terminal)
        && !source
            .rules
            .values()
            .any(|rule| rule.severity == Severity::Abort)
    {
        push(
            diagnostics,
            "RULE_COVERAGE",
            "abort-terminal scenario has no abort rule",
        );
    }
}

fn collect_coverage<'a>(expression: &'a Expression, coverage: &mut ExpressionCoverage<'a>) {
    match expression {
        Expression::Ref(reference) => {
            coverage.references.insert(reference);
        }
        Expression::Fresh(fresh) => {
            coverage.references.insert(&fresh.r#ref);
            coverage.fresh.insert(&fresh.r#ref);
        }
        Expression::PhaseIs(phase) => {
            coverage.phases.insert(phase);
        }
        Expression::Not(inner) => collect_coverage(inner, coverage),
        Expression::All(items) | Expression::Any(items) => {
            for item in items {
                collect_coverage(item, coverage);
            }
        }
        Expression::Compare(compare) => {
            collect_coverage(&compare.left, coverage);
            collect_coverage(&compare.right, coverage);
        }
        Expression::Between(between) => {
            collect_coverage(&between.value, coverage);
            collect_coverage(&between.min, coverage);
            collect_coverage(&between.max, coverage);
        }
        Expression::Const(_) | Expression::FaultActive(_) => {}
    }
}

fn validate_safe_states(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    for actuator in source.actuators.keys() {
        let reference = format!("actuator.{actuator}");
        if !source.safe_states.defaults.contains_key(&reference) {
            push(
                diagnostics,
                "SAFE_COVERAGE",
                "actuator is missing a default safe action",
            );
        }
    }
    for (reference, action) in &source.safe_states.defaults {
        validate_action(source, reference, action, false, diagnostics);
    }
    let mut global = 0;
    let mut abort = 0;
    let mut phase_profiles = BTreeSet::new();
    let mut hazard_priorities = BTreeSet::new();
    for (id, profile) in &source.safe_states.profiles {
        validate_id(id, diagnostics);
        match profile.layer {
            ProfileLayer::Global => {
                global += 1;
                if profile.phase.is_some() || profile.priority.is_some() || profile.when.is_some() {
                    push(
                        diagnostics,
                        "SAFE_CONFLICT",
                        "global profile has fields for another layer",
                    );
                }
            }
            ProfileLayer::AbortLatch => {
                abort += 1;
                if profile.phase.is_some() || profile.priority.is_some() || profile.when.is_some() {
                    push(
                        diagnostics,
                        "SAFE_CONFLICT",
                        "abort-latch profile has fields for another layer",
                    );
                }
            }
            ProfileLayer::Hazard => {
                let Some(priority) = profile.priority else {
                    push(
                        diagnostics,
                        "SAFE_CONFLICT",
                        "hazard profile requires priority",
                    );
                    continue;
                };
                if !hazard_priorities.insert(priority) {
                    push(
                        diagnostics,
                        "SAFE_CONFLICT",
                        "hazard priority is duplicated",
                    );
                }
                let Some(condition) = &profile.when else {
                    push(
                        diagnostics,
                        "SAFE_COVERAGE",
                        "hazard profile requires a condition",
                    );
                    continue;
                };
                require_bool(condition, source, diagnostics);
                if profile.phase.is_some() {
                    push(
                        diagnostics,
                        "SAFE_CONFLICT",
                        "hazard profile cannot name a phase",
                    );
                }
            }
            ProfileLayer::Phase => {
                let Some(phase) = &profile.phase else {
                    push(diagnostics, "SAFE_COVERAGE", "phase profile requires phase");
                    continue;
                };
                if !source.phases.items.contains_key(phase) {
                    push(
                        diagnostics,
                        "UNKNOWN_REF",
                        "safe profile phase does not exist",
                    );
                }
                if !phase_profiles.insert(phase) {
                    push(
                        diagnostics,
                        "SAFE_CONFLICT",
                        "phase has multiple safe profiles",
                    );
                }
                if profile.priority.is_some() || profile.when.is_some() {
                    push(
                        diagnostics,
                        "SAFE_CONFLICT",
                        "phase profile has fields for another layer",
                    );
                }
            }
        }
        for (reference, action) in &profile.actions {
            validate_action(source, reference, action, true, diagnostics);
        }
    }
    if global > 1 || abort > 1 {
        push(
            diagnostics,
            "SAFE_CONFLICT",
            "global or abort-latch profile is duplicated",
        );
    }
}

fn validate_action(
    source: &ScenarioSource,
    reference: &str,
    action: &Action,
    layered: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(id) = reference.strip_prefix("actuator.") else {
        push(
            diagnostics,
            "REF_KIND",
            "safe action must reference an actuator",
        );
        return;
    };
    let Some(actuator) = source.actuators.get(id) else {
        push(
            diagnostics,
            "UNKNOWN_REF",
            "safe action actuator does not exist",
        );
        return;
    };
    match action {
        Action::Set(value) => validate_value(
            &actuator.scalar_type,
            actuator.range.as_ref(),
            value,
            source,
            diagnostics,
        ),
        Action::Deenergize(value) => {
            if !*value || actuator.deenergized.is_none() {
                push(
                    diagnostics,
                    "SAFE_COVERAGE",
                    "deenergize requires a declared value",
                );
            }
        }
        Action::PreserveCurrent(preserve) => {
            if !layered
                || !actuator.allow_preserve_current
                || !(1..=1000).contains(&preserve.max_ticks)
                || preserve.justification.trim().is_empty()
            {
                push(
                    diagnostics,
                    "PRESERVE_UNSAFE",
                    "preserve_current requirements are not met",
                );
            }
        }
    }
}

fn validate_dynamics(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    let mut writers = BTreeSet::new();
    let mut delayed_edges: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, rule) in &source.dynamics {
        validate_id(id, diagnostics);
        if !writers.insert(rule.target.as_str()) {
            push(
                diagnostics,
                "DYNAMIC_WRITER",
                "multiple dynamics write one target",
            );
        }
        let Some(target_type) = reference_type(source, &rule.target, diagnostics) else {
            continue;
        };
        let namespace = rule.target.split_once('.').map(|pair| pair.0);
        if !matches!(namespace, Some("telemetry" | "feedback")) {
            push(
                diagnostics,
                "REF_KIND",
                "dynamic target must be telemetry or feedback",
            );
        }
        if let Some(condition) = &rule.when {
            require_bool(condition, source, diagnostics);
        }
        match &rule.operation {
            DynamicOperation::Set(value) => {
                validate_dynamic_value(source, &rule.target, value, diagnostics)
            }
            DynamicOperation::Copy(reference) => {
                if let Some(source_type) = reference_type(source, reference, diagnostics) {
                    compatible_types(&target_type, &source_type, diagnostics);
                }
            }
            DynamicOperation::AddClamped(_) => {
                if !matches!(target_type.scalar.as_str(), "i32" | "u32") {
                    push(
                        diagnostics,
                        "TYPE_MISMATCH",
                        "add_clamped target must be numeric",
                    );
                }
            }
            DynamicOperation::Approach(approach) => {
                if approach.step == 0 || approach.reference.is_some() == approach.value.is_some() {
                    push(
                        diagnostics,
                        "DYNAMIC_OVERFLOW",
                        "approach requires one source and positive step",
                    );
                }
                if let Some(reference) = &approach.reference {
                    if let Some(source_type) = reference_type(source, reference, diagnostics) {
                        compatible_types(&target_type, &source_type, diagnostics);
                    }
                }
                if let Some(value) = &approach.value {
                    validate_dynamic_value(source, &rule.target, value, diagnostics);
                }
            }
            DynamicOperation::MapEnum(mapping) => {
                let Some(source_type) = reference_type(source, &mapping.source, diagnostics) else {
                    continue;
                };
                let Some(definition) = source.types.get(&source_type.scalar) else {
                    push(diagnostics, "TYPE_MISMATCH", "map_enum source must be enum");
                    continue;
                };
                if mapping.values.keys().collect::<BTreeSet<_>>()
                    != definition.variants.iter().collect::<BTreeSet<_>>()
                {
                    push(diagnostics, "EXPR_NOT_TOTAL", "map_enum is not total");
                }
                for value in mapping.values.values() {
                    validate_dynamic_value(source, &rule.target, value, diagnostics);
                }
            }
            DynamicOperation::ConfirmAfter(confirm) => {
                check_reference(source, &confirm.actuator, Some("actuator"), diagnostics);
                if let Some(source_type) = reference_type(source, &confirm.actuator, diagnostics) {
                    compatible_types(&target_type, &source_type, diagnostics);
                }
                if confirm.delay_ticks == 0 {
                    push(
                        diagnostics,
                        "VALUE_RANGE",
                        "confirm_after delay must be positive",
                    );
                }
                delayed_edges
                    .entry(rule.target.as_str())
                    .or_default()
                    .push(confirm.actuator.as_str());
            }
        }
    }
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for target in delayed_edges.keys() {
        if has_cycle(target, &delayed_edges, &mut visiting, &mut visited) {
            push(
                diagnostics,
                "DYNAMIC_CYCLE",
                "delayed dynamics contain a cycle",
            );
            break;
        }
    }
}

fn validate_dynamic_value(
    source: &ScenarioSource,
    target: &str,
    value: &ScalarValue,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some((namespace, id)) = target.split_once('.') {
        match namespace {
            "telemetry" => {
                if let Some(channel) = source.telemetry.get(id) {
                    validate_value(
                        &channel.scalar_type,
                        channel.range.as_ref(),
                        value,
                        source,
                        diagnostics,
                    );
                }
            }
            "feedback" => {
                if let Some(channel) = source.feedback.get(id) {
                    validate_value(
                        &channel.scalar_type,
                        channel.range.as_ref(),
                        value,
                        source,
                        diagnostics,
                    );
                }
            }
            _ => {}
        }
    }
}

fn validate_faults(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    let mut priorities = BTreeSet::new();
    for (id, fault) in &source.faults {
        validate_id(id, diagnostics);
        if !priorities.insert(fault.priority) {
            push(
                diagnostics,
                "FAULT_CONFLICT",
                "fault priority is duplicated",
            );
        }
        if let FaultActivation::Window { window } = &fault.activation {
            if window.start_tick > window.end_tick {
                push(diagnostics, "VALUE_RANGE", "fault tick window is reversed");
            }
        }
        let mut targets = BTreeSet::new();
        for effect in &fault.effects {
            let (target, value) = match effect {
                FaultEffect::Override { target, value } => (target, Some(value)),
                FaultEffect::BiasClamped { target, .. }
                | FaultEffect::Freeze { target }
                | FaultEffect::DropUpdates { target }
                | FaultEffect::DelayFeedback { target, .. } => (target, None),
            };
            if !targets.insert(target) {
                push(
                    diagnostics,
                    "FAULT_CONFLICT",
                    "fault has conflicting effects on one target",
                );
            }
            let namespace = target.split_once('.').map(|pair| pair.0);
            if !matches!(namespace, Some("telemetry" | "feedback")) {
                push(
                    diagnostics,
                    "REF_KIND",
                    "fault target must be telemetry or feedback",
                );
            }
            check_reference(source, target, None, diagnostics);
            if let Some(value) = value {
                validate_dynamic_value(source, target, value, diagnostics);
            }
            if let FaultEffect::DelayFeedback { target, ticks } = effect {
                if !target.starts_with("feedback.") {
                    push(
                        diagnostics,
                        "REF_KIND",
                        "delay_feedback requires feedback target",
                    );
                }
                if *ticks == 0 {
                    push(
                        diagnostics,
                        "VALUE_RANGE",
                        "delay_feedback ticks must be positive",
                    );
                }
            }
        }
    }
}

fn validate_layout(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    let Some(layout) = &source.layout else {
        return;
    };
    validate_count(layout.panels.len(), 64, "layout panels", diagnostics);
    validate_count(layout.nodes.len(), 1024, "layout nodes", diagnostics);
    let mut placed_nodes = BTreeSet::new();
    for (id, panel) in &layout.panels {
        validate_id(id, diagnostics);
        if panel.title.chars().count() > 128 {
            push(
                diagnostics,
                "LIMIT_EXCEEDED",
                "panel title exceeds 128 characters",
            );
        }
        for node in &panel.nodes {
            if !layout.nodes.contains_key(node) {
                push(
                    diagnostics,
                    "UNKNOWN_REF",
                    "panel names unknown layout node",
                );
            }
            if !placed_nodes.insert(node) {
                push(
                    diagnostics,
                    "DUPLICATE_KEY",
                    "layout node is placed in multiple panels",
                );
            }
        }
    }
    for (id, node) in &layout.nodes {
        validate_id(id, diagnostics);
        if node.x > 4095
            || node.y > 4095
            || node.width == 0
            || node.width > 4095
            || node.height == 0
            || node.height > 4095
        {
            push(
                diagnostics,
                "VALUE_RANGE",
                "layout coordinates or size are invalid",
            );
        }
        if node.reference.starts_with("rule.") {
            if !source
                .rules
                .contains_key(node.reference.trim_start_matches("rule."))
            {
                push(diagnostics, "UNKNOWN_REF", "layout rule does not exist");
            }
        } else {
            check_reference(source, &node.reference, None, diagnostics);
        }
    }
}

fn allocate(
    source: &ScenarioSource,
    mode: AllocationMode<'_>,
) -> Result<Vec<AllocationEntry>, CompileError> {
    let required = required_allocations(source);
    match mode {
        AllocationMode::Clean => allocate_clean(required),
        AllocationMode::Stable {
            prior,
            expected_bundle_hash,
        } => {
            if prior.bundle_hash != expected_bundle_hash {
                return Err(one(
                    "HASH_MISMATCH",
                    "prior allocation bundle hash does not match",
                ));
            }
            if prior.scenario_id != source.id {
                return Err(one(
                    "STABLE_INCOMPAT",
                    "prior allocation belongs to another scenario",
                ));
            }
            if source.publication <= prior.publication {
                return Err(one(
                    "PUBLICATION_ORDER",
                    "stable allocation requires a newer publication",
                ));
            }
            allocate_stable(required, prior)
        }
    }
}

fn required_allocations(source: &ScenarioSource) -> Vec<AllocationEntry> {
    let mut entries = Vec::new();
    for (id, channel) in &source.telemetry {
        entries.push(unaddressed(
            format!("telemetry.{id}"),
            AllocationKind::Telemetry,
            &channel.scalar_type,
            channel.unit,
        ));
    }
    for (id, actuator) in &source.actuators {
        if actuator.privilege == Privilege::Ordinary {
            entries.push(unaddressed(
                format!("actuator.{id}"),
                AllocationKind::ActuatorRequest,
                &actuator.scalar_type,
                actuator.unit,
            ));
        }
    }
    for (id, channel) in &source.feedback {
        entries.push(unaddressed(
            format!("feedback.{id}"),
            AllocationKind::Feedback,
            &channel.scalar_type,
            channel.unit,
        ));
    }
    entries.sort_by(|left, right| {
        left.qualified_id
            .as_bytes()
            .cmp(right.qualified_id.as_bytes())
    });
    entries
}

fn unaddressed(id: String, kind: AllocationKind, scalar_type: &str, unit: Unit) -> AllocationEntry {
    AllocationEntry {
        qualified_id: id,
        address: 0,
        kind,
        scalar_type: scalar_type.to_owned(),
        unit,
    }
}

fn allocate_clean(mut entries: Vec<AllocationEntry>) -> Result<Vec<AllocationEntry>, CompileError> {
    let mut next = BTreeMap::from([
        ("telemetry", 0x4000_0000_u32),
        ("actuator", 0x5000_0000_u32),
        ("feedback", 0x6000_0000_u32),
        ("supervisor", 0x7000_0000_u32),
    ]);
    for entry in &mut entries {
        let namespace = entry.qualified_id.split_once('.').map_or("", |pair| pair.0);
        let address = next
            .get_mut(namespace)
            .ok_or_else(|| one("MMIO_EXHAUSTED", "unknown MMIO namespace"))?;
        entry.address = *address;
        *address = address
            .checked_add(4)
            .ok_or_else(|| one("MMIO_EXHAUSTED", "MMIO region exhausted"))?;
    }
    Ok(entries)
}

fn allocate_stable(
    mut required: Vec<AllocationEntry>,
    prior: &PriorAllocation,
) -> Result<Vec<AllocationEntry>, CompileError> {
    let mut prior_by_id = BTreeMap::new();
    let mut used = BTreeSet::new();
    for entry in &prior.entries {
        validate_allocation_entry(entry)?;
        let namespace = entry.qualified_id.split_once('.').map_or("", |pair| pair.0);
        let expected_namespace = match entry.kind {
            AllocationKind::Telemetry => "telemetry",
            AllocationKind::ActuatorRequest => "actuator",
            AllocationKind::Feedback => "feedback",
            AllocationKind::Supervisor => "supervisor",
        };
        if namespace != expected_namespace {
            return Err(one(
                "STABLE_INCOMPAT",
                "prior allocation ID and region disagree",
            ));
        }
        if prior_by_id
            .insert(entry.qualified_id.clone(), entry)
            .is_some()
            || !used.insert(entry.address)
        {
            return Err(one(
                "MMIO_COLLISION",
                "prior allocation has duplicate ID or address",
            ));
        }
    }
    for entry in &mut required {
        if let Some(previous) = prior_by_id.get(&entry.qualified_id) {
            if previous.kind != entry.kind
                || previous.scalar_type != entry.scalar_type
                || previous.unit != entry.unit
            {
                return Err(one(
                    "STABLE_INCOMPAT",
                    "stable allocation type, unit, or region changed",
                ));
            }
            entry.address = previous.address;
        } else {
            let (start, end) = region_bounds(&entry.kind);
            let mut candidate = start;
            while used.contains(&candidate) {
                candidate = candidate
                    .checked_add(4)
                    .ok_or_else(|| one("MMIO_EXHAUSTED", "MMIO region exhausted"))?;
            }
            if candidate > end {
                return Err(one("MMIO_EXHAUSTED", "MMIO region exhausted"));
            }
            entry.address = candidate;
            used.insert(candidate);
        }
    }
    required.sort_by(|left, right| left.qualified_id.cmp(&right.qualified_id));
    Ok(required)
}

fn validate_allocation_entry(entry: &AllocationEntry) -> Result<(), CompileError> {
    let (start, end) = region_bounds(&entry.kind);
    if entry.address % 4 != 0 {
        return Err(one("MMIO_ALIGNMENT", "prior allocation is unaligned"));
    }
    if !(start..=end).contains(&entry.address) {
        return Err(one(
            "MMIO_EXHAUSTED",
            "prior allocation is outside its region",
        ));
    }
    Ok(())
}

fn region_bounds(kind: &AllocationKind) -> (u32, u32) {
    match kind {
        AllocationKind::Telemetry => (0x4000_0000, 0x400F_FFFC),
        AllocationKind::ActuatorRequest => (0x5000_0000, 0x500F_FFFC),
        AllocationKind::Feedback => (0x6000_0000, 0x600F_FFFC),
        AllocationKind::Supervisor => (0x7000_0000, 0x7000_FFFC),
    }
}

fn generate_symbols(
    scenario_id: &str,
    bundle_hash: &str,
    entries: &[AllocationEntry],
) -> Result<String, CompileError> {
    let mut names = BTreeSet::new();
    let mut lines = vec![
        format!("# schema: {SCHEMA_ID}"),
        format!("# scenario: {scenario_id}"),
        format!("# bundle-sha256: {bundle_hash}"),
    ];
    for entry in entries {
        let name = format!(
            "S32_{}",
            entry
                .qualified_id
                .chars()
                .map(|character| if character.is_ascii_alphanumeric() {
                    character.to_ascii_uppercase()
                } else {
                    '_'
                })
                .collect::<String>()
        );
        if !names.insert(name.clone()) {
            return Err(one("MMIO_COLLISION", "generated symbol names collide"));
        }
        lines.push(format!(".equ {name}, 0x{:08X}", entry.address));
    }
    Ok(format!("{}\n", lines.join("\n")))
}

fn normalized_json<T: Serialize>(value: &T) -> Result<JsonValue, serde_json::Error> {
    serde_json::to_value(value).map(normalize_json_value)
}

fn normalize_json_value(value: JsonValue) -> JsonValue {
    match value {
        JsonValue::String(value) => JsonValue::String(value.nfc().collect()),
        JsonValue::Array(values) => {
            JsonValue::Array(values.into_iter().map(normalize_json_value).collect())
        }
        JsonValue::Object(values) => {
            let sorted = values
                .into_iter()
                .map(|(key, value)| (key.nfc().collect(), normalize_json_value(value)))
                .collect();
            JsonValue::Object(sorted)
        }
        other => other,
    }
}

fn canonical_json(value: &JsonValue) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_default()
}

fn hash(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn validate_id(id: &str, diagnostics: &mut Vec<Diagnostic>) {
    let valid = !id.is_empty()
        && id.len() <= 64
        && id.as_bytes()[0].is_ascii_lowercase()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
    if !valid {
        push(
            diagnostics,
            "INVALID_ID",
            &format!("invalid identifier `{id}`"),
        );
    }
}

fn validate_count(count: usize, maximum: usize, name: &str, diagnostics: &mut Vec<Diagnostic>) {
    if count > maximum {
        push(
            diagnostics,
            "LIMIT_EXCEEDED",
            &format!("{name} exceed limit {maximum}"),
        );
    }
}

fn internal_serialization(error: serde_json::Error) -> CompileError {
    one(
        "TYPE_MISMATCH",
        &format!("canonical serialization failed: {error}"),
    )
}

fn one(code: &'static str, message: &str) -> CompileError {
    CompileError {
        diagnostics: vec![Diagnostic {
            code,
            line: 1,
            column: 1,
            message: message.to_owned(),
        }],
    }
}

fn push(diagnostics: &mut Vec<Diagnostic>, code: &'static str, message: &str) {
    if diagnostics.len() < 256 {
        diagnostics.push(Diagnostic {
            code,
            line: 1,
            column: 1,
            message: message.to_owned(),
        });
    }
}
