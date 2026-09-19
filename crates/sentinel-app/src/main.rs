use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use sentinel_core::assembler::{Diagnostic, assemble};
use sentinel_core::{ISA_ID, decode};

fn parse_word(value: &str) -> Result<u32, String> {
    let (digits, radix) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map_or((value, 10), |digits| (digits, 16));
    u32::from_str_radix(digits, radix).map_err(|_| format!("invalid 32-bit word: {value}"))
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
        _ => Err(
            "usage: sentinel-app [decode <u32|0xHEX> | check <source.s32> | assemble <source.s32> [output.bin]]"
                .to_owned(),
        ),
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
    use super::parse_word;

    #[test]
    fn parses_decimal_and_hex_words() {
        assert_eq!(parse_word("42"), Ok(42));
        assert_eq!(parse_word("0xFC000001"), Ok(0xFC00_0001));
        assert!(parse_word("0x100000000").is_err());
    }
}
