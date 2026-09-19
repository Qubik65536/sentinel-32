use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use sentinel_core::assembler::{Diagnostic, assemble};
use sentinel_core::vm::{
    Capability, ImageManifest, Machine, MachineStatus, MemorySlot, Permissions,
};
use sentinel_core::{ISA_ID, Register, decode};

const LAB_STACK_BASE: u32 = 0x2000_0000;
const LAB_STACK_SIZE: usize = 64 * 1024;

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
        _ => Err(
            "usage: sentinel-app [decode <u32|0xHEX> | check <source.s32> | assemble <source.s32> [output.bin] | run <source.s32> <cycle-budget>]"
                .to_owned(),
        ),
    }
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
    let source =
        fs::read_to_string(path).map_err(|error| format!("cannot read `{path}`: {error}"))?;
    assemble(&source, 0).map_err(|diagnostics| render_diagnostics(Path::new(path), &diagnostics))
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
    use super::{parse_cycle_budget, parse_word};

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
}
