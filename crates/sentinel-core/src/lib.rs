#![forbid(unsafe_code)]

use core::fmt;

pub mod assembler;

pub const ISA_ID: &str = "s32-isa-v0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Register(u8);

impl Register {
    pub const ZERO: Self = Self(0);
    pub const SP: Self = Self(29);
    pub const FP: Self = Self(30);
    pub const RA: Self = Self(31);

    pub const fn new(index: u8) -> Option<Self> {
        if index < 32 { Some(Self(index)) } else { None }
    }

    pub const fn index(self) -> u8 {
        self.0
    }

    const fn from_field(index: u8) -> Self {
        Self(index)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShiftOperation {
    Left,
    RightLogical,
    RightArithmetic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AluOperation {
    Add,
    AddUnsigned,
    Subtract,
    SubtractUnsigned,
    And,
    Or,
    Xor,
    Nor,
    SetLessThan,
    SetLessThanUnsigned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmediateOperation {
    Add,
    AddUnsigned,
    SetLessThan,
    SetLessThanUnsigned,
    And,
    Or,
    Xor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpecialRegister {
    Hi,
    Lo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadKind {
    Byte,
    Halfword,
    Word,
    ByteUnsigned,
    HalfwordUnsigned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreWidth {
    Byte,
    Halfword,
    Word,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Instruction {
    ShiftImmediate {
        operation: ShiftOperation,
        destination: Register,
        value: Register,
        amount: u8,
    },
    ShiftVariable {
        operation: ShiftOperation,
        destination: Register,
        value: Register,
        amount: Register,
    },
    JumpRegister {
        target: Register,
        link: Option<Register>,
    },
    MoveFrom {
        destination: Register,
        source: SpecialRegister,
    },
    MoveTo {
        destination: SpecialRegister,
        source: Register,
    },
    Multiply {
        signed: bool,
        left: Register,
        right: Register,
    },
    Divide {
        signed: bool,
        dividend: Register,
        divisor: Register,
    },
    AluRegister {
        operation: AluOperation,
        destination: Register,
        left: Register,
        right: Register,
    },
    BranchSign {
        non_negative: bool,
        value: Register,
        displacement: i16,
    },
    Jump {
        link: bool,
        target: u32,
    },
    BranchEqual {
        equal: bool,
        left: Register,
        right: Register,
        displacement: i16,
    },
    AluImmediate {
        operation: ImmediateOperation,
        target: Register,
        source: Register,
        immediate_bits: u16,
    },
    LoadUpperImmediate {
        target: Register,
        immediate: u16,
    },
    Load {
        kind: LoadKind,
        target: Register,
        base: Register,
        offset: i16,
    },
    Store {
        width: StoreWidth,
        source: Register,
        base: Register,
        offset: i16,
    },
    Halt,
    Trap {
        code: u32,
    },
}

impl Instruction {
    pub const fn cycle_cost(self) -> u8 {
        match self {
            Self::Load { .. } | Self::Store { .. } => 2,
            Self::Multiply { .. } => 4,
            Self::Divide { .. } => 12,
            _ => 1,
        }
    }

    pub const fn encode(self) -> Result<u32, EncodeError> {
        Ok(match self {
            Self::ShiftImmediate {
                operation,
                destination,
                value,
                amount,
            } => {
                if amount > 31 {
                    return Err(EncodeError {
                        kind: EncodeErrorKind::ShiftAmount,
                        value: amount as u32,
                    });
                }
                r(
                    Register::ZERO,
                    value,
                    destination,
                    amount,
                    shift_funct(operation, false),
                )
            }
            Self::ShiftVariable {
                operation,
                destination,
                value,
                amount,
            } => r(amount, value, destination, 0, shift_funct(operation, true)),
            Self::JumpRegister { target, link: None } => {
                r(target, Register::ZERO, Register::ZERO, 0, 0x08)
            }
            Self::JumpRegister {
                target,
                link: Some(link),
            } => r(target, Register::ZERO, link, 0, 0x09),
            Self::MoveFrom {
                destination,
                source,
            } => r(
                Register::ZERO,
                Register::ZERO,
                destination,
                0,
                match source {
                    SpecialRegister::Hi => 0x10,
                    SpecialRegister::Lo => 0x12,
                },
            ),
            Self::MoveTo {
                destination,
                source,
            } => r(
                source,
                Register::ZERO,
                Register::ZERO,
                0,
                match destination {
                    SpecialRegister::Hi => 0x11,
                    SpecialRegister::Lo => 0x13,
                },
            ),
            Self::Multiply {
                signed,
                left,
                right,
            } => r(
                left,
                right,
                Register::ZERO,
                0,
                if signed { 0x18 } else { 0x19 },
            ),
            Self::Divide {
                signed,
                dividend,
                divisor,
            } => r(
                dividend,
                divisor,
                Register::ZERO,
                0,
                if signed { 0x1A } else { 0x1B },
            ),
            Self::AluRegister {
                operation,
                destination,
                left,
                right,
            } => r(left, right, destination, 0, alu_funct(operation)),
            Self::BranchSign {
                non_negative,
                value,
                displacement,
            } => i(
                0x01,
                value,
                Register::from_field(if non_negative { 1 } else { 0 }),
                displacement as u16,
            ),
            Self::Jump { link, target } => {
                if target > 0x03FF_FFFF {
                    return Err(EncodeError {
                        kind: EncodeErrorKind::JumpTarget,
                        value: target,
                    });
                }
                j(if link { 0x03 } else { 0x02 }, target)
            }
            Self::BranchEqual {
                equal,
                left,
                right,
                displacement,
            } => i(
                if equal { 0x04 } else { 0x05 },
                left,
                right,
                displacement as u16,
            ),
            Self::AluImmediate {
                operation,
                target,
                source,
                immediate_bits,
            } => i(immediate_opcode(operation), source, target, immediate_bits),
            Self::LoadUpperImmediate { target, immediate } => {
                i(0x0F, Register::ZERO, target, immediate)
            }
            Self::Load {
                kind,
                target,
                base,
                offset,
            } => i(load_opcode(kind), base, target, offset as u16),
            Self::Store {
                width,
                source,
                base,
                offset,
            } => i(store_opcode(width), base, source, offset as u16),
            Self::Halt => 0xF800_0000,
            Self::Trap { code } => {
                if code > 0x03FF_FFFF {
                    return Err(EncodeError {
                        kind: EncodeErrorKind::TrapCode,
                        value: code,
                    });
                }
                0xFC00_0000 | code
            }
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncodeErrorKind {
    ShiftAmount,
    JumpTarget,
    TrapCode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EncodeError {
    pub kind: EncodeErrorKind,
    pub value: u32,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} does not fit: {}",
            match self.kind {
                EncodeErrorKind::ShiftAmount => "shift amount",
                EncodeErrorKind::JumpTarget => "jump target",
                EncodeErrorKind::TrapCode => "trap code",
            },
            self.value
        )
    }
}

impl std::error::Error for EncodeError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeErrorKind {
    IllegalInstruction,
    ReservedEncoding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeError {
    pub kind: DecodeErrorKind,
    pub word: u32,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: 0x{:08X}",
            match self.kind {
                DecodeErrorKind::IllegalInstruction => "illegal_instruction",
                DecodeErrorKind::ReservedEncoding => "reserved_encoding",
            },
            self.word
        )
    }
}

impl std::error::Error for DecodeError {}

pub fn decode(word: u32) -> Result<Instruction, DecodeError> {
    let opcode = (word >> 26) as u8;
    let rs = Register::from_field(((word >> 21) & 0x1F) as u8);
    let rt = Register::from_field(((word >> 16) & 0x1F) as u8);
    let rd = Register::from_field(((word >> 11) & 0x1F) as u8);
    let shamt = ((word >> 6) & 0x1F) as u8;
    let immediate = word as u16;
    let signed_immediate = immediate as i16;
    match opcode {
        0x00 => decode_r(word, rs, rt, rd, shamt, (word & 0x3F) as u8),
        0x01 => match rt.index() {
            0 | 1 => Ok(Instruction::BranchSign {
                non_negative: rt.index() == 1,
                value: rs,
                displacement: signed_immediate,
            }),
            _ => illegal(word),
        },
        0x02 | 0x03 => Ok(Instruction::Jump {
            link: opcode == 0x03,
            target: word & 0x03FF_FFFF,
        }),
        0x04 | 0x05 => Ok(Instruction::BranchEqual {
            equal: opcode == 0x04,
            left: rs,
            right: rt,
            displacement: signed_immediate,
        }),
        0x08..=0x0E => Ok(Instruction::AluImmediate {
            operation: immediate_operation(opcode),
            target: rt,
            source: rs,
            immediate_bits: immediate,
        }),
        0x0F if rs == Register::ZERO => Ok(Instruction::LoadUpperImmediate {
            target: rt,
            immediate,
        }),
        0x0F => reserved(word),
        0x20 | 0x21 | 0x23..=0x25 => Ok(Instruction::Load {
            kind: load_kind(opcode),
            target: rt,
            base: rs,
            offset: signed_immediate,
        }),
        0x28 | 0x29 | 0x2B => Ok(Instruction::Store {
            width: store_width(opcode),
            source: rt,
            base: rs,
            offset: signed_immediate,
        }),
        0x3E if word & 0x03FF_FFFF == 0 => Ok(Instruction::Halt),
        0x3E => reserved(word),
        0x3F => Ok(Instruction::Trap {
            code: word & 0x03FF_FFFF,
        }),
        _ => illegal(word),
    }
}

fn decode_r(
    word: u32,
    rs: Register,
    rt: Register,
    rd: Register,
    shamt: u8,
    funct: u8,
) -> Result<Instruction, DecodeError> {
    let zero = Register::ZERO;
    match funct {
        0x00 | 0x02 | 0x03 if rs == zero => Ok(Instruction::ShiftImmediate {
            operation: shift_operation(funct),
            destination: rd,
            value: rt,
            amount: shamt,
        }),
        0x00 | 0x02 | 0x03 => reserved(word),
        0x04 | 0x06 | 0x07 if shamt == 0 => Ok(Instruction::ShiftVariable {
            operation: shift_operation(funct),
            destination: rd,
            value: rt,
            amount: rs,
        }),
        0x04 | 0x06 | 0x07 => reserved(word),
        0x08 if rt == zero && rd == zero && shamt == 0 => Ok(Instruction::JumpRegister {
            target: rs,
            link: None,
        }),
        0x08 => reserved(word),
        0x09 if rt == zero && shamt == 0 => Ok(Instruction::JumpRegister {
            target: rs,
            link: Some(rd),
        }),
        0x09 => reserved(word),
        0x10 | 0x12 if rs == zero && rt == zero && shamt == 0 => Ok(Instruction::MoveFrom {
            destination: rd,
            source: if funct == 0x10 {
                SpecialRegister::Hi
            } else {
                SpecialRegister::Lo
            },
        }),
        0x10 | 0x12 => reserved(word),
        0x11 | 0x13 if rt == zero && rd == zero && shamt == 0 => Ok(Instruction::MoveTo {
            destination: if funct == 0x11 {
                SpecialRegister::Hi
            } else {
                SpecialRegister::Lo
            },
            source: rs,
        }),
        0x11 | 0x13 => reserved(word),
        0x18 | 0x19 if rd == zero && shamt == 0 => Ok(Instruction::Multiply {
            signed: funct == 0x18,
            left: rs,
            right: rt,
        }),
        0x18 | 0x19 => reserved(word),
        0x1A | 0x1B if rd == zero && shamt == 0 => Ok(Instruction::Divide {
            signed: funct == 0x1A,
            dividend: rs,
            divisor: rt,
        }),
        0x1A | 0x1B => reserved(word),
        0x20..=0x27 | 0x2A | 0x2B if shamt == 0 => Ok(Instruction::AluRegister {
            operation: alu_operation(funct),
            destination: rd,
            left: rs,
            right: rt,
        }),
        0x20..=0x27 | 0x2A | 0x2B => reserved(word),
        _ => illegal(word),
    }
}

fn illegal<T>(word: u32) -> Result<T, DecodeError> {
    Err(DecodeError {
        kind: DecodeErrorKind::IllegalInstruction,
        word,
    })
}
fn reserved<T>(word: u32) -> Result<T, DecodeError> {
    Err(DecodeError {
        kind: DecodeErrorKind::ReservedEncoding,
        word,
    })
}

const fn r(rs: Register, rt: Register, rd: Register, shamt: u8, funct: u8) -> u32 {
    ((rs.index() as u32) << 21)
        | ((rt.index() as u32) << 16)
        | ((rd.index() as u32) << 11)
        | ((shamt as u32) << 6)
        | funct as u32
}
const fn i(opcode: u8, rs: Register, rt: Register, immediate: u16) -> u32 {
    ((opcode as u32) << 26)
        | ((rs.index() as u32) << 21)
        | ((rt.index() as u32) << 16)
        | immediate as u32
}
const fn j(opcode: u8, target: u32) -> u32 {
    ((opcode as u32) << 26) | (target & 0x03FF_FFFF)
}

const fn shift_operation(funct: u8) -> ShiftOperation {
    match funct {
        0x00 | 0x04 => ShiftOperation::Left,
        0x02 | 0x06 => ShiftOperation::RightLogical,
        _ => ShiftOperation::RightArithmetic,
    }
}
const fn shift_funct(operation: ShiftOperation, variable: bool) -> u8 {
    match (operation, variable) {
        (ShiftOperation::Left, false) => 0x00,
        (ShiftOperation::RightLogical, false) => 0x02,
        (ShiftOperation::RightArithmetic, false) => 0x03,
        (ShiftOperation::Left, true) => 0x04,
        (ShiftOperation::RightLogical, true) => 0x06,
        (ShiftOperation::RightArithmetic, true) => 0x07,
    }
}
const fn alu_operation(funct: u8) -> AluOperation {
    match funct {
        0x20 => AluOperation::Add,
        0x21 => AluOperation::AddUnsigned,
        0x22 => AluOperation::Subtract,
        0x23 => AluOperation::SubtractUnsigned,
        0x24 => AluOperation::And,
        0x25 => AluOperation::Or,
        0x26 => AluOperation::Xor,
        0x27 => AluOperation::Nor,
        0x2A => AluOperation::SetLessThan,
        _ => AluOperation::SetLessThanUnsigned,
    }
}
const fn alu_funct(operation: AluOperation) -> u8 {
    match operation {
        AluOperation::Add => 0x20,
        AluOperation::AddUnsigned => 0x21,
        AluOperation::Subtract => 0x22,
        AluOperation::SubtractUnsigned => 0x23,
        AluOperation::And => 0x24,
        AluOperation::Or => 0x25,
        AluOperation::Xor => 0x26,
        AluOperation::Nor => 0x27,
        AluOperation::SetLessThan => 0x2A,
        AluOperation::SetLessThanUnsigned => 0x2B,
    }
}
const fn immediate_operation(opcode: u8) -> ImmediateOperation {
    match opcode {
        0x08 => ImmediateOperation::Add,
        0x09 => ImmediateOperation::AddUnsigned,
        0x0A => ImmediateOperation::SetLessThan,
        0x0B => ImmediateOperation::SetLessThanUnsigned,
        0x0C => ImmediateOperation::And,
        0x0D => ImmediateOperation::Or,
        _ => ImmediateOperation::Xor,
    }
}
const fn immediate_opcode(operation: ImmediateOperation) -> u8 {
    match operation {
        ImmediateOperation::Add => 0x08,
        ImmediateOperation::AddUnsigned => 0x09,
        ImmediateOperation::SetLessThan => 0x0A,
        ImmediateOperation::SetLessThanUnsigned => 0x0B,
        ImmediateOperation::And => 0x0C,
        ImmediateOperation::Or => 0x0D,
        ImmediateOperation::Xor => 0x0E,
    }
}
const fn load_kind(opcode: u8) -> LoadKind {
    match opcode {
        0x20 => LoadKind::Byte,
        0x21 => LoadKind::Halfword,
        0x23 => LoadKind::Word,
        0x24 => LoadKind::ByteUnsigned,
        _ => LoadKind::HalfwordUnsigned,
    }
}
const fn load_opcode(kind: LoadKind) -> u8 {
    match kind {
        LoadKind::Byte => 0x20,
        LoadKind::Halfword => 0x21,
        LoadKind::Word => 0x23,
        LoadKind::ByteUnsigned => 0x24,
        LoadKind::HalfwordUnsigned => 0x25,
    }
}
const fn store_width(opcode: u8) -> StoreWidth {
    match opcode {
        0x28 => StoreWidth::Byte,
        0x29 => StoreWidth::Halfword,
        _ => StoreWidth::Word,
    }
}
const fn store_opcode(width: StoreWidth) -> u8 {
    match width {
        StoreWidth::Byte => 0x28,
        StoreWidth::Halfword => 0x29,
        StoreWidth::Word => 0x2B,
    }
}

#[cfg(test)]
mod tests {
    use super::{DecodeErrorKind, Instruction, decode};

    const GOLDEN_WORDS: &[u32] = &[
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

    #[test]
    fn all_golden_words_decode_and_reencode_exactly() {
        for &word in GOLDEN_WORDS {
            let instruction = decode(word).unwrap_or_else(|error| panic!("{error}"));
            assert_eq!(instruction.encode(), Ok(word), "{instruction:?}");
        }
    }

    #[test]
    fn reserved_fields_are_rejected() {
        for word in [
            0x0022_1900,
            0x0021_0008,
            0x0022_1860,
            0x3C22_1234,
            0xF800_0001,
        ] {
            assert_eq!(
                decode(word).map_err(|error| error.kind),
                Err(DecodeErrorKind::ReservedEncoding)
            );
        }
    }

    #[test]
    fn unknown_encodings_are_illegal() {
        for word in [0x0000_0001, 0x0442_0000, 0x4800_0000] {
            assert_eq!(
                decode(word).map_err(|error| error.kind),
                Err(DecodeErrorKind::IllegalInstruction)
            );
        }
    }

    #[test]
    fn architectural_cycle_costs_are_exposed() {
        assert_eq!(decode(0x8C22_0004).map(Instruction::cycle_cost), Ok(2));
        assert_eq!(decode(0x0022_0018).map(Instruction::cycle_cost), Ok(4));
        assert_eq!(decode(0x0022_001A).map(Instruction::cycle_cost), Ok(12));
        assert_eq!(decode(0x0022_1820).map(Instruction::cycle_cost), Ok(1));
    }

    #[test]
    fn representative_arbitrary_words_never_panic() {
        for low in 0..=u16::MAX {
            let _ = decode(u32::from(low));
            let _ = decode(0xFFFF_0000 | u32::from(low));
        }
    }

    #[test]
    fn oversized_encoded_fields_fail_instead_of_truncating() {
        use super::{EncodeErrorKind, Register, ShiftOperation};

        let cases = [
            (
                Instruction::ShiftImmediate {
                    operation: ShiftOperation::Left,
                    destination: Register::ZERO,
                    value: Register::ZERO,
                    amount: 32,
                },
                EncodeErrorKind::ShiftAmount,
            ),
            (
                Instruction::Jump {
                    link: false,
                    target: 0x0400_0000,
                },
                EncodeErrorKind::JumpTarget,
            ),
            (
                Instruction::Trap { code: 0x0400_0000 },
                EncodeErrorKind::TrapCode,
            ),
        ];
        for (instruction, expected) in cases {
            assert_eq!(
                instruction.encode().map_err(|error| error.kind),
                Err(expected)
            );
        }
    }
}
