use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use sentinel_core::assembler::{Assembly, Diagnostic, assemble, assemble_with_symbols};
use sentinel_core::vm::{
    Capability, ImageManifest, Machine, MachineStatus, MemorySlot, Permissions,
};
use sentinel_core::{ISA_ID, Register, decode};
use sentinel_scenario::{
    AllocationKind, AllocationMode, CompiledBundle, Runtime, ScalarValue, TickInput, TickResult,
    compile, compile_hardware,
};

const LAB_STACK_BASE: u32 = 0x2000_0000;
const LAB_STACK_SIZE: usize = 64 * 1024;
const TANK_PRESSURE_STEP: i32 = 10_000;

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
        [command, scenario, firmware, cycle_budget] if command == "tank-run" => {
            run_scenario_firmware(
                scenario,
                firmware,
                parse_cycle_budget(cycle_budget)?,
            )
        }
        [command, scenario, firmware, cycle_budget] if command == "rocket-run" => {
            run_rocket_firmware(
                scenario,
                firmware,
                parse_cycle_budget(cycle_budget)?,
            )
        }
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
            "usage: sentinel-app [decode <word> | check <source.asm> | assemble <source.asm> [output.bin] | run <source.asm> <cycle-budget> | hardware-check <hardware.yaml> | hardware-compile <hardware.yaml> <bundle.json> <symbols.inc> | tank-run <hardware.yaml> <firmware.asm> <cycle-budget> | rocket-run <scenario.yaml> <firmware.asm> <cycle-budget> | scenario-check <source.yaml> | scenario-compile <source.yaml> <bundle.json> <symbols.inc> | scenario-tick <source.yaml> <ticks>]"
                .to_owned(),
        ),
    }
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
        "rocket-firmware scenario={} publication={} bundle={} firmware={} start=operator_once cycle_budget={}",
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
            "{} supervisor={} firmware={} applied={}",
            format_tick_result(&tick.result),
            tick.transition.as_deref().unwrap_or("none"),
            format_values(&tick.firmware_requests),
            format_values(&tick.applied_requests)
        );
    }
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
        update_machine_devices(&mut machine, &bundle, &runtime.state().values)?;
        ticks.push(RocketTick {
            result,
            firmware_requests,
            applied_requests,
            transition,
        });
        frame_actions.clear();
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
        "tank-firmware hardware={} publication={} bundle={} firmware={} start=operator_once cycle_budget={}",
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
    let mut machine = Machine::new(
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
    .map_err(|error| error.to_string())?;
    let result = machine
        .run(cycle_budget)
        .map_err(|error| error.to_string())?;
    let status = match &result.status {
        MachineStatus::Running => "running".to_owned(),
        MachineStatus::Halted => "halted".to_owned(),
        MachineStatus::Trapped(trap) => format!("trapped:{trap}"),
    };
    println!(
        "status={status} steps={} cycles={} pc=0x{:08X} hi=0x{:08X} lo=0x{:08X}",
        result.steps,
        result.cycles,
        machine.pc(),
        machine.hi(),
        machine.lo()
    );
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
        println!("{registers}");
    }
    match result.status {
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

    use sentinel_core::assembler::assemble_with_symbols;
    use sentinel_scenario::{AllocationMode, Runtime, ScalarValue, compile, compile_hardware};

    use super::{
        execute_firmware_tick, execute_rocket_firmware, mmio_symbol, parse_cycle_budget,
        parse_tick_count, parse_word,
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
