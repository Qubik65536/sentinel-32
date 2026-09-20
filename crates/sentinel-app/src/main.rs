use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::ExitCode;
use std::sync::OnceLock;
use std::time::Duration;

use sentinel_ai_check::{
    AdvisoryProvider, CheckRequest, DeploymentProfile, FixtureProvider, Limits, LlamaCppConfig,
    LlamaCppProvider, parse_and_validate_rule_set, parse_and_validate_snapshot,
};
#[cfg(feature = "openai-test")]
use sentinel_ai_check::{OpenAiTestConfig, OpenAiTestProvider};
use sentinel_core::assembler::{Assembly, Diagnostic, assemble, assemble_with_symbols};
use sentinel_core::vm::{
    Capability, ImageManifest, Machine, MachineStatus, MemorySlot, Permissions,
};
use sentinel_core::{ISA_ID, Register, decode};
use sentinel_scenario::{
    AllocationKind, AllocationMode, CompiledBundle, Runtime, ScalarValue, Severity, TickInput,
    TickResult, compile, compile_hardware,
};

const LAB_STACK_BASE: u32 = 0x2000_0000;
const LAB_STACK_SIZE: usize = 64 * 1024;
const TANK_PRESSURE_STEP: i32 = 10_000;
const MAX_AI_CONFIG_BYTES: usize = 16 * 1024;

mod tui;

static AI_CONFIG: OnceLock<BTreeMap<String, String>> = OnceLock::new();

struct FirmwareRun {
    steps: u64,
    cycles: u64,
    final_requests: BTreeMap<String, ScalarValue>,
    tank_seconds: Vec<TankSecond>,
}

struct TankSecond {
    readings: BTreeMap<String, ScalarValue>,
    actions: Vec<(String, ScalarValue)>,
    requests: BTreeMap<String, ScalarValue>,
    next_readings: BTreeMap<String, ScalarValue>,
}

struct RocketRun {
    steps: u64,
    cycles: u64,
    ticks: Vec<RocketTick>,
    final_phase: String,
    final_requests: BTreeMap<String, ScalarValue>,
}

struct RocketTick {
    result: TickResult,
    firmware_requests: BTreeMap<String, ScalarValue>,
    applied_requests: BTreeMap<String, ScalarValue>,
    transition: Option<String>,
    frame_first_pc: u32,
    frame_last_pc: u32,
    frame_steps: u64,
    frame_cycles: u64,
    issues: Vec<RocketIssue>,
}

struct RocketIssue {
    rule: String,
    severity: Severity,
    message: String,
}

#[derive(Clone, Debug)]
pub(crate) struct MissionViewFrame {
    pub tick: u64,
    pub phase: String,
    pub pc: String,
    pub steps: u64,
    pub cycles: u64,
    pub telemetry: Vec<(String, String)>,
    pub requested: Vec<(String, String)>,
    pub applied: Vec<(String, String)>,
    pub rules: Vec<String>,
    pub faults: Vec<String>,
    pub hold: bool,
    pub abort: bool,
    pub supervisor: String,
}

#[derive(Clone, Debug)]
pub(crate) struct MissionViewSnapshot {
    pub mission: String,
    pub schema: String,
    pub summary: String,
    pub frames: Vec<MissionViewFrame>,
}

fn parse_word(value: &str) -> Result<u32, String> {
    let (digits, radix) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map_or((value, 10), |digits| (digits, 16));
    u32::from_str_radix(digits, radix).map_err(|_| format!("invalid 32-bit word: {value}"))
}

fn parse_cycle_budget(value: &str) -> Result<u64, String> {
    let budget = value
        .parse::<u64>()
        .map_err(|_| format!("invalid virtual cycle budget: {value}"))?;
    if budget == 0 {
        Err("virtual cycle budget must be positive".to_owned())
    } else {
        Ok(budget)
    }
}

fn parse_tick_count(value: &str) -> Result<u64, String> {
    let count = value
        .parse::<u64>()
        .map_err(|_| format!("invalid run/tick count: {value}"))?;
    if count == 0 {
        Err("run/tick count must be positive".to_owned())
    } else {
        Ok(count)
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args
        .first()
        .is_some_and(|command| command.starts_with("ai-") || command == "mission-advice")
    {
        load_ai_config()?;
    }
    match args.as_slice() {
        [] => {
            println!("sentinel-app {ISA_ID}");
            Ok(())
        }
        [command, word] if command == "decode" => {
            let word = parse_word(word)?;
            let instruction = decode(word).map_err(|error| error.to_string())?;
            println!("{instruction:?} cycles={}", instruction.cycle_cost());
            Ok(())
        }
        [command, source] if command == "check" => {
            let assembly = assemble_file(source)?;
            println!(
                "valid: {} bytes, entry={}",
                assembly.bytes.len(),
                assembly
                    .entry
                    .map_or_else(|| "none".to_owned(), |value| format!("0x{value:08X}"))
            );
            Ok(())
        }
        [command, source] if command == "assemble" => {
            let assembly = assemble_file(source)?;
            for (index, bytes) in assembly.bytes.chunks_exact(4).enumerate() {
                let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                let address = assembly.origin + (index as u32 * 4);
                println!("{address:08X}: {word:08X}");
            }
            Ok(())
        }
        [command, source, output] if command == "assemble" => {
            let assembly = assemble_file(source)?;
            fs::write(output, &assembly.bytes)
                .map_err(|error| format!("cannot write `{output}`: {error}"))?;
            println!("wrote {} bytes to {output}", assembly.bytes.len());
            Ok(())
        }
        [command, source, cycle_budget] if command == "run" => {
            let assembly = assemble_file(source)?;
            let cycle_budget = parse_cycle_budget(cycle_budget)?;
            run_assembly(assembly, cycle_budget)
        }
        [command, source, cycle_budget] if command == "step" => {
            let assembly = assemble_file(source)?;
            let cycle_budget = parse_cycle_budget(cycle_budget)?;
            let stdin = io::stdin();
            let stdout = io::stdout();
            step_assembly(assembly, cycle_budget, stdin.lock(), stdout.lock())
        }
        [command] if command == "tui" => tui::run_tui(None),
        [command, source] if command == "tui" => tui::run_tui(Some(source)),
        [command, source] if command == "scenario-check" => {
            let source = read_text(source)?;
            let compilation = compile(&source, AllocationMode::Clean)
                .map_err(|error| error.to_string())?;
            println!(
                "valid: scenario={} publication={} bundle={} mmio={}",
                compilation.bundle.scenario_id,
                compilation.bundle.publication,
                compilation.bundle_hash,
                compilation.bundle.mmio.len()
            );
            Ok(())
        }
        [command, source] if command == "hardware-check" => {
            let source = read_text(source)?;
            let compilation = compile_hardware(&source).map_err(|error| error.to_string())?;
            println!(
                "valid: hardware={} publication={} bundle={} mmio={}",
                compilation.bundle.scenario_id,
                compilation.bundle.publication,
                compilation.bundle_hash,
                compilation.bundle.mmio.len()
            );
            Ok(())
        }
        [command, source, ticks] if command == "scenario-tick" => {
            let source = read_text(source)?;
            let ticks = parse_tick_count(ticks)?;
            let compilation = compile(&source, AllocationMode::Clean)
                .map_err(|error| error.to_string())?;
            println!(
                "scenario-run scenario={} publication={} bundle={} tick_ms={} requested_ticks={} initial_phase={}",
                compilation.bundle.scenario_id,
                compilation.bundle.publication,
                compilation.bundle_hash,
                compilation.bundle.tick_ms,
                ticks,
                compilation.bundle.phases.initial
            );
            println!(
                "tick-fields tick=zero-based_tick phase=before->after changed=committed_channel_values active_rules=triggered_rule_ids active_faults=applied_fault_ids hold=progression_held abort_latched=attempt_abort_latched"
            );
            let mut runtime = Runtime::new(compilation.bundle);
            for _ in 0..ticks {
                let result = runtime
                    .tick(&TickInput::default())
                    .map_err(|error| error.to_string())?;
                println!("{}", format_tick_result(&result));
            }
            Ok(())
        }
        [command, mission, firmware, cycle_budget] if command == "mission-run" => {
            run_mission(
                mission,
                firmware,
                parse_cycle_budget(cycle_budget)?,
            )
        }
        [command, profile, snapshot] if command == "ai-check" => {
            run_ai_check(profile, snapshot)
        }
        [command, profile, snapshot] if command == "mission-advice" => {
            println!(
                "ADVISORY-REVIEW authority=none decision_owner=operator_and_deterministic_policy"
            );
            run_ai_check(profile, snapshot)
        }
        [command, profile] if command == "ai-health" => run_ai_health(profile),
        [command, source, bundle, symbols] if command == "scenario-compile" => {
            let source = read_text(source)?;
            let compilation = compile(&source, AllocationMode::Clean)
                .map_err(|error| error.to_string())?;
            fs::write(bundle, &compilation.canonical_bundle)
                .map_err(|error| format!("cannot write `{bundle}`: {error}"))?;
            fs::write(symbols, compilation.symbols.as_bytes())
                .map_err(|error| format!("cannot write `{symbols}`: {error}"))?;
            println!(
                "wrote bundle={} symbols={} hash={}",
                bundle, symbols, compilation.bundle_hash
            );
            Ok(())
        }
        [command, source, bundle, symbols] if command == "hardware-compile" => {
            let source = read_text(source)?;
            let compilation = compile_hardware(&source).map_err(|error| error.to_string())?;
            fs::write(bundle, &compilation.canonical_bundle)
                .map_err(|error| format!("cannot write `{bundle}`: {error}"))?;
            fs::write(symbols, compilation.symbols.as_bytes())
                .map_err(|error| format!("cannot write `{symbols}`: {error}"))?;
            println!(
                "wrote bundle={} symbols={} hash={}",
                bundle, symbols, compilation.bundle_hash
            );
            Ok(())
        }
        _ => Err(
            "usage: sentinel-app [tui [source.asm] | decode <word> | check <source.asm> | assemble <source.asm> [output.bin] | run <source.asm> <cycle-budget> | step <source.asm> <cycle-budget> | hardware-check <hardware.yaml> | hardware-compile <hardware.yaml> <bundle.json> <symbols.inc> | mission-run <mission.yaml> <firmware.asm> <cycle-budget> | mission-advice <deployment|development> <snapshot.json> | scenario-check <source.yaml> | scenario-compile <source.yaml> <bundle.json> <symbols.inc> | scenario-tick <source.yaml> <ticks> | ai-health <deployment|development> | ai-check <development> <snapshot.json>]"
                .to_owned(),
        ),
    }
}

fn run_mission(mission_path: &str, firmware_path: &str, cycle_budget: u64) -> Result<(), String> {
    let source = read_text(mission_path)?;
    match mission_schema(&source)? {
        "sentinel.hardware/v0" => run_scenario_firmware(mission_path, firmware_path, cycle_budget),
        "sentinel.scenario/v0" => run_rocket_firmware(mission_path, firmware_path, cycle_budget),
        schema => Err(format!("unsupported mission schema `{schema}`")),
    }
}

pub(crate) fn execute_mission_view(
    mission_path: &str,
    firmware_path: &str,
    cycle_budget: u64,
) -> Result<MissionViewSnapshot, String> {
    let source = read_text(mission_path)?;
    match mission_schema(&source)? {
        "sentinel.hardware/v0" => {
            let compilation = compile_hardware(&source).map_err(|error| error.to_string())?;
            let symbols = compilation
                .bundle
                .mmio
                .iter()
                .map(|entry| (mmio_symbol(&entry.qualified_id), entry.address))
                .collect::<BTreeMap<_, _>>();
            let firmware_source = read_text(firmware_path)?;
            let assembly =
                assemble_with_symbols(&firmware_source, 0, &symbols).map_err(|diagnostics| {
                    render_diagnostics(Path::new(firmware_path), &diagnostics)
                })?;
            let runtime = Runtime::new(compilation.bundle.clone());
            let run = execute_firmware_tick(
                &assembly,
                &compilation.bundle,
                &runtime.state().values,
                cycle_budget,
            )?;
            let frame_count = run.tank_seconds.len();
            let frames = run
                .tank_seconds
                .into_iter()
                .enumerate()
                .map(|(index, second)| MissionViewFrame {
                    tick: index as u64 + 1,
                    phase: "tank_sequence".to_owned(),
                    pc: "persistent firmware".to_owned(),
                    steps: run.steps,
                    cycles: run.cycles,
                    telemetry: display_values(&second.next_readings),
                    requested: display_values(&second.requests),
                    applied: second
                        .actions
                        .iter()
                        .map(|(key, value)| (key.clone(), format_scalar(value)))
                        .collect(),
                    rules: Vec::new(),
                    faults: Vec::new(),
                    hold: false,
                    abort: false,
                    supervisor: "none".to_owned(),
                })
                .collect();
            Ok(MissionViewSnapshot {
                mission: compilation.bundle.scenario_id,
                schema: "sentinel.hardware/v0".to_owned(),
                summary: format!(
                    "halted: {} steps, {} cycles, {} seconds, final {}",
                    run.steps,
                    run.cycles,
                    frame_count,
                    format_values(&run.final_requests)
                ),
                frames,
            })
        }
        "sentinel.scenario/v0" => {
            let compilation =
                compile(&source, AllocationMode::Clean).map_err(|error| error.to_string())?;
            let symbols = compilation
                .bundle
                .mmio
                .iter()
                .map(|entry| (mmio_symbol(&entry.qualified_id), entry.address))
                .collect::<BTreeMap<_, _>>();
            let firmware_source = read_text(firmware_path)?;
            let assembly =
                assemble_with_symbols(&firmware_source, 0, &symbols).map_err(|diagnostics| {
                    render_diagnostics(Path::new(firmware_path), &diagnostics)
                })?;
            let run = execute_rocket_firmware(&assembly, compilation.bundle.clone(), cycle_budget)?;
            let frames = run
                .ticks
                .iter()
                .map(|tick| MissionViewFrame {
                    tick: tick.result.tick,
                    phase: format!(
                        "{} -> {}",
                        tick.result.phase_before, tick.result.phase_after
                    ),
                    pc: format!(
                        "0x{:08X}..0x{:08X}",
                        tick.frame_first_pc, tick.frame_last_pc
                    ),
                    steps: tick.frame_steps,
                    cycles: tick.frame_cycles,
                    telemetry: tick
                        .result
                        .changed_values
                        .iter()
                        .map(|(key, value)| (key.clone(), format_scalar(value)))
                        .collect(),
                    requested: display_values(&tick.firmware_requests),
                    applied: display_values(&tick.applied_requests),
                    rules: tick
                        .issues
                        .iter()
                        .map(|issue| {
                            format!(
                                "{} [{}] {}",
                                issue.rule,
                                severity_name(issue.severity),
                                issue.message
                            )
                        })
                        .collect(),
                    faults: tick.result.active_faults.clone(),
                    hold: tick.result.hold,
                    abort: tick.result.abort_latched,
                    supervisor: tick.transition.clone().unwrap_or_else(|| "none".to_owned()),
                })
                .collect();
            Ok(MissionViewSnapshot {
                mission: compilation.bundle.scenario_id,
                schema: "sentinel.scenario/v0".to_owned(),
                summary: format!(
                    "halted: {} steps, {} cycles, {} ticks, phase {}, final {}",
                    run.steps,
                    run.cycles,
                    run.ticks.len(),
                    run.final_phase,
                    format_values(&run.final_requests)
                ),
                frames,
            })
        }
        schema => Err(format!("unsupported mission schema `{schema}`")),
    }
}

fn display_values(values: &BTreeMap<String, ScalarValue>) -> Vec<(String, String)> {
    values
        .iter()
        .map(|(key, value)| (key.clone(), format_scalar(value)))
        .collect()
}

fn mission_schema(source: &str) -> Result<&str, String> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key.trim() == "schema").then_some(value.trim())
        })
        .filter(|schema| !schema.is_empty())
        .ok_or_else(|| "mission document is missing top-level `schema`".to_owned())
}

fn run_ai_health(profile: &str) -> Result<(), String> {
    let profile = parse_deployment_profile(profile)?;
    let mode = required_env("S32_AI_CHECK_MODE")?;
    if mode != "llama_cpp" {
        return Err("ai-health requires S32_AI_CHECK_MODE=llama_cpp".to_owned());
    }
    let limits = ai_limits()?;
    let provider = llama_provider(profile)?;
    let identity = provider
        .health(limits.max_response_bytes)
        .map_err(|error| error.to_string())?;
    println!(
        "ai-health status=ready backend=llama_cpp backend_version={} model={} model_sha256={}",
        identity.build_info, identity.model, identity.model_sha256
    );
    Ok(())
}

fn run_ai_check(profile: &str, snapshot_path: &str) -> Result<(), String> {
    let (_request, response) = execute_advisory_check(profile, snapshot_path)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&response)
            .map_err(|error| format!("cannot encode advisory response: {error}"))?
    );
    Ok(())
}

fn execute_advisory_check(
    profile: &str,
    snapshot_path: &str,
) -> Result<(CheckRequest, sentinel_ai_check::CheckResponse), String> {
    execute_advisory_check_with_rules(profile, snapshot_path, None)
}

pub(crate) fn execute_advisory_check_with_rules(
    profile: &str,
    snapshot_path: &str,
    rules_path: Option<&Path>,
) -> Result<(CheckRequest, sentinel_ai_check::CheckResponse), String> {
    let profile = parse_deployment_profile(profile)?;
    let limits = ai_limits()?;
    let snapshot_bytes = fs::read(snapshot_path)
        .map_err(|error| format!("cannot read `{snapshot_path}`: {error}"))?;
    let rules_path = match rules_path {
        Some(path) => path.to_path_buf(),
        None => Path::new(&required_env("S32_AI_RULES_PATH")?).to_path_buf(),
    };
    let rule_bytes = fs::read(&rules_path)
        .map_err(|error| format!("cannot read AI rules `{}`: {error}", rules_path.display()))?;
    let request = CheckRequest {
        snapshot: parse_and_validate_snapshot(&snapshot_bytes, limits)
            .map_err(|error| error.to_string())?,
        rules: parse_and_validate_rule_set(&rule_bytes, limits)
            .map_err(|error| error.to_string())?,
    };
    let mode = required_env("S32_AI_CHECK_MODE")?;
    if profile == DeploymentProfile::Deployment && mode != "llama_cpp" {
        return Err("deployment profile requires S32_AI_CHECK_MODE=llama_cpp".to_owned());
    }
    let response = match mode.as_str() {
        "llama_cpp" => llama_provider(profile)?
            .check(&request, limits)
            .map_err(|error| error.to_string())?,
        "fixture" => {
            let path = required_env("S32_AI_FIXTURE_RESPONSE_PATH")?;
            let bytes = fs::read(path)
                .map_err(|error| format!("cannot read configured AI fixture: {error}"))?;
            FixtureProvider::new(bytes)
                .check(&request, limits)
                .map_err(|error| error.to_string())?
        }
        "openai_test" => run_openai_check(profile, &request, limits)?,
        "disabled" => return Err("advisory checker is disabled".to_owned()),
        _ => return Err("invalid S32_AI_CHECK_MODE".to_owned()),
    };
    Ok((request, response))
}

fn parse_deployment_profile(value: &str) -> Result<DeploymentProfile, String> {
    match value {
        "deployment" => Ok(DeploymentProfile::Deployment),
        "development" => Ok(DeploymentProfile::Development),
        _ => Err("AI profile must be `deployment` or `development`".to_owned()),
    }
}

fn llama_provider(profile: DeploymentProfile) -> Result<LlamaCppProvider, String> {
    let base_url = required_env("S32_LLAMA_BASE_URL")?;
    if profile == DeploymentProfile::Deployment && base_url.starts_with("https://") {
        return Err("deployment llama.cpp endpoint must use configured HTTP transport".to_owned());
    }
    LlamaCppProvider::new(LlamaCppConfig {
        base_url,
        model: required_env("S32_LLAMA_MODEL_ID")?,
        model_sha256: required_env("S32_LLAMA_MODEL_SHA256")?,
        api_key: required_env("S32_LLAMA_API_KEY")?,
        timeout: Duration::from_millis(ai_timeout_ms()?),
        max_output_tokens: optional_positive_env("S32_AI_MAX_OUTPUT_TOKENS", 2_048, 8_192)? as u32,
        diagnostic_response_path: configured_value("S32_AI_DIAGNOSTIC_RESPONSE_PATH")?,
    })
    .map_err(|error| error.to_string())
}

#[cfg(feature = "openai-test")]
fn run_openai_check(
    profile: DeploymentProfile,
    request: &CheckRequest,
    limits: Limits,
) -> Result<sentinel_ai_check::CheckResponse, String> {
    if profile != DeploymentProfile::Development {
        return Err("openai_test is forbidden in the deployment profile".to_owned());
    }
    OpenAiTestProvider::new(OpenAiTestConfig {
        model: required_env("S32_OPENAI_TEST_MODEL")?,
        api_key: required_env("OPENAI_API_KEY")?,
        timeout: Duration::from_millis(ai_timeout_ms()?),
        max_output_tokens: optional_positive_env("S32_AI_MAX_OUTPUT_TOKENS", 2_048, 8_192)? as u32,
    })
    .map_err(|error| error.to_string())?
    .check(request, limits)
    .map_err(|error| error.to_string())
}

#[cfg(not(feature = "openai-test"))]
fn run_openai_check(
    profile: DeploymentProfile,
    _request: &CheckRequest,
    _limits: Limits,
) -> Result<sentinel_ai_check::CheckResponse, String> {
    if profile != DeploymentProfile::Development {
        return Err("openai_test is forbidden in the deployment profile".to_owned());
    }
    Err("openai_test requires rebuilding sentinel-app with `--features openai-test`".to_owned())
}

fn ai_limits() -> Result<Limits, String> {
    Ok(Limits {
        max_snapshot_bytes: optional_positive_env(
            "S32_AI_MAX_SNAPSHOT_BYTES",
            Limits::default().max_snapshot_bytes,
            Limits::default().max_snapshot_bytes,
        )?,
        max_response_bytes: optional_positive_env(
            "S32_AI_MAX_OUTPUT_BYTES",
            Limits::default().max_response_bytes,
            Limits::default().max_response_bytes,
        )?,
        ..Limits::default()
    })
}

fn ai_timeout_ms() -> Result<u64, String> {
    u64::try_from(optional_positive_env(
        "S32_AI_CHECK_TIMEOUT_MS",
        30_000,
        300_000,
    )?)
    .map_err(|_| "S32_AI_CHECK_TIMEOUT_MS is too large".to_owned())
}

fn optional_positive_env(name: &str, default: usize, maximum: usize) -> Result<usize, String> {
    let Some(value) = configured_value(name)? else {
        return Ok(default);
    };
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{name} must be a positive integer"))?;
    if parsed == 0 || parsed > maximum {
        Err(format!("{name} must be between 1 and {maximum}"))
    } else {
        Ok(parsed)
    }
}

fn required_env(name: &str) -> Result<String, String> {
    configured_value(name)?
        .ok_or_else(|| format!("required AI configuration value `{name}` is missing"))
}

fn configured_value(name: &str) -> Result<Option<String>, String> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => {
            Ok(AI_CONFIG.get().and_then(|config| config.get(name)).cloned())
        }
        Err(env::VarError::NotUnicode(_)) => Err(format!("{name} is not valid UTF-8")),
    }
}

fn load_ai_config() -> Result<(), String> {
    load_ai_config_with_default(Path::new("config/ai-llama-default.env"))
}

pub(crate) fn load_ai_config_with_default(default_path: &Path) -> Result<(), String> {
    if AI_CONFIG.get().is_some() {
        return Ok(());
    }
    let explicit_path = env::var("S32_AI_CONFIG_PATH").ok();
    let path = explicit_path
        .as_deref()
        .map(Path::new)
        .unwrap_or(default_path);
    let mut config = BTreeMap::new();
    load_ai_config_file(
        &path.to_string_lossy(),
        explicit_path.is_some(),
        &mut config,
    )?;
    AI_CONFIG
        .set(config)
        .map_err(|_| "AI configuration was loaded more than once".to_owned())
}

fn load_ai_config_file(
    path: &str,
    required: bool,
    config: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if !required && error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("cannot read AI config `{path}`: {error}")),
    };
    if bytes.len() > MAX_AI_CONFIG_BYTES {
        return Err(format!(
            "AI config `{path}` exceeds {MAX_AI_CONFIG_BYTES} bytes"
        ));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| format!("AI config `{path}` is not valid UTF-8"))?;
    parse_ai_config(text, path, config)
}

fn parse_ai_config(
    text: &str,
    path: &str,
    config: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, value) = line
            .split_once('=')
            .ok_or_else(|| format!("invalid AI config line {} in `{path}`", index + 1))?;
        if !is_ai_config_key(name) || value.is_empty() || value.trim() != value {
            return Err(format!("invalid AI config line {} in `{path}`", index + 1));
        }
        if config.insert(name.to_owned(), value.to_owned()).is_some() {
            return Err(format!(
                "duplicate AI config key `{name}` on line {} in `{path}`",
                index + 1
            ));
        }
    }
    Ok(())
}

fn is_ai_config_key(name: &str) -> bool {
    matches!(
        name,
        "S32_AI_CHECK_MODE"
            | "S32_AI_RULES_PATH"
            | "S32_AI_CHECK_TIMEOUT_MS"
            | "S32_AI_MAX_SNAPSHOT_BYTES"
            | "S32_AI_MAX_OUTPUT_BYTES"
            | "S32_AI_MAX_OUTPUT_TOKENS"
            | "S32_AI_FIXTURE_RESPONSE_PATH"
            | "S32_AI_DIAGNOSTIC_RESPONSE_PATH"
            | "S32_LLAMA_BASE_URL"
            | "S32_LLAMA_MODEL_ID"
            | "S32_LLAMA_MODEL_SHA256"
            | "S32_LLAMA_API_KEY"
            | "S32_OPENAI_TEST_MODEL"
    )
}

fn run_rocket_firmware(
    scenario_path: &str,
    firmware_path: &str,
    cycle_budget: u64,
) -> Result<(), String> {
    let source = read_text(scenario_path)?;
    let compilation = compile(&source, AllocationMode::Clean).map_err(|error| error.to_string())?;
    let symbols = compilation
        .bundle
        .mmio
        .iter()
        .map(|entry| (mmio_symbol(&entry.qualified_id), entry.address))
        .collect::<BTreeMap<_, _>>();
    let firmware_source = read_text(firmware_path)?;
    let assembly = assemble_with_symbols(&firmware_source, 0, &symbols)
        .map_err(|diagnostics| render_diagnostics(Path::new(firmware_path), &diagnostics))?;
    if assembly.bytes.is_empty() {
        return Err("cannot run an empty S32 firmware image".to_owned());
    }

    println!(
        "mission-firmware mission={} schema=sentinel.scenario/v0 publication={} bundle={} firmware={} start=operator_once cycle_budget={}",
        compilation.bundle.scenario_id,
        compilation.bundle.publication,
        compilation.bundle_hash,
        firmware_path,
        cycle_budget
    );
    println!(
        "supervisor-plan start_loading=operator_start arm_launch=simulated_approval begin_terminal_count=simulated_approval"
    );
    let run = execute_rocket_firmware(&assembly, compilation.bundle, cycle_budget)?;
    for tick in &run.ticks {
        println!(
            "{} asm_first_pc=0x{:08X} asm_last_pc=0x{:08X} asm_steps={} asm_cycles={} supervisor={} firmware={} applied={}",
            format_tick_result(&tick.result),
            tick.frame_first_pc,
            tick.frame_last_pc,
            tick.frame_steps,
            tick.frame_cycles,
            tick.transition.as_deref().unwrap_or("none"),
            format_values(&tick.firmware_requests),
            format_values(&tick.applied_requests)
        );
        for issue in &tick.issues {
            let label = if issue.severity == Severity::Advisory {
                "NOTICE"
            } else {
                "ISSUE"
            };
            println!(
                "{label} tick={} phase={}->{} rule={} severity={} containment={} asm_first_pc=0x{:08X} asm_last_pc=0x{:08X} message={}",
                tick.result.tick,
                tick.result.phase_before,
                tick.result.phase_after,
                issue.rule,
                severity_name(issue.severity),
                containment_name(issue.severity),
                tick.frame_first_pc,
                tick.frame_last_pc,
                serde_json::to_string(&issue.message)
                    .map_err(|error| format!("cannot encode issue message: {error}"))?
            );
        }
    }
    print_rocket_issue_summary(&run.ticks);
    println!(
        "operation=complete firmware_status=halted firmware_steps={} firmware_cycles={} scenario_ticks={} final_phase={} final_requests={}",
        run.steps,
        run.cycles,
        run.ticks.len(),
        run.final_phase,
        format_values(&run.final_requests)
    );
    Ok(())
}

fn execute_rocket_firmware(
    assembly: &Assembly,
    bundle: CompiledBundle,
    cycle_budget: u64,
) -> Result<RocketRun, String> {
    let mut runtime = Runtime::new(bundle.clone());
    let mut machine = firmware_machine(assembly, &bundle, &runtime.state().values)?;
    let mut steps = 0_u64;
    let mut write_cursor = 0_usize;
    let mut frame_actions = Vec::new();
    let mut ticks = Vec::new();
    let mut frame_first_pc = None;
    let mut frame_steps = 0_u64;
    let mut frame_cycles = 0_u64;

    while machine.status() == &MachineStatus::Running {
        let step = machine
            .step(cycle_budget)
            .map_err(|error| error.to_string())?;
        steps = steps.saturating_add(1);
        frame_first_pc.get_or_insert(step.pc);
        frame_steps = frame_steps.saturating_add(1);
        frame_cycles = frame_cycles.saturating_add(u64::from(step.cycles_charged));
        let Some(write) = machine.memory_writes().get(write_cursor).cloned() else {
            continue;
        };
        write_cursor = write_cursor.saturating_add(1);
        let Some(entry) = bundle.mmio.iter().find(|entry| {
            entry.kind == AllocationKind::ActuatorRequest && entry.address == write.address
        }) else {
            continue;
        };
        let bytes = write
            .bytes
            .get(..4)
            .ok_or_else(|| format!("request write `{}` is not 32 bits", entry.qualified_id))?;
        let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        frame_actions.push((
            entry.qualified_id.clone(),
            word_to_scalar(word, &entry.scalar_type, &bundle)?,
        ));
        if entry.qualified_id != "actuator.ignition" {
            continue;
        }
        if frame_actions.len() != 6 {
            return Err(format!(
                "rocket firmware command frame contains {} writes instead of 6",
                frame_actions.len()
            ));
        }

        let firmware_requests = actuator_requests(&machine, &bundle)?;
        let transition =
            rocket_supervisor_transition(runtime.state().phase.as_str(), &firmware_requests)?;
        let applied_requests = firmware_requests.clone();
        let channel_updates = runtime
            .state()
            .values
            .iter()
            .filter(|(reference, _)| reference.starts_with("telemetry."))
            .map(|(reference, value)| (reference.clone(), value.clone()))
            .collect();
        let result = runtime
            .tick(&TickInput {
                channel_updates,
                actuator_requests: applied_requests.clone(),
                transition: transition.clone(),
                ..TickInput::default()
            })
            .map_err(|error| error.to_string())?;
        let issues = result
            .active_rules
            .iter()
            .filter_map(|id| {
                bundle.rules.get(id).map(|rule| RocketIssue {
                    rule: id.clone(),
                    severity: rule.severity,
                    message: rule.message.clone(),
                })
            })
            .collect();
        update_machine_devices(&mut machine, &bundle, &runtime.state().values)?;
        ticks.push(RocketTick {
            result,
            firmware_requests,
            applied_requests,
            transition,
            frame_first_pc: frame_first_pc
                .ok_or_else(|| "rocket command frame has no instructions".to_owned())?,
            frame_last_pc: step.pc,
            frame_steps,
            frame_cycles,
            issues,
        });
        frame_actions.clear();
        frame_first_pc = None;
        frame_steps = 0;
        frame_cycles = 0;
    }

    if machine.status() != &MachineStatus::Halted {
        return Err(format!("rocket firmware ended with {:?}", machine.status()));
    }
    if !frame_actions.is_empty() {
        return Err("rocket firmware halted with an incomplete command frame".to_owned());
    }
    if runtime.state().phase != "complete" {
        return Err(format!(
            "rocket firmware halted before scenario completion in phase `{}`",
            runtime.state().phase
        ));
    }
    Ok(RocketRun {
        steps,
        cycles: machine.cycles(),
        ticks,
        final_phase: runtime.state().phase.clone(),
        final_requests: actuator_requests(&machine, &bundle)?,
    })
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Advisory => "advisory",
        Severity::Inhibit => "inhibit",
        Severity::Hold => "hold",
        Severity::Abort => "abort",
    }
}

fn containment_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Advisory => "observe",
        Severity::Inhibit => "inhibit_output",
        Severity::Hold => "hold_progression",
        Severity::Abort => "latch_abort",
    }
}

fn print_rocket_issue_summary(ticks: &[RocketTick]) {
    let mut issue_rules = BTreeSet::new();
    let mut notice_rules = BTreeSet::new();
    let mut issue_events = 0_usize;
    let mut notice_events = 0_usize;
    let mut hold_ticks = 0_usize;
    let mut abort_ticks = 0_usize;
    for tick in ticks {
        for issue in &tick.issues {
            if issue.severity == Severity::Advisory {
                notice_events = notice_events.saturating_add(1);
                notice_rules.insert(issue.rule.clone());
            } else {
                issue_events = issue_events.saturating_add(1);
                issue_rules.insert(issue.rule.clone());
            }
        }
        hold_ticks = hold_ticks.saturating_add(usize::from(tick.result.hold));
        abort_ticks = abort_ticks.saturating_add(usize::from(tick.result.abort_latched));
    }
    println!(
        "ISSUE-SUMMARY issue_events={} notice_events={} hold_ticks={} abort_ticks={} issue_rules={} notice_rules={}",
        issue_events,
        notice_events,
        hold_ticks,
        abort_ticks,
        format_ids(&issue_rules.into_iter().collect::<Vec<_>>()),
        format_ids(&notice_rules.into_iter().collect::<Vec<_>>())
    );
}

fn rocket_supervisor_transition(
    phase: &str,
    requests: &BTreeMap<String, ScalarValue>,
) -> Result<Option<String>, String> {
    let ignition = requests
        .get("actuator.ignition")
        .ok_or_else(|| "rocket firmware did not write `actuator.ignition`".to_owned())?;
    Ok(match (phase, ignition) {
        ("idle", _) => Some("start_loading".to_owned()),
        ("stabilize", ScalarValue::Enum(value)) if value == "armed" => {
            Some("arm_launch".to_owned())
        }
        ("armed", ScalarValue::Enum(value)) if value == "armed" => {
            Some("begin_terminal_count".to_owned())
        }
        _ => None,
    })
}

fn firmware_machine(
    assembly: &Assembly,
    bundle: &CompiledBundle,
    values: &BTreeMap<String, ScalarValue>,
) -> Result<Machine, String> {
    let mut slots = vec![
        MemorySlot::new(
            assembly.origin,
            assembly.bytes.clone(),
            Permissions::READ_EXECUTE,
        )
        .map_err(|error| error.to_string())?,
        MemorySlot::new(
            LAB_STACK_BASE,
            vec![0; LAB_STACK_SIZE],
            Permissions::READ_WRITE,
        )
        .map_err(|error| error.to_string())?,
    ];
    let mut capabilities = vec![Capability {
        base: LAB_STACK_BASE,
        length: LAB_STACK_SIZE as u32,
        read: true,
        write: true,
    }];
    for entry in &bundle.mmio {
        let value = values
            .get(&entry.qualified_id)
            .ok_or_else(|| format!("scenario value `{}` is missing", entry.qualified_id))?;
        let permissions = match entry.kind {
            AllocationKind::ActuatorRequest => Permissions::WRITE,
            AllocationKind::Telemetry | AllocationKind::Feedback | AllocationKind::Supervisor => {
                Permissions::READ
            }
        };
        slots.push(
            MemorySlot::new(
                entry.address,
                scalar_to_word(value, &entry.scalar_type, bundle)
                    .map_err(|error| format!("{}: {error}", entry.qualified_id))?
                    .to_le_bytes()
                    .to_vec(),
                permissions,
            )
            .map_err(|error| error.to_string())?,
        );
        capabilities.push(Capability {
            base: entry.address,
            length: 4,
            read: permissions.read,
            write: permissions.write,
        });
    }
    Machine::new(
        ImageManifest {
            entry_point: assembly.entry.unwrap_or(assembly.origin),
            stack_low: LAB_STACK_BASE,
            stack_high: LAB_STACK_BASE + LAB_STACK_SIZE as u32,
            capabilities,
        },
        slots,
    )
    .map_err(|error| error.to_string())
}

fn format_tick_result(result: &TickResult) -> String {
    format!(
        "tick={} phase={}->{} changed={} active_rules={} active_faults={} hold={} abort_latched={}",
        result.tick,
        result.phase_before,
        result.phase_after,
        format_changes(result),
        format_ids(&result.active_rules),
        format_ids(&result.active_faults),
        result.hold,
        result.abort_latched
    )
}

fn format_changes(result: &TickResult) -> String {
    if result.changed_values.is_empty() {
        return "[]".to_owned();
    }
    let values = result
        .changed_values
        .iter()
        .map(|(reference, value)| format!("{reference}={}", format_scalar(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

fn format_ids(ids: &[String]) -> String {
    if ids.is_empty() {
        "[]".to_owned()
    } else {
        format!("[{}]", ids.join(","))
    }
}

fn format_scalar(value: &ScalarValue) -> String {
    match value {
        ScalarValue::Bool(value) => value.to_string(),
        ScalarValue::Signed(value) => value.to_string(),
        ScalarValue::Unsigned(value) => value.to_string(),
        ScalarValue::Enum(value) => value.clone(),
    }
}

fn run_scenario_firmware(
    scenario_path: &str,
    firmware_path: &str,
    cycle_budget: u64,
) -> Result<(), String> {
    let scenario_source = read_text(scenario_path)?;
    let compilation = compile_hardware(&scenario_source).map_err(|error| error.to_string())?;
    let symbols = compilation
        .bundle
        .mmio
        .iter()
        .map(|entry| (mmio_symbol(&entry.qualified_id), entry.address))
        .collect::<BTreeMap<_, _>>();
    let firmware_source = read_text(firmware_path)?;
    let assembly = assemble_with_symbols(&firmware_source, 0, &symbols)
        .map_err(|diagnostics| render_diagnostics(Path::new(firmware_path), &diagnostics))?;
    if assembly.bytes.is_empty() {
        return Err("cannot run an empty S32 firmware image".to_owned());
    }

    println!(
        "mission-firmware mission={} schema=sentinel.hardware/v0 publication={} bundle={} firmware={} start=operator_once cycle_budget={}",
        compilation.bundle.scenario_id,
        compilation.bundle.publication,
        compilation.bundle_hash,
        firmware_path,
        cycle_budget
    );
    for entry in &compilation.bundle.mmio {
        let direction = match entry.kind {
            AllocationKind::ActuatorRequest => "firmware_write_request",
            AllocationKind::Telemetry | AllocationKind::Feedback => "firmware_read",
            AllocationKind::Supervisor => "supervisor_read",
        };
        println!(
            "mmio symbol={} address=0x{:08X} reference={} access={}",
            mmio_symbol(&entry.qualified_id),
            entry.address,
            entry.qualified_id,
            direction
        );
    }

    let bundle = compilation.bundle;
    let runtime = Runtime::new(bundle.clone());
    let run = execute_firmware_tick(&assembly, &bundle, &runtime.state().values, cycle_budget)?;
    for (index, second) in run.tank_seconds.iter().enumerate() {
        println!(
            "second={} readings={} actions={} requests={} next_readings={}",
            index + 1,
            format_values(&second.readings),
            format_action_values(&second.actions),
            format_values(&second.requests),
            format_values(&second.next_readings)
        );
    }
    println!(
        "operation=complete firmware_status=halted firmware_steps={} firmware_cycles={} simulated_seconds={} final_requests={}",
        run.steps,
        run.cycles,
        run.tank_seconds.len(),
        format_values(&run.final_requests)
    );
    Ok(())
}

fn execute_firmware_tick(
    assembly: &Assembly,
    bundle: &CompiledBundle,
    hardware_values: &BTreeMap<String, ScalarValue>,
    cycle_budget: u64,
) -> Result<FirmwareRun, String> {
    let mut slots = vec![
        MemorySlot::new(
            assembly.origin,
            assembly.bytes.clone(),
            Permissions::READ_EXECUTE,
        )
        .map_err(|error| error.to_string())?,
        MemorySlot::new(
            LAB_STACK_BASE,
            vec![0; LAB_STACK_SIZE],
            Permissions::READ_WRITE,
        )
        .map_err(|error| error.to_string())?,
    ];
    let mut capabilities = vec![Capability {
        base: LAB_STACK_BASE,
        length: LAB_STACK_SIZE as u32,
        read: true,
        write: true,
    }];
    for entry in &bundle.mmio {
        let value = hardware_values
            .get(&entry.qualified_id)
            .ok_or_else(|| format!("hardware value `{}` is missing", entry.qualified_id))?;
        let permissions = match entry.kind {
            AllocationKind::ActuatorRequest => Permissions::WRITE,
            AllocationKind::Telemetry | AllocationKind::Feedback | AllocationKind::Supervisor => {
                Permissions::READ
            }
        };
        slots.push(
            MemorySlot::new(
                entry.address,
                scalar_to_word(value, &entry.scalar_type, bundle)?
                    .to_le_bytes()
                    .to_vec(),
                permissions,
            )
            .map_err(|error| error.to_string())?,
        );
        capabilities.push(Capability {
            base: entry.address,
            length: 4,
            read: permissions.read,
            write: permissions.write,
        });
    }
    let mut machine = Machine::new(
        ImageManifest {
            entry_point: assembly.entry.unwrap_or(assembly.origin),
            stack_low: LAB_STACK_BASE,
            stack_high: LAB_STACK_BASE + LAB_STACK_SIZE as u32,
            capabilities,
        },
        slots,
    )
    .map_err(|error| error.to_string())?;
    let mut hardware_values = hardware_values.clone();
    let mut steps = 0_u64;
    let mut write_cursor = 0_usize;
    let mut second_actions = Vec::new();
    let mut tank_seconds = Vec::new();
    while machine.status() == &MachineStatus::Running {
        machine
            .step(cycle_budget)
            .map_err(|error| error.to_string())?;
        steps = steps.saturating_add(1);
        let Some(write) = machine.memory_writes().get(write_cursor).cloned() else {
            continue;
        };
        write_cursor = write_cursor.saturating_add(1);
        let Some(entry) = bundle.mmio.iter().find(|entry| {
            entry.kind == AllocationKind::ActuatorRequest && entry.address == write.address
        }) else {
            continue;
        };
        let bytes = write
            .bytes
            .get(..4)
            .ok_or_else(|| format!("request write `{}` is not 32 bits", entry.qualified_id))?;
        let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let action = (
            entry.qualified_id.clone(),
            word_to_scalar(word, &entry.scalar_type, bundle)?,
        );
        second_actions.push(action);
        if entry.qualified_id == "actuator.outlet_valve" {
            let readings = tank_readings(&hardware_values)?;
            let requests = actuator_requests(&machine, bundle)?;
            advance_tank_hardware(&mut hardware_values, &requests)?;
            update_machine_devices(&mut machine, bundle, &hardware_values)?;
            tank_seconds.push(TankSecond {
                readings,
                actions: std::mem::take(&mut second_actions),
                requests,
                next_readings: tank_readings(&hardware_values)?,
            });
        }
    }
    if machine.status() != &MachineStatus::Halted {
        return Err(format!("tank firmware ended with {:?}", machine.status()));
    }
    if !second_actions.is_empty() {
        return Err("tank firmware halted with an incomplete valve-command pair".to_owned());
    }
    let requests = actuator_requests(&machine, bundle)?;
    Ok(FirmwareRun {
        steps,
        cycles: machine.cycles(),
        final_requests: requests,
        tank_seconds,
    })
}

fn actuator_requests(
    machine: &Machine,
    bundle: &CompiledBundle,
) -> Result<BTreeMap<String, ScalarValue>, String> {
    let mut requests = BTreeMap::new();
    for entry in &bundle.mmio {
        if entry.kind != AllocationKind::ActuatorRequest {
            continue;
        }
        let bytes = machine
            .slot_bytes(entry.address)
            .and_then(|bytes| bytes.get(..4))
            .ok_or_else(|| format!("request slot `{}` is missing", entry.qualified_id))?;
        let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        requests.insert(
            entry.qualified_id.clone(),
            word_to_scalar(word, &entry.scalar_type, bundle)?,
        );
    }
    Ok(requests)
}

fn tank_readings(
    hardware_values: &BTreeMap<String, ScalarValue>,
) -> Result<BTreeMap<String, ScalarValue>, String> {
    ["telemetry.tank_pressure"]
        .into_iter()
        .map(|reference| {
            hardware_values
                .get(reference)
                .cloned()
                .map(|value| (reference.to_owned(), value))
                .ok_or_else(|| format!("tank hardware is missing `{reference}`"))
        })
        .collect()
}

fn advance_tank_hardware(
    hardware_values: &mut BTreeMap<String, ScalarValue>,
    requests: &BTreeMap<String, ScalarValue>,
) -> Result<(), String> {
    let inlet_open = valve_request_is_open(requests, "actuator.inlet_valve")?;
    let outlet_open = valve_request_is_open(requests, "actuator.outlet_valve")?;
    if inlet_open && outlet_open {
        return Err("tank controller requested inlet and outlet open together".to_owned());
    }

    let pressure = match hardware_values.get("telemetry.tank_pressure") {
        Some(ScalarValue::Signed(value)) => *value,
        Some(_) => return Err("tank pressure is not an i32 value".to_owned()),
        None => return Err("tank hardware is missing `telemetry.tank_pressure`".to_owned()),
    };
    let next_pressure = if inlet_open {
        pressure.saturating_add(TANK_PRESSURE_STEP).min(100_000)
    } else if outlet_open {
        pressure.saturating_sub(TANK_PRESSURE_STEP).max(0)
    } else {
        pressure
    };
    hardware_values.insert(
        "telemetry.tank_pressure".to_owned(),
        ScalarValue::Signed(next_pressure),
    );
    set_valve_hardware(hardware_values, "inlet_valve", inlet_open);
    set_valve_hardware(hardware_values, "outlet_valve", outlet_open);
    Ok(())
}

fn update_machine_devices(
    machine: &mut Machine,
    bundle: &CompiledBundle,
    hardware_values: &BTreeMap<String, ScalarValue>,
) -> Result<(), String> {
    for entry in &bundle.mmio {
        if !matches!(
            entry.kind,
            AllocationKind::Telemetry | AllocationKind::Feedback
        ) {
            continue;
        }
        let value = hardware_values
            .get(&entry.qualified_id)
            .ok_or_else(|| format!("hardware value `{}` is missing", entry.qualified_id))?;
        let bytes = scalar_to_word(value, &entry.scalar_type, bundle)?.to_le_bytes();
        if !machine.update_device_slot(entry.address, &bytes) {
            return Err(format!(
                "cannot update device slot `{}`",
                entry.qualified_id
            ));
        }
    }
    Ok(())
}

fn valve_request_is_open(
    requests: &BTreeMap<String, ScalarValue>,
    reference: &str,
) -> Result<bool, String> {
    match requests.get(reference) {
        Some(ScalarValue::Enum(value)) if value == "open" => Ok(true),
        Some(ScalarValue::Enum(value)) if value == "closed" => Ok(false),
        Some(ScalarValue::Enum(value)) => Err(format!("invalid valve state `{value}`")),
        Some(_) => Err(format!("valve request `{reference}` is not an enum")),
        None => Err(format!("firmware did not write `{reference}`")),
    }
}

fn set_valve_hardware(hardware_values: &mut BTreeMap<String, ScalarValue>, id: &str, open: bool) {
    let value = ScalarValue::Enum(if open { "open" } else { "closed" }.to_owned());
    hardware_values.insert(format!("actuator.{id}"), value.clone());
    hardware_values.insert(format!("feedback.{id}_position"), value);
}

fn scalar_to_word(
    value: &ScalarValue,
    scalar_type: &str,
    bundle: &CompiledBundle,
) -> Result<u32, String> {
    match (scalar_type, value) {
        ("bool", ScalarValue::Bool(value)) => Ok(u32::from(*value)),
        ("i32", ScalarValue::Signed(value)) => Ok(*value as u32),
        ("i32", ScalarValue::Unsigned(value)) => i32::try_from(*value)
            .map(|value| value as u32)
            .map_err(|_| format!("value does not fit scalar type `{scalar_type}`")),
        ("u32", ScalarValue::Unsigned(value)) => Ok(*value),
        ("u32", ScalarValue::Signed(value)) => u32::try_from(*value)
            .map_err(|_| format!("value does not fit scalar type `{scalar_type}`")),
        (enum_id, ScalarValue::Enum(value)) => bundle
            .types
            .get(enum_id)
            .and_then(|definition| definition.variants.iter().position(|item| item == value))
            .and_then(|ordinal| u32::try_from(ordinal).ok())
            .ok_or_else(|| format!("unknown enum value `{enum_id}.{value}`")),
        _ => Err(format!("value does not match scalar type `{scalar_type}`")),
    }
}

fn word_to_scalar(
    word: u32,
    scalar_type: &str,
    bundle: &CompiledBundle,
) -> Result<ScalarValue, String> {
    match scalar_type {
        "bool" => match word {
            0 => Ok(ScalarValue::Bool(false)),
            1 => Ok(ScalarValue::Bool(true)),
            _ => Err(format!("firmware wrote invalid bool value {word}")),
        },
        "i32" => Ok(ScalarValue::Signed(word as i32)),
        "u32" => Ok(ScalarValue::Unsigned(word)),
        enum_id => bundle
            .types
            .get(enum_id)
            .and_then(|definition| definition.variants.get(word as usize))
            .cloned()
            .map(ScalarValue::Enum)
            .ok_or_else(|| format!("firmware wrote invalid `{enum_id}` ordinal {word}")),
    }
}

fn mmio_symbol(reference: &str) -> String {
    format!(
        "S32_{}",
        reference
            .chars()
            .map(|character| if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            })
            .collect::<String>()
    )
}

fn format_values(values: &BTreeMap<String, ScalarValue>) -> String {
    if values.is_empty() {
        return "[]".to_owned();
    }
    format!(
        "[{}]",
        values
            .iter()
            .map(|(reference, value)| format!("{reference}={}", format_scalar(value)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn format_action_values(values: &[(String, ScalarValue)]) -> String {
    if values.is_empty() {
        return "[]".to_owned();
    }
    format!(
        "[{}]",
        values
            .iter()
            .map(|(reference, value)| format!("{reference}={}", format_scalar(value)))
            .collect::<Vec<_>>()
            .join("->")
    )
}

fn run_assembly(
    assembly: sentinel_core::assembler::Assembly,
    cycle_budget: u64,
) -> Result<(), String> {
    let mut machine = build_lab_machine(assembly)?;
    let result = machine
        .run(cycle_budget)
        .map_err(|error| error.to_string())?;
    println!(
        "status={} steps={} cycles={} pc=0x{:08X} hi=0x{:08X} lo=0x{:08X}",
        machine_status_label(&result.status),
        result.steps,
        result.cycles,
        machine.pc(),
        machine.hi(),
        machine.lo()
    );
    let stdout = io::stdout();
    write_registers(&machine, &mut stdout.lock())?;
    terminal_execution_result(&result.status)
}

fn build_lab_machine(assembly: Assembly) -> Result<Machine, String> {
    if assembly.bytes.is_empty() {
        return Err("cannot run an empty S32 image".to_owned());
    }
    let entry_point = assembly.entry.unwrap_or(assembly.origin);
    let program = MemorySlot::new(assembly.origin, assembly.bytes, Permissions::READ_EXECUTE)
        .map_err(|error| error.to_string())?;
    let stack = MemorySlot::new(
        LAB_STACK_BASE,
        vec![0; LAB_STACK_SIZE],
        Permissions::READ_WRITE,
    )
    .map_err(|error| error.to_string())?;
    Machine::new(
        ImageManifest {
            entry_point,
            stack_low: LAB_STACK_BASE,
            stack_high: LAB_STACK_BASE + LAB_STACK_SIZE as u32,
            capabilities: vec![Capability {
                base: LAB_STACK_BASE,
                length: LAB_STACK_SIZE as u32,
                read: true,
                write: true,
            }],
        },
        vec![program, stack],
    )
    .map_err(|error| error.to_string())
}

fn step_assembly<R: BufRead, W: Write>(
    assembly: Assembly,
    cycle_budget: u64,
    mut input: R,
    mut output: W,
) -> Result<(), String> {
    let mut machine = build_lab_machine(assembly)?;
    let mut steps = 0_u64;
    writeln!(
        output,
        "S32 stepper commands: Enter/s=step, s N=step N, c=continue, r=registers, h=help, q=quit"
    )
    .map_err(|error| format!("cannot write stepper output: {error}"))?;
    write_stepper_state(&machine, steps, cycle_budget, &mut output)?;

    let mut command = String::new();
    while machine.status() == &MachineStatus::Running {
        write!(output, "s32> ")
            .and_then(|()| output.flush())
            .map_err(|error| format!("cannot write stepper prompt: {error}"))?;
        command.clear();
        let read = input
            .read_line(&mut command)
            .map_err(|error| format!("cannot read stepper command: {error}"))?;
        if read == 0 {
            writeln!(output, "stepper-exit reason=end_of_input")
                .map_err(|error| format!("cannot write stepper output: {error}"))?;
            return Ok(());
        }
        let trimmed = command.trim();
        match trimmed {
            "" | "s" => execute_visible_steps(
                &mut machine,
                &mut steps,
                1,
                cycle_budget,
                &mut output,
            )?,
            "c" => {
                while machine.status() == &MachineStatus::Running {
                    execute_visible_steps(
                        &mut machine,
                        &mut steps,
                        1,
                        cycle_budget,
                        &mut output,
                    )?;
                }
            }
            "r" => write_registers(&machine, &mut output)?,
            "h" | "help" => writeln!(
                output,
                "Enter or s executes one instruction; s N executes N; c continues; r prints registers; q quits"
            )
            .map_err(|error| format!("cannot write stepper output: {error}"))?,
            "q" | "quit" => {
                writeln!(
                    output,
                    "stepper-exit reason=operator steps={steps} cycles={} pc=0x{:08X}",
                    machine.cycles(),
                    machine.pc()
                )
                .map_err(|error| format!("cannot write stepper output: {error}"))?;
                return Ok(());
            }
            _ => {
                let Some(count) = trimmed.strip_prefix("s ") else {
                    writeln!(output, "unknown command; enter h for help")
                        .map_err(|error| format!("cannot write stepper output: {error}"))?;
                    continue;
                };
                let Ok(count) = count.trim().parse::<u64>() else {
                    writeln!(output, "step count must be a positive integer")
                        .map_err(|error| format!("cannot write stepper output: {error}"))?;
                    continue;
                };
                if count == 0 {
                    writeln!(output, "step count must be a positive integer")
                        .map_err(|error| format!("cannot write stepper output: {error}"))?;
                    continue;
                }
                execute_visible_steps(
                    &mut machine,
                    &mut steps,
                    count,
                    cycle_budget,
                    &mut output,
                )?;
            }
        }
    }

    writeln!(
        output,
        "stepper-complete status={} steps={steps} cycles={} pc=0x{:08X}",
        machine_status_label(machine.status()),
        machine.cycles(),
        machine.pc()
    )
    .map_err(|error| format!("cannot write stepper output: {error}"))?;
    write_registers(&machine, &mut output)?;
    terminal_execution_result(machine.status())
}

fn execute_visible_steps<W: Write>(
    machine: &mut Machine,
    steps: &mut u64,
    count: u64,
    cycle_budget: u64,
    output: &mut W,
) -> Result<(), String> {
    for _ in 0..count {
        if machine.status() != &MachineStatus::Running {
            break;
        }
        let before = snapshot_registers(machine);
        let before_hi = machine.hi();
        let before_lo = machine.lo();
        let first_write = machine.memory_writes().len();
        let result = machine
            .step(cycle_budget)
            .map_err(|error| error.to_string())?;
        *steps = steps.saturating_add(1);
        let changes = format_register_changes(machine, &before, before_hi, before_lo);
        let instruction = result
            .instruction
            .map_or_else(|| "unavailable".to_owned(), |value| format!("{value:?}"));
        let writes = format_memory_writes(
            machine
                .memory_writes()
                .get(first_write..)
                .unwrap_or_default(),
        );
        writeln!(
            output,
            "step={} pc=0x{:08X} instruction={instruction} cost={} cycles={} next_pc=0x{:08X} status={} changes={changes} writes={writes}",
            *steps,
            result.pc,
            result.cycles_charged,
            machine.cycles(),
            machine.pc(),
            machine_status_label(&result.status)
        )
        .map_err(|error| format!("cannot write stepper output: {error}"))?;
    }
    Ok(())
}

fn snapshot_registers(machine: &Machine) -> [u32; 32] {
    std::array::from_fn(|index| {
        Register::new(index as u8).map_or(0, |register| machine.register(register))
    })
}

fn format_register_changes(
    machine: &Machine,
    before: &[u32; 32],
    before_hi: u32,
    before_lo: u32,
) -> String {
    let mut changes = before
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            let register = Register::new(index as u8)?;
            let after = machine.register(register);
            (*value != after).then(|| format!("R{index:02}=0x{after:08X}"))
        })
        .collect::<Vec<_>>();
    if before_hi != machine.hi() {
        changes.push(format!("HI=0x{:08X}", machine.hi()));
    }
    if before_lo != machine.lo() {
        changes.push(format!("LO=0x{:08X}", machine.lo()));
    }
    format!("[{}]", changes.join(","))
}

fn format_memory_writes(writes: &[sentinel_core::vm::MemoryWrite]) -> String {
    let writes = writes
        .iter()
        .map(|write| {
            let bytes = write
                .bytes
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<String>();
            format!("0x{:08X}={bytes}", write.address)
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{writes}]")
}

fn write_stepper_state<W: Write>(
    machine: &Machine,
    steps: u64,
    cycle_budget: u64,
    output: &mut W,
) -> Result<(), String> {
    writeln!(
        output,
        "stepper-state status={} steps={steps} cycles={} pc=0x{:08X} budget={cycle_budget}",
        machine_status_label(machine.status()),
        machine.cycles(),
        machine.pc()
    )
    .map_err(|error| format!("cannot write stepper output: {error}"))
}

fn machine_status_label(status: &MachineStatus) -> String {
    match status {
        MachineStatus::Running => "running".to_owned(),
        MachineStatus::Halted => "halted".to_owned(),
        MachineStatus::Trapped(trap) => format!("trapped:{trap}"),
    }
}

fn write_registers<W: Write>(machine: &Machine, output: &mut W) -> Result<(), String> {
    for row in 0..4 {
        let first = row * 8;
        let registers = (first..first + 8)
            .filter_map(|index| Register::new(index as u8))
            .map(|register| {
                format!(
                    "R{:02}=0x{:08X}",
                    register.index(),
                    machine.register(register)
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        writeln!(output, "{registers}")
            .map_err(|error| format!("cannot write register output: {error}"))?;
    }
    Ok(())
}

fn terminal_execution_result(status: &MachineStatus) -> Result<(), String> {
    match status {
        MachineStatus::Halted => Ok(()),
        MachineStatus::Trapped(trap) => Err(format!("S32 execution trapped: {trap}")),
        MachineStatus::Running => Err("S32 execution stopped while still running".to_owned()),
    }
}

fn assemble_file(path: &str) -> Result<sentinel_core::assembler::Assembly, String> {
    let source = read_text(path)?;
    assemble(&source, 0).map_err(|diagnostics| render_diagnostics(Path::new(path), &diagnostics))
}

fn read_text(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("cannot read `{path}`: {error}"))
}

fn render_diagnostics(path: &Path, diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| format!("{}:{diagnostic}", path.display()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::Cursor;

    use sentinel_core::assembler::{assemble, assemble_with_symbols};
    use sentinel_scenario::{
        AllocationMode, Runtime, ScalarValue, Severity, compile, compile_hardware,
    };

    use super::{
        execute_firmware_tick, execute_rocket_firmware, mission_schema, mmio_symbol,
        parse_ai_config, parse_cycle_budget, parse_tick_count, parse_word, step_assembly,
    };

    const SCENARIO: &str = include_str!("../../../examples/lab-scenario.yaml");
    const FIRMWARE: &str = include_str!("../../../examples/valve-controller.asm");
    const ROCKET_SCENARIO: &str = include_str!("../../../examples/rocket-launch-default.yaml");
    const ROCKET_FIRMWARE: &str = include_str!("../../../examples/rocket-controller.asm");

    #[test]
    fn parses_decimal_and_hex_words() {
        assert_eq!(parse_word("42"), Ok(42));
        assert_eq!(parse_word("0xFC000001"), Ok(0xFC00_0001));
        assert!(parse_word("0x100000000").is_err());
    }

    #[test]
    fn requires_a_positive_decimal_cycle_budget() {
        assert_eq!(parse_cycle_budget("100"), Ok(100));
        assert!(parse_cycle_budget("0").is_err());
        assert!(parse_cycle_budget("forever").is_err());
    }

    #[test]
    fn requires_a_positive_decimal_tick_count() {
        assert_eq!(parse_tick_count("3"), Ok(3));
        assert!(parse_tick_count("0").is_err());
        assert!(parse_tick_count("many").is_err());
    }

    #[test]
    fn interactive_stepper_steps_inspects_and_continues() {
        let assembly = assemble(
            ".entry start\nstart:\nli r1, 3\nloop:\naddiu r1, r1, -1\nbne r1, r0, loop\nhalt\n",
            0,
        )
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let input = Cursor::new(b"s 2\nr\nc\n");
        let mut output = Vec::new();

        step_assembly(assembly, 9, input, &mut output).unwrap_or_else(|error| panic!("{error}"));

        let output = String::from_utf8(output).unwrap_or_else(|error| panic!("{error}"));
        assert!(output.contains("step=1 pc=0x00000000"));
        assert!(output.contains("step=2 pc=0x00000004"));
        assert!(output.contains("changes=[R01=0x00000003]"));
        assert!(output.contains("writes=[]"));
        assert!(output.contains("R01=0x00000003"));
        assert!(output.contains("stepper-complete status=halted steps=9 cycles=9 pc=0x00000014"));
        assert!(output.contains("R01=0x00000000"));
    }

    #[test]
    fn identifies_supported_mission_document_schemas() {
        assert_eq!(
            mission_schema("# mission\nschema: sentinel.hardware/v0\nid: tank"),
            Ok("sentinel.hardware/v0")
        );
        assert_eq!(
            mission_schema("schema: sentinel.scenario/v0\nid: rocket"),
            Ok("sentinel.scenario/v0")
        );
        assert!(mission_schema("id: missing_schema").is_err());
    }

    #[test]
    fn parses_strict_ai_configuration() {
        let mut config = BTreeMap::new();
        parse_ai_config(
            "# defaults\nS32_AI_CHECK_MODE=llama_cpp\nS32_LLAMA_MODEL_ID=qwen2.5-1.5b\nS32_LLAMA_API_KEY=test-key\nS32_AI_DIAGNOSTIC_RESPONSE_PATH=/tmp/llama-response.json\n",
            "defaults",
            &mut config,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            config.get("S32_LLAMA_MODEL_ID").map(String::as_str),
            Some("qwen2.5-1.5b")
        );
        assert_eq!(
            config.get("S32_LLAMA_API_KEY").map(String::as_str),
            Some("test-key")
        );
        assert_eq!(
            config
                .get("S32_AI_DIAGNOSTIC_RESPONSE_PATH")
                .map(String::as_str),
            Some("/tmp/llama-response.json")
        );
        assert!(
            parse_ai_config("UNKNOWN=value\n", "bad", &mut config).is_err(),
            "unknown configuration keys must be rejected"
        );
        let mut duplicate = BTreeMap::new();
        assert!(
            parse_ai_config(
                "S32_AI_CHECK_MODE=llama_cpp\nS32_AI_CHECK_MODE=fixture\n",
                "duplicate",
                &mut duplicate
            )
            .is_err(),
            "duplicate configuration keys must be rejected"
        );
    }

    #[test]
    fn firmware_loads_holds_and_unloads_the_tank() {
        let compilation = compile_hardware(SCENARIO).unwrap_or_else(|error| panic!("{error}"));
        let symbols = compilation
            .bundle
            .mmio
            .iter()
            .map(|entry| (mmio_symbol(&entry.qualified_id), entry.address))
            .collect::<BTreeMap<_, _>>();
        let assembly = assemble_with_symbols(FIRMWARE, 0, &symbols)
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let runtime = Runtime::new(compilation.bundle.clone());
        let run = execute_firmware_tick(
            &assembly,
            &compilation.bundle,
            &runtime.state().values,
            1_000,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(run.tank_seconds.len(), 21);
        assert_eq!(
            run.tank_seconds
                .iter()
                .map(|second| second.actions.len())
                .sum::<usize>(),
            42
        );

        for second in &run.tank_seconds[0..5] {
            assert_eq!(
                second.requests.get("actuator.inlet_valve"),
                Some(&ScalarValue::Enum("open".to_owned()))
            );
            assert_eq!(
                second.requests.get("actuator.outlet_valve"),
                Some(&ScalarValue::Enum("closed".to_owned()))
            );
        }
        for second in &run.tank_seconds[5..15] {
            assert_eq!(
                second.requests.get("actuator.inlet_valve"),
                Some(&ScalarValue::Enum("closed".to_owned()))
            );
            assert_eq!(
                second.requests.get("actuator.outlet_valve"),
                Some(&ScalarValue::Enum("closed".to_owned()))
            );
        }
        for second in &run.tank_seconds[15..20] {
            assert_eq!(
                second.requests.get("actuator.inlet_valve"),
                Some(&ScalarValue::Enum("closed".to_owned()))
            );
            assert_eq!(
                second.requests.get("actuator.outlet_valve"),
                Some(&ScalarValue::Enum("open".to_owned()))
            );
        }
        assert_eq!(
            run.tank_seconds[20].requests.get("actuator.inlet_valve"),
            Some(&ScalarValue::Enum("closed".to_owned()))
        );
        assert_eq!(
            run.tank_seconds[20].requests.get("actuator.outlet_valve"),
            Some(&ScalarValue::Enum("closed".to_owned()))
        );
        assert_eq!(
            run.tank_seconds[20]
                .next_readings
                .get("telemetry.tank_pressure"),
            Some(&ScalarValue::Signed(0))
        );
    }

    #[test]
    fn persistent_firmware_controls_the_nominal_rocket_sequence() {
        let compilation = compile(ROCKET_SCENARIO, AllocationMode::Clean)
            .unwrap_or_else(|error| panic!("{error}"));
        let symbols = compilation
            .bundle
            .mmio
            .iter()
            .map(|entry| (mmio_symbol(&entry.qualified_id), entry.address))
            .collect::<BTreeMap<_, _>>();
        let assembly = assemble_with_symbols(ROCKET_FIRMWARE, 0, &symbols)
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let run = execute_rocket_firmware(&assembly, compilation.bundle, 5_000)
            .unwrap_or_else(|error| panic!("{error}"));

        assert_eq!(run.final_phase, "complete");
        assert_eq!(run.ticks.len(), 19);
        assert!(run.ticks.iter().all(|tick| tick.frame_steps > 0));
        assert!(run.ticks.iter().all(|tick| tick.frame_cycles > 0));
        assert!(
            run.ticks
                .iter()
                .all(|tick| tick.frame_first_pc % 4 == 0 && tick.frame_last_pc % 4 == 0)
        );
        assert!(run.ticks.iter().any(|tick| {
            tick.issues.iter().any(|issue| {
                issue.rule == "valve_feedback_mismatch" && issue.severity == Severity::Hold
            })
        }));
        assert!(
            run.ticks
                .iter()
                .any(|tick| tick.result.phase_after == "loading")
        );
        assert!(
            run.ticks
                .iter()
                .any(|tick| tick.result.phase_after == "terminal_count")
        );
        assert!(
            run.ticks
                .iter()
                .any(|tick| tick.result.phase_after == "ignition")
        );
        let final_tick = run.ticks.last().expect("rocket run must have ticks");
        assert_eq!(final_tick.result.phase_after, "complete");
        assert!(!final_tick.result.hold);
        assert!(final_tick.result.active_rules.is_empty());
        for actuator in [
            "actuator.fuel_fill_valve",
            "actuator.oxidizer_fill_valve",
            "actuator.fuel_main_valve",
            "actuator.oxidizer_main_valve",
            "actuator.vent_valve",
        ] {
            assert_eq!(
                run.final_requests.get(actuator),
                Some(&ScalarValue::Enum("closed".to_owned()))
            );
        }
        assert_eq!(
            run.final_requests.get("actuator.ignition"),
            Some(&ScalarValue::Enum("safe".to_owned()))
        );
    }
}
