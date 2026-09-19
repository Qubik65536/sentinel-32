use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

fn default_false() -> bool {
    false
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioSource {
    pub schema: String,
    pub id: String,
    pub publication: u32,
    pub tick_ms: u32,
    pub metadata: Metadata,
    #[serde(default)]
    pub types: BTreeMap<String, EnumDefinition>,
    pub telemetry: BTreeMap<String, Channel>,
    pub actuators: BTreeMap<String, Actuator>,
    pub feedback: BTreeMap<String, FeedbackChannel>,
    pub phases: Phases,
    pub rules: BTreeMap<String, Rule>,
    pub safe_states: SafeStates,
    pub dynamics: BTreeMap<String, DynamicRule>,
    #[serde(default)]
    pub faults: BTreeMap<String, Fault>,
    #[serde(default)]
    pub layout: Option<Layout>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnumDefinition {
    pub variants: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ScalarValue {
    Bool(bool),
    Signed(i32),
    Unsigned(u32),
    Enum(String),
}

impl PartialEq for ScalarValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(left), Self::Bool(right)) => left == right,
            (Self::Signed(left), Self::Signed(right)) => left == right,
            (Self::Unsigned(left), Self::Unsigned(right)) => left == right,
            (Self::Enum(left), Self::Enum(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for ScalarValue {}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    None,
    Count,
    Tick,
    Millisecond,
    PressureMilli,
    PercentMilli,
    Millivolt,
    Milliampere,
    TemperatureMilliC,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NumericRange {
    pub min: i64,
    pub max: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Band {
    pub min: i64,
    pub max: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    #[serde(rename = "type")]
    pub scalar_type: String,
    pub unit: Unit,
    #[serde(default)]
    pub range: Option<NumericRange>,
    pub initial: ScalarValue,
    pub stale_after_ticks: u32,
    #[serde(default)]
    pub bands: BTreeMap<String, Band>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackChannel {
    #[serde(rename = "type")]
    pub scalar_type: String,
    pub unit: Unit,
    #[serde(default)]
    pub range: Option<NumericRange>,
    pub initial: ScalarValue,
    pub stale_after_ticks: u32,
    #[serde(default)]
    pub actuator: Option<String>,
    #[serde(default)]
    pub confirms: Option<Expression>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Privilege {
    Ordinary,
    Supervisor,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reversibility {
    Reversible,
    AttemptLatched,
    Irreversible,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum CommandBehavior {
    Set(CommandSet),
    Pulse { pulse: PulseCommand },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum CommandSet {
    #[serde(rename = "set")]
    Set,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PulseCommand {
    pub duration_ticks: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Actuator {
    #[serde(rename = "type")]
    pub scalar_type: String,
    pub unit: Unit,
    #[serde(default)]
    pub range: Option<NumericRange>,
    pub initial: ScalarValue,
    pub safe_default: ScalarValue,
    #[serde(default)]
    pub deenergized: Option<ScalarValue>,
    pub privilege: Privilege,
    pub reversibility: Reversibility,
    #[serde(default)]
    pub feedback: Option<String>,
    #[serde(default)]
    pub feedback_timeout_ticks: Option<u32>,
    #[serde(default)]
    pub mutually_exclusive: Vec<Expression>,
    #[serde(default = "default_false")]
    pub allow_preserve_current: bool,
    pub command: CommandBehavior,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypedConstant {
    #[serde(rename = "type")]
    pub scalar_type: String,
    pub value: ScalarValue,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Comparison {
    pub op: CompareOp,
    pub left: Box<Expression>,
    pub right: Box<Expression>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Between {
    pub value: Box<Expression>,
    pub min: Box<Expression>,
    pub max: Box<Expression>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Fresh {
    pub r#ref: String,
    pub max_age_ticks: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Expression {
    Const(TypedConstant),
    Ref(String),
    Not(Box<Expression>),
    All(Vec<Expression>),
    Any(Vec<Expression>),
    Compare(Comparison),
    Between(Between),
    Fresh(Fresh),
    PhaseIs(String),
    FaultActive(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Phases {
    pub initial: String,
    pub items: BTreeMap<String, Phase>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Phase {
    pub entry: Expression,
    pub completion: Expression,
    pub timeout_ticks: u32,
    pub on_timeout: TimeoutAction,
    #[serde(default)]
    pub transitions: Vec<Transition>,
    #[serde(default = "default_false")]
    pub armed: bool,
    #[serde(default = "default_false")]
    pub terminal: bool,
    #[serde(default = "default_false")]
    pub abort_terminal: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TimeoutAction {
    Builtin(TimeoutBuiltin),
    Transition { transition: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeoutBuiltin {
    Hold,
    Abort,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionTrigger {
    Automatic,
    Supervisor,
    Hold,
    Abort,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub id: String,
    pub to: String,
    pub guard: Expression,
    pub trigger: TransitionTrigger,
    #[serde(default = "default_false")]
    pub new_attempt: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RuleScope {
    Global(RuleGlobal),
    Phases(Vec<String>),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuleGlobal {
    #[serde(rename = "global")]
    Global,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Advisory,
    Inhibit,
    Hold,
    Abort,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleResponse {
    Observe(bool),
    Inhibit { targets: Vec<String> },
    Hold { profile: String },
    Abort { profile: String },
    Force { profile: String },
    Review { transition: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub condition: Expression,
    pub scope: RuleScope,
    pub severity: Severity,
    pub response: RuleResponse,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Set(ScalarValue),
    Deenergize(bool),
    PreserveCurrent(PreserveCurrent),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreserveCurrent {
    pub max_ticks: u32,
    pub justification: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileLayer {
    Global,
    AbortLatch,
    Hazard,
    Phase,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SafeProfile {
    pub layer: ProfileLayer,
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub priority: Option<u8>,
    #[serde(default)]
    pub when: Option<Expression>,
    pub actions: BTreeMap<String, Action>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SafeStates {
    pub defaults: BTreeMap<String, Action>,
    pub profiles: BTreeMap<String, SafeProfile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicRule {
    #[serde(default)]
    pub when: Option<Expression>,
    pub target: String,
    #[serde(flatten)]
    pub operation: DynamicOperation,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicOperation {
    Set(ScalarValue),
    Copy(String),
    AddClamped(i32),
    Approach(Approach),
    MapEnum(MapEnum),
    ConfirmAfter(ConfirmAfter),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Approach {
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub value: Option<ScalarValue>,
    pub step: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MapEnum {
    pub source: String,
    pub values: BTreeMap<String, ScalarValue>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmAfter {
    pub actuator: String,
    pub delay_ticks: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FaultActivation {
    Mode(FaultMode),
    Window { window: TickWindow },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultMode {
    Fixture,
    Supervisor,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TickWindow {
    pub start_tick: u64,
    pub end_tick: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultEffect {
    Override { target: String, value: ScalarValue },
    BiasClamped { target: String, amount: i32 },
    Freeze { target: String },
    DropUpdates { target: String },
    DelayFeedback { target: String, ticks: u32 },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Fault {
    pub activation: FaultActivation,
    #[serde(default = "default_false")]
    pub one_shot: bool,
    pub priority: u8,
    pub effects: Vec<FaultEffect>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    #[serde(default)]
    pub panels: BTreeMap<String, Panel>,
    #[serde(default)]
    pub nodes: BTreeMap<String, LayoutNode>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Panel {
    pub title: String,
    pub nodes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayStyle {
    Value,
    Gauge,
    State,
    Command,
    Alarm,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutNode {
    pub reference: String,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    #[serde(default)]
    pub style: Option<DisplayStyle>,
}
