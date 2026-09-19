use std::collections::BTreeMap;
use std::fmt;

use crate::{
    AluOperation, ImmediateOperation, Instruction, LoadKind, Register, ShiftOperation,
    SpecialRegister, StoreWidth,
};

const MAX_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_PROGRAM_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Assembly {
    pub origin: u32,
    pub entry: Option<u32>,
    pub bytes: Vec<u8>,
}

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

#[derive(Clone, Debug)]
struct Line {
    number: usize,
    column: usize,
    label: Option<String>,
    statement: Option<String>,
}

pub fn assemble(source: &str, origin: u32) -> Result<Assembly, Vec<Diagnostic>> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(vec![diag("ASM_SOURCE_LIMIT", 1, 1, "source exceeds 1 MiB")]);
    }
    if origin % 4 != 0 || origin > 0x000F_FFFF {
        return Err(vec![diag(
            "ASM_ORIGIN",
            1,
            1,
            "origin must be four-byte aligned in the S32 program region",
        )]);
    }

    let mut diagnostics = Vec::new();
    let lines = parse_lines(source, &mut diagnostics);
    let mut symbols = BTreeMap::new();
    let mut pc = u64::from(origin);
    for line in &lines {
        if let Some(label) = &line.label {
            if symbols.insert(label.clone(), pc as u32).is_some() {
                diagnostics.push(at(
                    line,
                    "ASM_DUPLICATE_LABEL",
                    format!("duplicate label `{label}`"),
                ));
            }
        }
        if let Some(statement) = &line.statement {
            match statement_size(statement, line) {
                Ok(size) => match pc.checked_add(size) {
                    Some(next)
                        if next - u64::from(origin) <= MAX_PROGRAM_BYTES && next <= 0x0010_0000 =>
                    {
                        pc = next
                    }
                    _ => diagnostics.push(at(
                        line,
                        "ASM_PROGRAM_LIMIT",
                        "program exceeds the S32 program region",
                    )),
                },
                Err(error) => diagnostics.push(error),
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let capacity = usize::try_from(pc - u64::from(origin)).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    let mut entry = None;
    pc = u64::from(origin);
    for line in &lines {
        let Some(statement) = &line.statement else {
            continue;
        };
        match emit_statement(statement, line, pc as u32, &symbols, &mut entry) {
            Ok(emitted) => {
                pc += emitted.len() as u64;
                bytes.extend_from_slice(&emitted);
            }
            Err(error) => diagnostics.push(error),
        }
    }
    if diagnostics.is_empty() {
        Ok(Assembly {
            origin,
            entry,
            bytes,
        })
    } else {
        Err(diagnostics)
    }
}

fn parse_lines(source: &str, diagnostics: &mut Vec<Diagnostic>) -> Vec<Line> {
    source
        .lines()
        .enumerate()
        .map(|(index, original)| {
            let number = index + 1;
            let comment = original.find(['#', ';']).unwrap_or(original.len());
            let text = &original[..comment];
            let leading = text.len() - text.trim_start().len();
            let mut body = text.trim();
            let mut label = None;
            if let Some(colon) = body.find(':') {
                let candidate = body[..colon].trim();
                if valid_identifier(candidate) {
                    label = Some(candidate.to_owned());
                } else {
                    diagnostics.push(diag("ASM_LABEL", number, leading + 1, "invalid label"));
                }
                body = body[colon + 1..].trim();
            }
            Line {
                number,
                column: leading + 1,
                label,
                statement: (!body.is_empty()).then(|| body.to_owned()),
            }
        })
        .collect()
}

fn valid_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z' | 'A'..='Z' | '_'))
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.'))
}

fn split_statement(statement: &str) -> (&str, &str) {
    statement
        .find(char::is_whitespace)
        .map_or((statement, ""), |index| {
            (&statement[..index], statement[index..].trim())
        })
}

fn statement_size(statement: &str, line: &Line) -> Result<u64, Diagnostic> {
    let (name, operands) = split_statement(statement);
    let upper = name.to_ascii_uppercase();
    match upper.as_str() {
        ".ENTRY" => Ok(0),
        ".WORD" => Ok(4),
        ".ZERO" => {
            let count =
                parse_integer(operands).map_err(|message| at(line, "ASM_LITERAL", message))?;
            if count < 0 || count % 4 != 0 {
                Err(at(
                    line,
                    "ASM_ALIGNMENT",
                    ".zero count must be a nonnegative multiple of four",
                ))
            } else {
                Ok(count as u64)
            }
        }
        "LI" | "LA" => Ok(8),
        name if known_instruction(name) => Ok(4),
        _ => Err(at(
            line,
            "ASM_UNKNOWN_MNEMONIC",
            format!("unknown mnemonic `{name}`"),
        )),
    }
}

fn known_instruction(name: &str) -> bool {
    matches!(
        name,
        "NOP"
            | "MOVE"
            | "B"
            | "RET"
            | "SLL"
            | "SRL"
            | "SRA"
            | "SLLV"
            | "SRLV"
            | "SRAV"
            | "JR"
            | "JALR"
            | "MFHI"
            | "MTHI"
            | "MFLO"
            | "MTLO"
            | "MULT"
            | "MULTU"
            | "DIV"
            | "DIVU"
            | "ADD"
            | "ADDU"
            | "SUB"
            | "SUBU"
            | "AND"
            | "OR"
            | "XOR"
            | "NOR"
            | "SLT"
            | "SLTU"
            | "BLTZ"
            | "BGEZ"
            | "J"
            | "JAL"
            | "BEQ"
            | "BNE"
            | "ADDI"
            | "ADDIU"
            | "SLTI"
            | "SLTIU"
            | "ANDI"
            | "ORI"
            | "XORI"
            | "LUI"
            | "LB"
            | "LH"
            | "LW"
            | "LBU"
            | "LHU"
            | "SB"
            | "SH"
            | "SW"
            | "HALT"
            | "TRAP"
    )
}

fn emit_statement(
    statement: &str,
    line: &Line,
    pc: u32,
    symbols: &BTreeMap<String, u32>,
    entry: &mut Option<u32>,
) -> Result<Vec<u8>, Diagnostic> {
    let (name, operand_text) = split_statement(statement);
    let name = name.to_ascii_uppercase();
    if name == ".ENTRY" {
        if entry.is_some() {
            return Err(at(line, "ASM_ENTRY", "duplicate .entry directive"));
        }
        let value = expression(operand_text, symbols)
            .map_err(|message| at(line, "ASM_EXPRESSION", message))?;
        let address = u32_value(value, line, "entry address")?;
        if address % 4 != 0 {
            return Err(at(line, "ASM_ALIGNMENT", "entry address is not aligned"));
        }
        *entry = Some(address);
        return Ok(Vec::new());
    }
    if name == ".WORD" {
        let value = expression(operand_text, symbols)
            .map_err(|message| at(line, "ASM_EXPRESSION", message))?;
        if !(-2_147_483_648..=4_294_967_295).contains(&value) {
            return Err(at(line, "ASM_RANGE", ".word value does not fit 32 bits"));
        }
        return Ok((value as u32).to_le_bytes().to_vec());
    }
    if name == ".ZERO" {
        let count =
            parse_integer(operand_text).map_err(|message| at(line, "ASM_LITERAL", message))?;
        return usize::try_from(count)
            .ok()
            .map(|count| vec![0; count])
            .ok_or_else(|| at(line, "ASM_RANGE", "invalid .zero count"));
    }

    let operands =
        split_operands(operand_text).map_err(|message| at(line, "ASM_SYNTAX", message))?;
    let instructions = build_instructions(&name, &operands, line, pc, symbols)?;
    let mut bytes = Vec::with_capacity(instructions.len() * 4);
    for instruction in instructions {
        let word = instruction
            .encode()
            .map_err(|error| at(line, "ASM_RANGE", error.to_string()))?;
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    Ok(bytes)
}

fn build_instructions(
    name: &str,
    operands: &[&str],
    line: &Line,
    pc: u32,
    symbols: &BTreeMap<String, u32>,
) -> Result<Vec<Instruction>, Diagnostic> {
    let count = |expected| operand_count(operands, expected, line);
    let r = |index| register(operands[index], line);
    let expr = |index| {
        expression(operands[index], symbols).map_err(|message| at(line, "ASM_EXPRESSION", message))
    };
    let one = |instruction| Ok(vec![instruction]);
    match name {
        "NOP" => {
            count(0)?;
            one(Instruction::ShiftImmediate {
                operation: ShiftOperation::Left,
                destination: Register::ZERO,
                value: Register::ZERO,
                amount: 0,
            })
        }
        "MOVE" => {
            count(2)?;
            one(Instruction::AluRegister {
                operation: AluOperation::AddUnsigned,
                destination: r(0)?,
                left: r(1)?,
                right: Register::ZERO,
            })
        }
        "B" => {
            count(1)?;
            one(Instruction::BranchEqual {
                equal: true,
                left: Register::ZERO,
                right: Register::ZERO,
                displacement: branch_displacement(operands[0], symbols, pc, line)?,
            })
        }
        "RET" => {
            count(0)?;
            one(Instruction::JumpRegister {
                target: Register::RA,
                link: None,
            })
        }
        "LI" | "LA" => {
            count(2)?;
            let target = r(0)?;
            let value = u32_value(expr(1)?, line, "literal/address")?;
            Ok(vec![
                Instruction::LoadUpperImmediate {
                    target,
                    immediate: (value >> 16) as u16,
                },
                Instruction::AluImmediate {
                    operation: ImmediateOperation::Or,
                    target,
                    source: target,
                    immediate_bits: value as u16,
                },
            ])
        }
        "SLL" | "SRL" | "SRA" => {
            count(3)?;
            one(Instruction::ShiftImmediate {
                operation: shift(name),
                destination: r(0)?,
                value: r(1)?,
                amount: shift_amount(expr(2)?, line)?,
            })
        }
        "SLLV" | "SRLV" | "SRAV" => {
            count(3)?;
            one(Instruction::ShiftVariable {
                operation: shift(name),
                destination: r(0)?,
                value: r(1)?,
                amount: r(2)?,
            })
        }
        "JR" => {
            count(1)?;
            one(Instruction::JumpRegister {
                target: r(0)?,
                link: None,
            })
        }
        "JALR" => {
            count(2)?;
            one(Instruction::JumpRegister {
                target: r(1)?,
                link: Some(r(0)?),
            })
        }
        "MFHI" | "MFLO" => {
            count(1)?;
            one(Instruction::MoveFrom {
                destination: r(0)?,
                source: special(name),
            })
        }
        "MTHI" | "MTLO" => {
            count(1)?;
            one(Instruction::MoveTo {
                destination: special(name),
                source: r(0)?,
            })
        }
        "MULT" | "MULTU" => {
            count(2)?;
            one(Instruction::Multiply {
                signed: name == "MULT",
                left: r(0)?,
                right: r(1)?,
            })
        }
        "DIV" | "DIVU" => {
            count(2)?;
            one(Instruction::Divide {
                signed: name == "DIV",
                dividend: r(0)?,
                divisor: r(1)?,
            })
        }
        "ADD" | "ADDU" | "SUB" | "SUBU" | "AND" | "OR" | "XOR" | "NOR" | "SLT" | "SLTU" => {
            count(3)?;
            one(Instruction::AluRegister {
                operation: alu(name),
                destination: r(0)?,
                left: r(1)?,
                right: r(2)?,
            })
        }
        "BLTZ" | "BGEZ" => {
            count(2)?;
            one(Instruction::BranchSign {
                non_negative: name == "BGEZ",
                value: r(0)?,
                displacement: branch_displacement(operands[1], symbols, pc, line)?,
            })
        }
        "J" | "JAL" => {
            count(1)?;
            one(Instruction::Jump {
                link: name == "JAL",
                target: jump_target(expr(0)?, pc, line)?,
            })
        }
        "BEQ" | "BNE" => {
            count(3)?;
            one(Instruction::BranchEqual {
                equal: name == "BEQ",
                left: r(0)?,
                right: r(1)?,
                displacement: branch_displacement(operands[2], symbols, pc, line)?,
            })
        }
        "ADDI" | "ADDIU" | "SLTI" | "SLTIU" | "ANDI" | "ORI" | "XORI" => {
            count(3)?;
            one(Instruction::AluImmediate {
                operation: immediate(name),
                target: r(0)?,
                source: r(1)?,
                immediate_bits: immediate_bits(
                    expr(2)?,
                    matches!(name, "ANDI" | "ORI" | "XORI"),
                    line,
                )?,
            })
        }
        "LUI" => {
            count(2)?;
            one(Instruction::LoadUpperImmediate {
                target: r(0)?,
                immediate: immediate_bits(expr(1)?, true, line)?,
            })
        }
        "LB" | "LH" | "LW" | "LBU" | "LHU" => {
            count(2)?;
            let (offset, base) = memory(operands[1], symbols, line)?;
            one(Instruction::Load {
                kind: load(name),
                target: r(0)?,
                base,
                offset,
            })
        }
        "SB" | "SH" | "SW" => {
            count(2)?;
            let (offset, base) = memory(operands[1], symbols, line)?;
            one(Instruction::Store {
                width: store(name),
                source: r(0)?,
                base,
                offset,
            })
        }
        "HALT" => {
            count(0)?;
            one(Instruction::Halt)
        }
        "TRAP" => {
            count(1)?;
            let value = expr(0)?;
            if !(0..=0x03FF_FFFF).contains(&value) {
                return Err(at(line, "ASM_RANGE", "trap code does not fit 26 bits"));
            }
            one(Instruction::Trap { code: value as u32 })
        }
        _ => Err(at(
            line,
            "ASM_UNKNOWN_MNEMONIC",
            format!("unknown mnemonic `{name}`"),
        )),
    }
}

fn split_operands(text: &str) -> Result<Vec<&str>, &'static str> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let mut start = 0;
    let mut depth = 0u8;
    for (index, ch) in text.char_indices() {
        match ch {
            '(' => {
                depth = depth
                    .checked_add(1)
                    .ok_or("parenthesis nesting is too deep")?
            }
            ')' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or("unmatched closing parenthesis")?
            }
            ',' if depth == 0 => {
                let value = text[start..index].trim();
                if value.is_empty() {
                    return Err("empty operand");
                }
                result.push(value);
                start = index + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err("unclosed parenthesis");
    }
    let value = text[start..].trim();
    if value.is_empty() {
        return Err("empty operand");
    }
    result.push(value);
    Ok(result)
}

fn operand_count(operands: &[&str], expected: usize, line: &Line) -> Result<(), Diagnostic> {
    if operands.len() == expected {
        Ok(())
    } else {
        Err(at(
            line,
            "ASM_OPERAND_COUNT",
            format!("expected {expected} operands, found {}", operands.len()),
        ))
    }
}

fn register(value: &str, line: &Line) -> Result<Register, Diagnostic> {
    let upper = value.to_ascii_uppercase();
    let index = match upper.as_str() {
        "SP" => 29,
        "FP" => 30,
        "RA" => 31,
        _ => upper
            .strip_prefix('R')
            .and_then(|digits| digits.parse::<u8>().ok())
            .ok_or_else(|| at(line, "ASM_REGISTER", format!("invalid register `{value}`")))?,
    };
    Register::new(index).ok_or_else(|| {
        at(
            line,
            "ASM_REGISTER",
            format!("register out of range `{value}`"),
        )
    })
}

fn expression(text: &str, symbols: &BTreeMap<String, u32>) -> Result<i64, String> {
    let text = text.trim();
    if let Some(inner) = text.strip_prefix("hi16(").and_then(|v| v.strip_suffix(')')) {
        return expression(inner, symbols).map(|value| ((value as u64 >> 16) & 0xFFFF) as i64);
    }
    if let Some(inner) = text.strip_prefix("lo16(").and_then(|v| v.strip_suffix(')')) {
        return expression(inner, symbols).map(|value| (value as u64 & 0xFFFF) as i64);
    }
    if let Ok(value) = parse_integer(text) {
        return Ok(value);
    }
    let split = text
        .char_indices()
        .skip(1)
        .find(|(_, ch)| matches!(ch, '+' | '-'));
    let (name, addend) = split.map_or((text, 0), |(index, op)| {
        let raw = parse_integer(text[index + 1..].trim()).unwrap_or(i64::MIN);
        (
            &text[..index],
            if op == '-' { raw.saturating_neg() } else { raw },
        )
    });
    if !valid_identifier(name.trim()) {
        return Err(format!("invalid expression `{text}`"));
    }
    if addend == i64::MIN {
        return Err(format!("invalid addend in `{text}`"));
    }
    let base = symbols
        .get(name.trim())
        .ok_or_else(|| format!("unknown symbol `{}`", name.trim()))?;
    i64::from(*base)
        .checked_add(addend)
        .ok_or_else(|| format!("expression overflow `{text}`"))
}

fn parse_integer(text: &str) -> Result<i64, String> {
    let compact = text.trim().replace('_', "");
    if compact.is_empty() {
        return Err("expected integer".to_owned());
    }
    let (negative, unsigned) = compact
        .strip_prefix('-')
        .map_or((false, compact.as_str()), |v| (true, v));
    let unsigned = unsigned.strip_prefix('+').unwrap_or(unsigned);
    let (digits, radix) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
        .map_or_else(
            || {
                unsigned
                    .strip_prefix("0b")
                    .or_else(|| unsigned.strip_prefix("0B"))
                    .map_or((unsigned, 10), |v| (v, 2))
            },
            |v| (v, 16),
        );
    let magnitude =
        i64::from_str_radix(digits, radix).map_err(|_| format!("invalid integer `{text}`"))?;
    if negative {
        magnitude
            .checked_neg()
            .ok_or_else(|| format!("integer overflow `{text}`"))
    } else {
        Ok(magnitude)
    }
}

fn branch_displacement(
    text: &str,
    symbols: &BTreeMap<String, u32>,
    pc: u32,
    line: &Line,
) -> Result<i16, Diagnostic> {
    if let Ok(value) = parse_integer(text) {
        return i16::try_from(value)
            .map_err(|_| at(line, "ASM_RANGE", "branch displacement does not fit i16"));
    }
    let target =
        expression(text, symbols).map_err(|message| at(line, "ASM_EXPRESSION", message))?;
    let next = i64::from(pc)
        .checked_add(4)
        .ok_or_else(|| at(line, "ASM_RANGE", "PC overflow"))?;
    let delta = target - next;
    if delta % 4 != 0 {
        return Err(at(
            line,
            "ASM_ALIGNMENT",
            "branch target is not four-byte aligned",
        ));
    }
    i16::try_from(delta / 4).map_err(|_| at(line, "ASM_RANGE", "branch target is out of range"))
}

fn jump_target(value: i64, pc: u32, line: &Line) -> Result<u32, Diagnostic> {
    let address = u32_value(value, line, "jump target")?;
    if address % 4 != 0 {
        return Err(at(line, "ASM_ALIGNMENT", "jump target is not aligned"));
    }
    let next = pc
        .checked_add(4)
        .ok_or_else(|| at(line, "ASM_RANGE", "PC overflow"))?;
    if address & 0xF000_0000 != next & 0xF000_0000 {
        return Err(at(
            line,
            "ASM_RANGE",
            "jump target is outside the current 256 MiB region",
        ));
    }
    Ok((address >> 2) & 0x03FF_FFFF)
}

fn memory(
    text: &str,
    symbols: &BTreeMap<String, u32>,
    line: &Line,
) -> Result<(i16, Register), Diagnostic> {
    let open = text
        .rfind('(')
        .ok_or_else(|| at(line, "ASM_MEMORY", "expected offset(base)"))?;
    let inner = text[open + 1..]
        .strip_suffix(')')
        .ok_or_else(|| at(line, "ASM_MEMORY", "expected closing parenthesis"))?;
    let offset = expression(text[..open].trim(), symbols)
        .map_err(|message| at(line, "ASM_EXPRESSION", message))?;
    Ok((
        i16::try_from(offset)
            .map_err(|_| at(line, "ASM_RANGE", "memory offset does not fit i16"))?,
        register(inner.trim(), line)?,
    ))
}

fn u32_value(value: i64, line: &Line, name: &str) -> Result<u32, Diagnostic> {
    if (-2_147_483_648..=4_294_967_295).contains(&value) {
        Ok(value as u32)
    } else {
        Err(at(
            line,
            "ASM_RANGE",
            format!("{name} does not fit 32 bits"),
        ))
    }
}
fn shift_amount(value: i64, line: &Line) -> Result<u8, Diagnostic> {
    if (0..=31).contains(&value) {
        Ok(value as u8)
    } else {
        Err(at(line, "ASM_RANGE", "shift amount must be 0..31"))
    }
}
fn immediate_bits(value: i64, unsigned_only: bool, line: &Line) -> Result<u16, Diagnostic> {
    let min = if unsigned_only { 0 } else { -32768 };
    if (min..=65535).contains(&value) {
        Ok(value as u16)
    } else {
        Err(at(line, "ASM_RANGE", "immediate does not fit 16 bits"))
    }
}

fn shift(name: &str) -> ShiftOperation {
    if name.contains("SRA") {
        ShiftOperation::RightArithmetic
    } else if name.contains("SRL") {
        ShiftOperation::RightLogical
    } else {
        ShiftOperation::Left
    }
}
fn special(name: &str) -> SpecialRegister {
    if name.ends_with("HI") {
        SpecialRegister::Hi
    } else {
        SpecialRegister::Lo
    }
}
fn alu(name: &str) -> AluOperation {
    match name {
        "ADD" => AluOperation::Add,
        "ADDU" => AluOperation::AddUnsigned,
        "SUB" => AluOperation::Subtract,
        "SUBU" => AluOperation::SubtractUnsigned,
        "AND" => AluOperation::And,
        "OR" => AluOperation::Or,
        "XOR" => AluOperation::Xor,
        "NOR" => AluOperation::Nor,
        "SLT" => AluOperation::SetLessThan,
        _ => AluOperation::SetLessThanUnsigned,
    }
}
fn immediate(name: &str) -> ImmediateOperation {
    match name {
        "ADDI" => ImmediateOperation::Add,
        "ADDIU" => ImmediateOperation::AddUnsigned,
        "SLTI" => ImmediateOperation::SetLessThan,
        "SLTIU" => ImmediateOperation::SetLessThanUnsigned,
        "ANDI" => ImmediateOperation::And,
        "ORI" => ImmediateOperation::Or,
        _ => ImmediateOperation::Xor,
    }
}
fn load(name: &str) -> LoadKind {
    match name {
        "LB" => LoadKind::Byte,
        "LH" => LoadKind::Halfword,
        "LW" => LoadKind::Word,
        "LBU" => LoadKind::ByteUnsigned,
        _ => LoadKind::HalfwordUnsigned,
    }
}
fn store(name: &str) -> StoreWidth {
    match name {
        "SB" => StoreWidth::Byte,
        "SH" => StoreWidth::Halfword,
        _ => StoreWidth::Word,
    }
}

fn at(line: &Line, code: &'static str, message: impl Into<String>) -> Diagnostic {
    diag(code, line.number, line.column, message)
}
fn diag(code: &'static str, line: usize, column: usize, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        code,
        line,
        column,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::assemble;

    #[test]
    fn assembles_labels_directives_and_pseudo_instructions() {
        let source = ".entry start\nstart:\n  li r1, 0x12345678\n  addi r2, r0, 3\nloop: addiu r2, r2, -1\n  bne r2, r0, loop\n  halt\n";
        let result = assemble(source, 0).expect("valid assembly");
        assert_eq!(result.entry, Some(0));
        let words: Vec<u32> = result
            .bytes
            .chunks_exact(4)
            .map(|v| u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
            .collect();
        assert_eq!(
            words,
            [
                0x3C01_1234,
                0x3421_5678,
                0x2002_0003,
                0x2442_FFFF,
                0x1440_FFFE,
                0xF800_0000
            ]
        );
    }

    #[test]
    fn supports_comments_aliases_memory_and_data() {
        let source = "move fp, sp ; alias test\nlw r2, -4(fp)\n.word 0x12345678\n.zero 4\nret\n";
        let result = assemble(source, 0x100).expect("valid assembly");
        assert_eq!(result.bytes.len(), 20);
        assert_eq!(&result.bytes[8..12], &[0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn reports_stable_source_diagnostics() {
        let errors =
            assemble("same: nop\nsame: sll r1, r2, 32\nwat r1\n", 0).expect_err("invalid assembly");
        assert!(
            errors
                .iter()
                .any(|error| error.code == "ASM_DUPLICATE_LABEL")
        );
        assert!(
            errors
                .iter()
                .any(|error| error.code == "ASM_UNKNOWN_MNEMONIC")
        );
    }

    #[test]
    fn rejects_unknown_symbols_without_panicking() {
        let errors = assemble("beq r0, r0, missing\n", 0).expect_err("invalid assembly");
        assert_eq!(errors[0].code, "ASM_EXPRESSION");
    }

    #[test]
    fn assembles_every_golden_instruction() {
        let source = "\
sll r3,r2,4\n\
srl r3,r2,4\n\
sra r3,r2,4\n\
sllv r3,r2,r1\n\
srlv r3,r2,r1\n\
srav r3,r2,r1\n\
jr r1\n\
jalr r3,r1\n\
mfhi r3\n\
mthi r1\n\
mflo r3\n\
mtlo r1\n\
mult r1,r2\n\
multu r1,r2\n\
div r1,r2\n\
divu r1,r2\n\
add r3,r1,r2\n\
addu r3,r1,r2\n\
sub r3,r1,r2\n\
subu r3,r1,r2\n\
and r3,r1,r2\n\
or r3,r1,r2\n\
xor r3,r1,r2\n\
nor r3,r1,r2\n\
slt r3,r1,r2\n\
sltu r3,r1,r2\n\
bltz r1,+1\n\
bgez r1,+1\n\
j 0x00000010\n\
jal 0x00000010\n\
beq r1,r2,+1\n\
bne r1,r2,+1\n\
addi r2,r1,-1\n\
addiu r2,r1,-1\n\
slti r2,r1,-1\n\
sltiu r2,r1,-1\n\
andi r2,r1,0xff\n\
ori r2,r1,0xff\n\
xori r2,r1,0xff\n\
lui r2,0x1234\n\
lb r2,4(r1)\n\
lh r2,4(r1)\n\
lw r2,4(r1)\n\
lbu r2,4(r1)\n\
lhu r2,4(r1)\n\
sb r2,4(r1)\n\
sh r2,4(r1)\n\
sw r2,4(r1)\n\
halt\n\
trap 1\n";
        let expected = [
            0x0002_1900,
            0x0002_1902,
            0x0002_1903,
            0x0022_1804,
            0x0022_1806,
            0x0022_1807,
            0x0020_0008,
            0x0020_1809,
            0x0000_1810,
            0x0020_0011,
            0x0000_1812,
            0x0020_0013,
            0x0022_0018,
            0x0022_0019,
            0x0022_001A,
            0x0022_001B,
            0x0022_1820,
            0x0022_1821,
            0x0022_1822,
            0x0022_1823,
            0x0022_1824,
            0x0022_1825,
            0x0022_1826,
            0x0022_1827,
            0x0022_182A,
            0x0022_182B,
            0x0420_0001,
            0x0421_0001,
            0x0800_0004,
            0x0C00_0004,
            0x1022_0001,
            0x1422_0001,
            0x2022_FFFF,
            0x2422_FFFF,
            0x2822_FFFF,
            0x2C22_FFFF,
            0x3022_00FF,
            0x3422_00FF,
            0x3822_00FF,
            0x3C02_1234,
            0x8022_0004,
            0x8422_0004,
            0x8C22_0004,
            0x9022_0004,
            0x9422_0004,
            0xA022_0004,
            0xA422_0004,
            0xAC22_0004,
            0xF800_0000,
            0xFC00_0001,
        ];
        let result = assemble(source, 0).expect("golden assembly");
        let words: Vec<u32> = result
            .bytes
            .chunks_exact(4)
            .map(|value| u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
            .collect();
        assert_eq!(words, expected);
    }

    #[test]
    fn resolves_symbol_arithmetic_and_half_selectors() {
        let source = "base: nop\n.word base+4\n.word hi16(0x12345678)\n.word lo16(0x12345678)\n";
        let result = assemble(source, 0x100).expect("valid expressions");
        let words: Vec<u32> = result
            .bytes
            .chunks_exact(4)
            .map(|value| u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
            .collect();
        assert_eq!(words, [0, 0x104, 0x1234, 0x5678]);
    }
}
