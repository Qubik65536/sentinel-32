use core::fmt;

use crate::{
    AluOperation, DecodeErrorKind, ImmediateOperation, Instruction, LoadKind, Register,
    ShiftOperation, SpecialRegister, StoreWidth, decode,
};

const PROGRAM_START: u32 = 0x0000_0000;
const PROGRAM_END: u32 = 0x000F_FFFF;
const DATA_START: u32 = 0x1000_0000;
const DATA_END: u32 = 0x100F_FFFF;
const STACK_START: u32 = 0x2000_0000;
const STACK_END: u32 = 0x200F_FFFF;
const TELEMETRY_START: u32 = 0x4000_0000;
const TELEMETRY_END: u32 = 0x400F_FFFF;
const REQUEST_START: u32 = 0x5000_0000;
const REQUEST_END: u32 = 0x500F_FFFF;
const FEEDBACK_START: u32 = 0x6000_0000;
const FEEDBACK_END: u32 = 0x600F_FFFF;
const SUPERVISOR_START: u32 = 0x7000_0000;
const SUPERVISOR_END: u32 = 0x7000_FFFF;
const SAFETY_START: u32 = 0xF000_0000;
const SAFETY_END: u32 = 0xF000_FFFF;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegionKind {
    Program,
    Data,
    Stack,
    Telemetry,
    ActuatorRequests,
    Feedback,
    Supervisor,
    SafetyControl,
}

impl RegionKind {
    fn for_range(start: u32, end: u32) -> Option<Self> {
        [
            (PROGRAM_START, PROGRAM_END, Self::Program),
            (DATA_START, DATA_END, Self::Data),
            (STACK_START, STACK_END, Self::Stack),
            (TELEMETRY_START, TELEMETRY_END, Self::Telemetry),
            (REQUEST_START, REQUEST_END, Self::ActuatorRequests),
            (FEEDBACK_START, FEEDBACK_END, Self::Feedback),
            (SUPERVISOR_START, SUPERVISOR_END, Self::Supervisor),
            (SAFETY_START, SAFETY_END, Self::SafetyControl),
        ]
        .into_iter()
        .find_map(|(region_start, region_end, kind)| {
            (start >= region_start && end <= region_end).then_some(kind)
        })
    }

    fn requires_capability(self) -> bool {
        !matches!(self, Self::Program | Self::SafetyControl)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Permissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl Permissions {
    pub const READ_EXECUTE: Self = Self {
        read: true,
        write: false,
        execute: true,
    };
    pub const READ_WRITE: Self = Self {
        read: true,
        write: true,
        execute: false,
    };
    pub const READ: Self = Self {
        read: true,
        write: false,
        execute: false,
    };
    pub const WRITE: Self = Self {
        read: false,
        write: true,
        execute: false,
    };
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemorySlot {
    base: u32,
    bytes: Vec<u8>,
    permissions: Permissions,
    kind: RegionKind,
}

impl MemorySlot {
    pub fn new(base: u32, bytes: Vec<u8>, permissions: Permissions) -> Result<Self, ImageError> {
        if bytes.is_empty() {
            return Err(ImageError::EmptySlot { base });
        }
        let length = u32::try_from(bytes.len()).map_err(|_| ImageError::SlotAddressOverflow {
            base,
            length: bytes.len(),
        })?;
        let end = base
            .checked_add(length - 1)
            .ok_or(ImageError::SlotAddressOverflow {
                base,
                length: bytes.len(),
            })?;
        let kind = RegionKind::for_range(base, end).ok_or(ImageError::SlotOutsideRegion {
            base,
            length: bytes.len(),
        })?;
        validate_region_permissions(kind, permissions)?;
        Ok(Self {
            base,
            bytes,
            permissions,
            kind,
        })
    }

    pub fn base(&self) -> u32 {
        self.base
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn end(&self) -> u32 {
        self.base + u32::try_from(self.bytes.len()).unwrap_or(u32::MAX) - 1
    }

    fn contains(&self, address: u32) -> bool {
        address >= self.base && address <= self.end()
    }

    fn contains_range(&self, address: u32, end: u32) -> bool {
        address >= self.base && end <= self.end()
    }
}

fn validate_region_permissions(
    kind: RegionKind,
    permissions: Permissions,
) -> Result<(), ImageError> {
    let allowed = match kind {
        RegionKind::Program => Permissions::READ_EXECUTE,
        RegionKind::Data | RegionKind::Stack => Permissions::READ_WRITE,
        RegionKind::Telemetry | RegionKind::Feedback | RegionKind::Supervisor => Permissions::READ,
        RegionKind::ActuatorRequests => Permissions::WRITE,
        RegionKind::SafetyControl => Permissions::READ_WRITE,
    };
    if (permissions.read && !allowed.read)
        || (permissions.write && !allowed.write)
        || (permissions.execute && !allowed.execute)
    {
        Err(ImageError::RegionPermission { kind, permissions })
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capability {
    pub base: u32,
    pub length: u32,
    pub read: bool,
    pub write: bool,
}

impl Capability {
    fn permits(self, address: u32, end: u32, access: Access) -> bool {
        let Some(capability_end) = self
            .length
            .checked_sub(1)
            .and_then(|offset| self.base.checked_add(offset))
        else {
            return false;
        };
        address >= self.base
            && end <= capability_end
            && match access {
                Access::Read => self.read,
                Access::Write => self.write,
            }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageManifest {
    pub entry_point: u32,
    pub stack_low: u32,
    pub stack_high: u32,
    pub capabilities: Vec<Capability>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageError {
    EmptySlot {
        base: u32,
    },
    SlotAddressOverflow {
        base: u32,
        length: usize,
    },
    SlotOutsideRegion {
        base: u32,
        length: usize,
    },
    RegionPermission {
        kind: RegionKind,
        permissions: Permissions,
    },
    OverlappingSlots {
        first: u32,
        second: u32,
    },
    InvalidCapability {
        index: usize,
    },
    InvalidStackBounds,
    StackNotMapped,
    EntryMisaligned {
        entry: u32,
    },
    EntryUnmapped {
        entry: u32,
    },
    EntryNotExecutable {
        entry: u32,
    },
}

impl fmt::Display for ImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid image: {self:?}")
    }
}

impl std::error::Error for ImageError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Trap {
    PcMisaligned,
    ExecuteUnmapped,
    ExecutePermission,
    IllegalInstruction,
    ReservedEncoding,
    ExecuteAddressOverflow,
    TargetMisaligned,
    TargetUnmapped,
    TargetExecutePermission,
    ArithmeticOverflow,
    DivisionByZero,
    DivisionOverflow,
    DataAddressOverflow,
    DataMisaligned,
    DataUnmapped,
    DataCrossesMapping,
    SupervisorAccess,
    ReadPermission,
    WritePermission,
    CapabilityViolation,
    Explicit { code: u32 },
}

impl Trap {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::PcMisaligned => "pc_misaligned",
            Self::ExecuteUnmapped => "execute_unmapped",
            Self::ExecutePermission => "execute_permission",
            Self::IllegalInstruction => "illegal_instruction",
            Self::ReservedEncoding => "reserved_encoding",
            Self::ExecuteAddressOverflow => "execute_address_overflow",
            Self::TargetMisaligned => "target_misaligned",
            Self::TargetUnmapped => "target_unmapped",
            Self::TargetExecutePermission => "target_execute_permission",
            Self::ArithmeticOverflow => "arithmetic_overflow",
            Self::DivisionByZero => "division_by_zero",
            Self::DivisionOverflow => "division_overflow",
            Self::DataAddressOverflow => "data_address_overflow",
            Self::DataMisaligned => "data_misaligned",
            Self::DataUnmapped => "data_unmapped",
            Self::DataCrossesMapping => "data_crosses_mapping",
            Self::SupervisorAccess => "supervisor_access",
            Self::ReadPermission => "read_permission",
            Self::WritePermission => "write_permission",
            Self::CapabilityViolation => "capability_violation",
            Self::Explicit { .. } => "explicit_trap",
        }
    }
}

impl fmt::Display for Trap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Explicit { code } => write!(formatter, "explicit_trap({code})"),
            other => formatter.write_str(other.code()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineStatus {
    Running,
    Halted,
    Trapped(Trap),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepResult {
    pub pc: u32,
    pub instruction: Option<Instruction>,
    pub cycles_charged: u8,
    pub status: MachineStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StepError {
    NotRunning(MachineStatus),
    CycleBudgetExceeded {
        cycles: u64,
        instruction_cost: u8,
        budget: u64,
    },
}

impl fmt::Display for StepError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRunning(status) => write!(formatter, "machine is not running: {status:?}"),
            Self::CycleBudgetExceeded {
                cycles,
                instruction_cost,
                budget,
            } => write!(
                formatter,
                "cycle budget exceeded: cycles={cycles}, next_cost={instruction_cost}, budget={budget}"
            ),
        }
    }
}

impl std::error::Error for StepError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunResult {
    pub steps: u64,
    pub cycles: u64,
    pub status: MachineStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryWrite {
    pub address: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Machine {
    registers: [u32; 32],
    hi: u32,
    lo: u32,
    pc: u32,
    cycles: u64,
    status: MachineStatus,
    slots: Vec<MemorySlot>,
    capabilities: Vec<Capability>,
    stack_low: u32,
    stack_high: u32,
    memory_writes: Vec<MemoryWrite>,
}

impl Machine {
    pub fn new(manifest: ImageManifest, mut slots: Vec<MemorySlot>) -> Result<Self, ImageError> {
        slots.sort_by_key(MemorySlot::base);
        for pair in slots.windows(2) {
            if pair[0].end() >= pair[1].base() {
                return Err(ImageError::OverlappingSlots {
                    first: pair[0].base(),
                    second: pair[1].base(),
                });
            }
        }
        for (index, capability) in manifest.capabilities.iter().enumerate() {
            let Some(end) = capability
                .length
                .checked_sub(1)
                .and_then(|offset| capability.base.checked_add(offset))
            else {
                return Err(ImageError::InvalidCapability { index });
            };
            let Some(kind) = RegionKind::for_range(capability.base, end) else {
                return Err(ImageError::InvalidCapability { index });
            };
            if !kind.requires_capability() || kind == RegionKind::SafetyControl {
                return Err(ImageError::InvalidCapability { index });
            }
        }
        if manifest.stack_low >= manifest.stack_high
            || manifest.stack_low % 4 != 0
            || manifest.stack_high % 4 != 0
            || RegionKind::for_range(manifest.stack_low, manifest.stack_high - 1)
                != Some(RegionKind::Stack)
        {
            return Err(ImageError::InvalidStackBounds);
        }
        let stack_mapped = slots.iter().any(|slot| {
            slot.kind == RegionKind::Stack
                && slot.contains_range(manifest.stack_low, manifest.stack_high - 1)
                && slot.permissions.read
                && slot.permissions.write
        });
        if !stack_mapped {
            return Err(ImageError::StackNotMapped);
        }
        if manifest.entry_point % 4 != 0 {
            return Err(ImageError::EntryMisaligned {
                entry: manifest.entry_point,
            });
        }
        let Some(entry_slot) = slots
            .iter()
            .find(|slot| slot.contains_range(manifest.entry_point, manifest.entry_point + 3))
        else {
            return Err(ImageError::EntryUnmapped {
                entry: manifest.entry_point,
            });
        };
        if !entry_slot.permissions.execute {
            return Err(ImageError::EntryNotExecutable {
                entry: manifest.entry_point,
            });
        }
        let mut registers = [0; 32];
        registers[usize::from(Register::SP.index())] = manifest.stack_high;
        Ok(Self {
            registers,
            hi: 0,
            lo: 0,
            pc: manifest.entry_point,
            cycles: 0,
            status: MachineStatus::Running,
            slots,
            capabilities: manifest.capabilities,
            stack_low: manifest.stack_low,
            stack_high: manifest.stack_high,
            memory_writes: Vec::new(),
        })
    }

    pub fn register(&self, register: Register) -> u32 {
        if register == Register::ZERO {
            0
        } else {
            self.registers[usize::from(register.index())]
        }
    }

    pub fn set_register(&mut self, register: Register, value: u32) {
        if register != Register::ZERO {
            self.registers[usize::from(register.index())] = value;
        }
    }

    pub fn hi(&self) -> u32 {
        self.hi
    }

    pub fn lo(&self) -> u32 {
        self.lo
    }

    pub fn pc(&self) -> u32 {
        self.pc
    }

    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    pub fn status(&self) -> &MachineStatus {
        &self.status
    }

    pub fn slot_bytes(&self, base: u32) -> Option<&[u8]> {
        self.slots
            .iter()
            .find(|slot| slot.base == base)
            .map(MemorySlot::bytes)
    }

    /// Replaces one complete telemetry or feedback slot from the external
    /// hardware model. This bypasses firmware permissions but cannot alter
    /// program, stack, actuator-request, supervisor, or safety memory.
    pub fn update_device_slot(&mut self, base: u32, bytes: &[u8]) -> bool {
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.base == base) else {
            return false;
        };
        if !matches!(slot.kind, RegionKind::Telemetry | RegionKind::Feedback)
            || slot.bytes.len() != bytes.len()
        {
            return false;
        }
        slot.bytes.copy_from_slice(bytes);
        true
    }

    pub fn memory_writes(&self) -> &[MemoryWrite] {
        &self.memory_writes
    }

    pub fn run(&mut self, cycle_budget: u64) -> Result<RunResult, StepError> {
        let mut steps = 0_u64;
        while self.status == MachineStatus::Running {
            self.step(cycle_budget)?;
            steps = steps.saturating_add(1);
        }
        Ok(RunResult {
            steps,
            cycles: self.cycles,
            status: self.status.clone(),
        })
    }

    pub fn step(&mut self, cycle_budget: u64) -> Result<StepResult, StepError> {
        if self.status != MachineStatus::Running {
            return Err(StepError::NotRunning(self.status.clone()));
        }
        let start_pc = self.pc;
        let word = match self.fetch() {
            Ok(word) => word,
            Err(trap) => return self.commit_trap(start_pc, None, 1, trap, cycle_budget),
        };
        let instruction = match decode(word) {
            Ok(instruction) => instruction,
            Err(error) => {
                let trap = match error.kind {
                    DecodeErrorKind::IllegalInstruction => Trap::IllegalInstruction,
                    DecodeErrorKind::ReservedEncoding => Trap::ReservedEncoding,
                };
                return self.commit_trap(start_pc, None, 1, trap, cycle_budget);
            }
        };
        let cost = instruction.cycle_cost();
        self.check_budget(cost, cycle_budget)?;
        let result = self.execute(instruction);
        match result {
            Ok(()) => {
                self.cycles += u64::from(cost);
                Ok(StepResult {
                    pc: start_pc,
                    instruction: Some(instruction),
                    cycles_charged: cost,
                    status: self.status.clone(),
                })
            }
            Err(trap) => self.commit_trap(start_pc, Some(instruction), cost, trap, cycle_budget),
        }
    }

    fn check_budget(&self, cost: u8, budget: u64) -> Result<(), StepError> {
        if self
            .cycles
            .checked_add(u64::from(cost))
            .is_none_or(|next| next > budget)
        {
            Err(StepError::CycleBudgetExceeded {
                cycles: self.cycles,
                instruction_cost: cost,
                budget,
            })
        } else {
            Ok(())
        }
    }

    fn commit_trap(
        &mut self,
        pc: u32,
        instruction: Option<Instruction>,
        cost: u8,
        trap: Trap,
        budget: u64,
    ) -> Result<StepResult, StepError> {
        self.check_budget(cost, budget)?;
        self.cycles += u64::from(cost);
        self.status = MachineStatus::Trapped(trap);
        Ok(StepResult {
            pc,
            instruction,
            cycles_charged: cost,
            status: self.status.clone(),
        })
    }

    fn fetch(&self) -> Result<u32, Trap> {
        if self.pc % 4 != 0 {
            return Err(Trap::PcMisaligned);
        }
        let end = self.pc.checked_add(3).ok_or(Trap::ExecuteUnmapped)?;
        let slot = self
            .slots
            .iter()
            .find(|slot| slot.contains_range(self.pc, end))
            .ok_or(Trap::ExecuteUnmapped)?;
        if !slot.permissions.execute {
            return Err(Trap::ExecutePermission);
        }
        let offset = usize::try_from(self.pc - slot.base).map_err(|_| Trap::ExecuteUnmapped)?;
        let bytes = slot
            .bytes
            .get(offset..offset + 4)
            .ok_or(Trap::ExecuteUnmapped)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn execute(&mut self, instruction: Instruction) -> Result<(), Trap> {
        let next = self.pc.checked_add(4).ok_or(Trap::ExecuteAddressOverflow)?;
        match instruction {
            Instruction::ShiftImmediate {
                operation,
                destination,
                value,
                amount,
            } => self.write_and_advance(
                destination,
                shift(operation, self.register(value), amount),
                next,
            ),
            Instruction::ShiftVariable {
                operation,
                destination,
                value,
                amount,
            } => self.write_and_advance(
                destination,
                shift(
                    operation,
                    self.register(value),
                    (self.register(amount) & 31) as u8,
                ),
                next,
            ),
            Instruction::JumpRegister { target, link } => {
                let target = self.register(target);
                self.validate_target(target)?;
                if let Some(link) = link {
                    self.set_register(link, next);
                }
                self.pc = target;
            }
            Instruction::MoveFrom {
                destination,
                source,
            } => {
                let value = match source {
                    SpecialRegister::Hi => self.hi,
                    SpecialRegister::Lo => self.lo,
                };
                self.write_and_advance(destination, value, next);
            }
            Instruction::MoveTo {
                destination,
                source,
            } => {
                let value = self.register(source);
                match destination {
                    SpecialRegister::Hi => self.hi = value,
                    SpecialRegister::Lo => self.lo = value,
                }
                self.pc = next;
            }
            Instruction::Multiply {
                signed,
                left,
                right,
            } => {
                let product = if signed {
                    i64::from(self.register(left) as i32)
                        .wrapping_mul(i64::from(self.register(right) as i32))
                        as u64
                } else {
                    u64::from(self.register(left)) * u64::from(self.register(right))
                };
                self.hi = (product >> 32) as u32;
                self.lo = product as u32;
                self.pc = next;
            }
            Instruction::Divide {
                signed,
                dividend,
                divisor,
            } => {
                let dividend = self.register(dividend);
                let divisor = self.register(divisor);
                if divisor == 0 {
                    return Err(Trap::DivisionByZero);
                }
                if signed {
                    let left = dividend as i32;
                    let right = divisor as i32;
                    if left == i32::MIN && right == -1 {
                        return Err(Trap::DivisionOverflow);
                    }
                    self.lo = (left / right) as u32;
                    self.hi = (left % right) as u32;
                } else {
                    self.lo = dividend / divisor;
                    self.hi = dividend % divisor;
                }
                self.pc = next;
            }
            Instruction::AluRegister {
                operation,
                destination,
                left,
                right,
            } => {
                let value = alu(operation, self.register(left), self.register(right))?;
                self.write_and_advance(destination, value, next);
            }
            Instruction::BranchSign {
                non_negative,
                value,
                displacement,
            } => {
                let taken = (self.register(value) as i32 >= 0) == non_negative;
                self.pc = if taken {
                    let target = branch_target(next, displacement)?;
                    self.validate_target(target)?;
                    target
                } else {
                    next
                };
            }
            Instruction::Jump { link, target } => {
                let target = (next & 0xF000_0000) | (target << 2);
                self.validate_target(target)?;
                if link {
                    self.set_register(Register::RA, next);
                }
                self.pc = target;
            }
            Instruction::BranchEqual {
                equal,
                left,
                right,
                displacement,
            } => {
                let taken = (self.register(left) == self.register(right)) == equal;
                self.pc = if taken {
                    let target = branch_target(next, displacement)?;
                    self.validate_target(target)?;
                    target
                } else {
                    next
                };
            }
            Instruction::AluImmediate {
                operation,
                target,
                source,
                immediate_bits,
            } => {
                let value = immediate_alu(operation, self.register(source), immediate_bits)?;
                self.write_and_advance(target, value, next);
            }
            Instruction::LoadUpperImmediate { target, immediate } => {
                self.write_and_advance(target, u32::from(immediate) << 16, next);
            }
            Instruction::Load {
                kind,
                target,
                base,
                offset,
            } => {
                let width = match kind {
                    LoadKind::Byte | LoadKind::ByteUnsigned => 1,
                    LoadKind::Halfword | LoadKind::HalfwordUnsigned => 2,
                    LoadKind::Word => 4,
                };
                let address = effective_address(self.register(base), offset)?;
                let bytes = self.validate_data(address, width, Access::Read)?;
                let value = match kind {
                    LoadKind::Byte => i32::from(bytes[0] as i8) as u32,
                    LoadKind::Halfword => {
                        i32::from(i16::from_le_bytes([bytes[0], bytes[1]])) as u32
                    }
                    LoadKind::Word => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
                    LoadKind::ByteUnsigned => u32::from(bytes[0]),
                    LoadKind::HalfwordUnsigned => {
                        u32::from(u16::from_le_bytes([bytes[0], bytes[1]]))
                    }
                };
                self.write_and_advance(target, value, next);
            }
            Instruction::Store {
                width,
                source,
                base,
                offset,
            } => {
                let width_bytes = match width {
                    StoreWidth::Byte => 1,
                    StoreWidth::Halfword => 2,
                    StoreWidth::Word => 4,
                };
                let address = effective_address(self.register(base), offset)?;
                self.validate_data(address, width_bytes, Access::Write)?;
                let value = self.register(source).to_le_bytes();
                self.write_memory(address, &value[..width_bytes]);
                self.pc = next;
            }
            Instruction::Halt => {
                self.pc = next;
                self.status = MachineStatus::Halted;
            }
            Instruction::Trap { code } => return Err(Trap::Explicit { code }),
        }
        Ok(())
    }

    fn write_and_advance(&mut self, register: Register, value: u32, next: u32) {
        self.set_register(register, value);
        self.pc = next;
    }

    fn validate_target(&self, target: u32) -> Result<(), Trap> {
        if target % 4 != 0 {
            return Err(Trap::TargetMisaligned);
        }
        let Some(end) = target.checked_add(3) else {
            return Err(Trap::TargetUnmapped);
        };
        let slot = self
            .slots
            .iter()
            .find(|slot| slot.contains_range(target, end))
            .ok_or(Trap::TargetUnmapped)?;
        if !slot.permissions.execute {
            return Err(Trap::TargetExecutePermission);
        }
        Ok(())
    }

    fn validate_data(&self, address: u32, width: usize, access: Access) -> Result<&[u8], Trap> {
        if width > 1 && address % u32::try_from(width).unwrap_or(1) != 0 {
            return Err(Trap::DataMisaligned);
        }
        let end = address
            .checked_add(u32::try_from(width - 1).map_err(|_| Trap::DataAddressOverflow)?)
            .ok_or(Trap::DataAddressOverflow)?;
        let Some(slot) = self.slots.iter().find(|slot| slot.contains(address)) else {
            return Err(Trap::DataUnmapped);
        };
        if !slot.contains_range(address, end) {
            return Err(Trap::DataCrossesMapping);
        }
        if slot.kind == RegionKind::SafetyControl {
            return Err(Trap::SupervisorAccess);
        }
        if slot.kind == RegionKind::Stack && (address < self.stack_low || end >= self.stack_high) {
            return Err(Trap::CapabilityViolation);
        }
        match access {
            Access::Read if !slot.permissions.read => return Err(Trap::ReadPermission),
            Access::Write if !slot.permissions.write => return Err(Trap::WritePermission),
            _ => {}
        }
        if slot.kind.requires_capability()
            && !self
                .capabilities
                .iter()
                .any(|capability| capability.permits(address, end, access))
        {
            return Err(Trap::CapabilityViolation);
        }
        let offset = usize::try_from(address - slot.base).map_err(|_| Trap::DataUnmapped)?;
        slot.bytes
            .get(offset..offset + width)
            .ok_or(Trap::DataCrossesMapping)
    }

    fn write_memory(&mut self, address: u32, bytes: &[u8]) {
        if let Some(slot) = self.slots.iter_mut().find(|slot| slot.contains(address)) {
            let offset = usize::try_from(address - slot.base).unwrap_or(0);
            if let Some(destination) = slot.bytes.get_mut(offset..offset + bytes.len()) {
                destination.copy_from_slice(bytes);
                self.memory_writes.push(MemoryWrite {
                    address,
                    bytes: bytes.to_vec(),
                });
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Access {
    Read,
    Write,
}

fn effective_address(base: u32, offset: i16) -> Result<u32, Trap> {
    let value = i64::from(base) + i64::from(offset);
    u32::try_from(value).map_err(|_| Trap::DataAddressOverflow)
}

fn branch_target(next: u32, displacement: i16) -> Result<u32, Trap> {
    let value = i64::from(next) + (i64::from(displacement) << 2);
    u32::try_from(value).map_err(|_| Trap::ExecuteAddressOverflow)
}

fn shift(operation: ShiftOperation, value: u32, amount: u8) -> u32 {
    match operation {
        ShiftOperation::Left => value << amount,
        ShiftOperation::RightLogical => value >> amount,
        ShiftOperation::RightArithmetic => ((value as i32) >> amount) as u32,
    }
}

fn alu(operation: AluOperation, left: u32, right: u32) -> Result<u32, Trap> {
    match operation {
        AluOperation::Add => (left as i32)
            .checked_add(right as i32)
            .map(|value| value as u32)
            .ok_or(Trap::ArithmeticOverflow),
        AluOperation::AddUnsigned => Ok(left.wrapping_add(right)),
        AluOperation::Subtract => (left as i32)
            .checked_sub(right as i32)
            .map(|value| value as u32)
            .ok_or(Trap::ArithmeticOverflow),
        AluOperation::SubtractUnsigned => Ok(left.wrapping_sub(right)),
        AluOperation::And => Ok(left & right),
        AluOperation::Or => Ok(left | right),
        AluOperation::Xor => Ok(left ^ right),
        AluOperation::Nor => Ok(!(left | right)),
        AluOperation::SetLessThan => Ok(u32::from((left as i32) < (right as i32))),
        AluOperation::SetLessThanUnsigned => Ok(u32::from(left < right)),
    }
}

fn immediate_alu(
    operation: ImmediateOperation,
    source: u32,
    immediate_bits: u16,
) -> Result<u32, Trap> {
    let sign_extended = i32::from(immediate_bits as i16) as u32;
    match operation {
        ImmediateOperation::Add => (source as i32)
            .checked_add(i32::from(immediate_bits as i16))
            .map(|value| value as u32)
            .ok_or(Trap::ArithmeticOverflow),
        ImmediateOperation::AddUnsigned => Ok(source.wrapping_add(sign_extended)),
        ImmediateOperation::SetLessThan => Ok(u32::from(
            (source as i32) < i32::from(immediate_bits as i16),
        )),
        ImmediateOperation::SetLessThanUnsigned => Ok(u32::from(source < sign_extended)),
        ImmediateOperation::And => Ok(source & u32::from(immediate_bits)),
        ImmediateOperation::Or => Ok(source | u32::from(immediate_bits)),
        ImmediateOperation::Xor => Ok(source ^ u32::from(immediate_bits)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STACK_BASE: u32 = 0x2000_0000;
    const STACK_SIZE: usize = 256;

    fn register(index: u8) -> Register {
        Register::new(index).unwrap_or(Register::ZERO)
    }

    fn words(instructions: &[Instruction]) -> Vec<u8> {
        instructions
            .iter()
            .flat_map(|instruction| instruction.encode().unwrap_or(0xFFFF_FFFF).to_le_bytes())
            .collect()
    }

    fn machine(instructions: &[Instruction]) -> Machine {
        machine_with_slots(instructions, Vec::new(), Vec::new())
    }

    fn machine_with_slots(
        instructions: &[Instruction],
        mut slots: Vec<MemorySlot>,
        mut capabilities: Vec<Capability>,
    ) -> Machine {
        slots.push(
            MemorySlot::new(0, words(instructions), Permissions::READ_EXECUTE)
                .unwrap_or_else(|error| panic!("{error}")),
        );
        slots.push(
            MemorySlot::new(STACK_BASE, vec![0; STACK_SIZE], Permissions::READ_WRITE)
                .unwrap_or_else(|error| panic!("{error}")),
        );
        capabilities.push(Capability {
            base: STACK_BASE,
            length: STACK_SIZE as u32,
            read: true,
            write: true,
        });
        Machine::new(
            ImageManifest {
                entry_point: 0,
                stack_low: STACK_BASE,
                stack_high: STACK_BASE + STACK_SIZE as u32,
                capabilities,
            },
            slots,
        )
        .unwrap_or_else(|error| panic!("{error}"))
    }

    fn step(machine: &mut Machine) -> StepResult {
        machine
            .step(u64::MAX)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn reset_sets_entry_stack_and_zero_state() {
        let machine = machine(&[Instruction::Halt]);
        assert_eq!(machine.pc(), 0);
        assert_eq!(
            machine.register(Register::SP),
            STACK_BASE + STACK_SIZE as u32
        );
        assert_eq!(machine.register(Register::ZERO), 0);
        assert_eq!(machine.hi(), 0);
        assert_eq!(machine.lo(), 0);
        assert_eq!(machine.cycles(), 0);
        assert_eq!(machine.status(), &MachineStatus::Running);
    }

    #[test]
    fn external_hardware_updates_only_telemetry_and_feedback_slots() {
        let telemetry = TELEMETRY_START;
        let request = REQUEST_START;
        let mut machine = machine_with_slots(
            &[Instruction::Halt],
            vec![
                MemorySlot::new(telemetry, vec![0; 4], Permissions::READ)
                    .unwrap_or_else(|error| panic!("{error}")),
                MemorySlot::new(request, vec![0; 4], Permissions::WRITE)
                    .unwrap_or_else(|error| panic!("{error}")),
            ],
            vec![
                Capability {
                    base: telemetry,
                    length: 4,
                    read: true,
                    write: false,
                },
                Capability {
                    base: request,
                    length: 4,
                    read: false,
                    write: true,
                },
            ],
        );

        assert!(machine.update_device_slot(telemetry, &42_u32.to_le_bytes()));
        assert_eq!(
            machine.slot_bytes(telemetry),
            Some(42_u32.to_le_bytes().as_slice())
        );
        assert!(!machine.update_device_slot(request, &1_u32.to_le_bytes()));
        assert!(!machine.update_device_slot(telemetry, &[1, 2]));
    }

    #[test]
    fn shifts_alu_and_r0_follow_execution_goldens() {
        let r1 = register(1);
        let r2 = register(2);
        let r3 = register(3);
        let mut vm = machine(&[
            Instruction::ShiftImmediate {
                operation: ShiftOperation::RightArithmetic,
                destination: r3,
                value: r2,
                amount: 4,
            },
            Instruction::ShiftVariable {
                operation: ShiftOperation::RightLogical,
                destination: r3,
                value: r2,
                amount: r1,
            },
            Instruction::AluRegister {
                operation: AluOperation::Add,
                destination: r3,
                left: r1,
                right: r2,
            },
            Instruction::AluRegister {
                operation: AluOperation::SubtractUnsigned,
                destination: Register::ZERO,
                left: r1,
                right: r2,
            },
            Instruction::Halt,
        ]);
        vm.set_register(r1, 4);
        vm.set_register(r2, 0x8000_0001);
        step(&mut vm);
        assert_eq!(vm.register(r3), 0xF800_0000);
        step(&mut vm);
        assert_eq!(vm.register(r3), 0x0800_0000);
        step(&mut vm);
        assert_eq!(vm.register(r3), 0x8000_0005);
        step(&mut vm);
        assert_eq!(vm.register(Register::ZERO), 0);
    }

    #[test]
    fn every_register_alu_operation_has_expected_result() {
        let cases = [
            (AluOperation::Add, 1, 2, 3),
            (AluOperation::AddUnsigned, u32::MAX, 2, 1),
            (AluOperation::Subtract, 1, 2, u32::MAX),
            (AluOperation::SubtractUnsigned, 0, 1, u32::MAX),
            (AluOperation::And, 0xA5, 0x3C, 0x24),
            (AluOperation::Or, 0xA5, 0x3C, 0xBD),
            (AluOperation::Xor, 0xA5, 0x3C, 0x99),
            (AluOperation::Nor, 0xA5, 0x3C, !0xBD),
            (AluOperation::SetLessThan, u32::MAX, 1, 1),
            (AluOperation::SetLessThanUnsigned, u32::MAX, 1, 0),
        ];
        for (operation, left, right, expected) in cases {
            let mut vm = machine(&[Instruction::AluRegister {
                operation,
                destination: register(3),
                left: register(1),
                right: register(2),
            }]);
            vm.set_register(register(1), left);
            vm.set_register(register(2), right);
            step(&mut vm);
            assert_eq!(vm.register(register(3)), expected, "{operation:?}");
        }
    }

    #[test]
    fn immediates_sign_extend_only_where_specified() {
        let cases = [
            (ImmediateOperation::Add, 2, 0xFFFF, 1),
            (ImmediateOperation::AddUnsigned, 0, 0xFFFF, u32::MAX),
            (ImmediateOperation::SetLessThan, u32::MAX, 0, 1),
            (ImmediateOperation::SetLessThanUnsigned, 0, 0xFFFF, 1),
            (ImmediateOperation::And, 0xFFFF_00FF, 0x0FF0, 0xF0),
            (ImmediateOperation::Or, 0xFFFF_0000, 0x00FF, 0xFFFF_00FF),
            (ImmediateOperation::Xor, 0xFFFF_00FF, 0x00FF, 0xFFFF_0000),
        ];
        for (operation, source, immediate_bits, expected) in cases {
            let mut vm = machine(&[Instruction::AluImmediate {
                operation,
                target: register(2),
                source: register(1),
                immediate_bits,
            }]);
            vm.set_register(register(1), source);
            step(&mut vm);
            assert_eq!(vm.register(register(2)), expected, "{operation:?}");
        }
    }

    #[test]
    fn multiply_divide_and_special_register_moves_are_bit_exact() {
        let r1 = register(1);
        let r2 = register(2);
        let r3 = register(3);
        let mut signed_product = machine(&[
            Instruction::Multiply {
                signed: true,
                left: r1,
                right: r2,
            },
            Instruction::MoveFrom {
                destination: r3,
                source: SpecialRegister::Hi,
            },
        ]);
        signed_product.set_register(r1, u32::MAX - 1);
        signed_product.set_register(r2, 3);
        step(&mut signed_product);
        assert_eq!(
            (signed_product.hi(), signed_product.lo()),
            (u32::MAX, 0xFFFF_FFFA)
        );
        step(&mut signed_product);
        assert_eq!(signed_product.register(r3), u32::MAX);

        let mut unsigned_product = machine(&[Instruction::Multiply {
            signed: false,
            left: r1,
            right: r2,
        }]);
        unsigned_product.set_register(r1, u32::MAX - 1);
        unsigned_product.set_register(r2, 3);
        step(&mut unsigned_product);
        assert_eq!(
            (unsigned_product.hi(), unsigned_product.lo()),
            (2, 0xFFFF_FFFA)
        );

        let mut division = machine(&[
            Instruction::MoveTo {
                destination: SpecialRegister::Hi,
                source: r3,
            },
            Instruction::Divide {
                signed: true,
                dividend: r1,
                divisor: r2,
            },
        ]);
        division.set_register(r1, (-7_i32) as u32);
        division.set_register(r2, 3);
        division.set_register(r3, 99);
        step(&mut division);
        assert_eq!(division.hi(), 99);
        step(&mut division);
        assert_eq!((division.hi(), division.lo()), (u32::MAX, (-2_i32) as u32));
    }

    #[test]
    fn branches_jumps_and_links_have_no_delay_slots() {
        let r1 = register(1);
        let r2 = register(2);
        let mut branch = machine(&[
            Instruction::BranchEqual {
                equal: true,
                left: r1,
                right: r2,
                displacement: 1,
            },
            Instruction::Trap { code: 7 },
            Instruction::BranchSign {
                non_negative: false,
                value: r1,
                displacement: -1,
            },
        ]);
        branch.set_register(r1, u32::MAX);
        branch.set_register(r2, u32::MAX);
        step(&mut branch);
        assert_eq!(branch.pc(), 8);
        step(&mut branch);
        assert_eq!(branch.pc(), 8);

        let mut jump = machine(&[
            Instruction::Jump {
                link: true,
                target: 2,
            },
            Instruction::Trap { code: 8 },
            Instruction::JumpRegister {
                target: register(3),
                link: Some(r2),
            },
            Instruction::Halt,
        ]);
        jump.set_register(register(3), 12);
        step(&mut jump);
        assert_eq!(jump.pc(), 8);
        assert_eq!(jump.register(Register::RA), 4);
        step(&mut jump);
        assert_eq!(jump.pc(), 12);
        assert_eq!(jump.register(r2), 12);
    }

    #[test]
    fn bne_bgez_plain_jump_unsigned_divide_and_lo_moves_execute() {
        let r1 = register(1);
        let r2 = register(2);
        let r3 = register(3);
        let mut control = machine(&[
            Instruction::BranchEqual {
                equal: false,
                left: r1,
                right: r2,
                displacement: 1,
            },
            Instruction::Trap { code: 1 },
            Instruction::BranchSign {
                non_negative: true,
                value: r1,
                displacement: 1,
            },
            Instruction::Trap { code: 2 },
            Instruction::Jump {
                link: false,
                target: 6,
            },
            Instruction::Trap { code: 3 },
            Instruction::Halt,
        ]);
        control.set_register(r1, 1);
        control.set_register(r2, 2);
        let result = control.run(10).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(result.status, MachineStatus::Halted);
        assert_eq!(control.pc(), 28);
        assert_eq!(control.register(Register::RA), 0);

        let mut arithmetic = machine(&[
            Instruction::MoveTo {
                destination: SpecialRegister::Lo,
                source: r3,
            },
            Instruction::MoveFrom {
                destination: r3,
                source: SpecialRegister::Lo,
            },
            Instruction::Divide {
                signed: false,
                dividend: r1,
                divisor: r2,
            },
        ]);
        arithmetic.set_register(r1, 7);
        arithmetic.set_register(r2, 3);
        arithmetic.set_register(r3, 99);
        step(&mut arithmetic);
        arithmetic.set_register(r3, 0);
        step(&mut arithmetic);
        assert_eq!(arithmetic.register(r3), 99);
        step(&mut arithmetic);
        assert_eq!((arithmetic.hi(), arithmetic.lo()), (1, 2));
    }

    #[test]
    fn loads_stores_permissions_and_capabilities_are_enforced() {
        let data_base = 0x1000_0000;
        let data = MemorySlot::new(
            data_base,
            vec![0x80, 0, 0, 0x80, 0, 0, 0, 0],
            Permissions::READ_WRITE,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        let capability = Capability {
            base: data_base,
            length: 8,
            read: true,
            write: true,
        };
        let r1 = register(1);
        let r2 = register(2);
        let r3 = register(3);
        let mut vm = machine_with_slots(
            &[
                Instruction::Load {
                    kind: LoadKind::Byte,
                    target: r2,
                    base: r1,
                    offset: 0,
                },
                Instruction::Load {
                    kind: LoadKind::HalfwordUnsigned,
                    target: r3,
                    base: r1,
                    offset: 2,
                },
                Instruction::Store {
                    width: StoreWidth::Word,
                    source: r2,
                    base: r1,
                    offset: 4,
                },
            ],
            vec![data],
            vec![capability],
        );
        vm.set_register(r1, data_base);
        step(&mut vm);
        assert_eq!(vm.register(r2), 0xFFFF_FF80);
        step(&mut vm);
        assert_eq!(vm.register(r3), 0x8000);
        step(&mut vm);
        assert_eq!(
            vm.slot_bytes(data_base).and_then(|bytes| bytes.get(4..8)),
            Some(&[0x80, 0xFF, 0xFF, 0xFF][..])
        );

        let read_only = MemorySlot::new(data_base, vec![0; 4], Permissions::READ)
            .unwrap_or_else(|error| panic!("{error}"));
        let mut denied = machine_with_slots(
            &[Instruction::Store {
                width: StoreWidth::Byte,
                source: r2,
                base: r1,
                offset: 0,
            }],
            vec![read_only],
            vec![capability],
        );
        denied.set_register(r1, data_base);
        step(&mut denied);
        assert_eq!(
            denied.status(),
            &MachineStatus::Trapped(Trap::WritePermission)
        );
    }

    #[test]
    fn remaining_shift_load_store_and_lui_forms_execute() {
        let data_base = 0x1000_0000;
        let data = MemorySlot::new(
            data_base,
            vec![0x80, 0xFF, 0, 0, 0x78, 0x56, 0x34, 0x12, 0, 0, 0, 0],
            Permissions::READ_WRITE,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        let r1 = register(1);
        let r2 = register(2);
        let r3 = register(3);
        let r4 = register(4);
        let mut vm = machine_with_slots(
            &[
                Instruction::LoadUpperImmediate {
                    target: r2,
                    immediate: 0x8000,
                },
                Instruction::ShiftImmediate {
                    operation: ShiftOperation::Left,
                    destination: r3,
                    value: r2,
                    amount: 1,
                },
                Instruction::ShiftImmediate {
                    operation: ShiftOperation::RightLogical,
                    destination: r3,
                    value: r2,
                    amount: 4,
                },
                Instruction::ShiftVariable {
                    operation: ShiftOperation::Left,
                    destination: r3,
                    value: r2,
                    amount: r4,
                },
                Instruction::ShiftVariable {
                    operation: ShiftOperation::RightArithmetic,
                    destination: r3,
                    value: r2,
                    amount: r4,
                },
                Instruction::Load {
                    kind: LoadKind::Halfword,
                    target: r2,
                    base: r1,
                    offset: 0,
                },
                Instruction::Load {
                    kind: LoadKind::Word,
                    target: r3,
                    base: r1,
                    offset: 4,
                },
                Instruction::Load {
                    kind: LoadKind::ByteUnsigned,
                    target: r4,
                    base: r1,
                    offset: 0,
                },
                Instruction::Store {
                    width: StoreWidth::Byte,
                    source: r3,
                    base: r1,
                    offset: 8,
                },
                Instruction::Store {
                    width: StoreWidth::Halfword,
                    source: r3,
                    base: r1,
                    offset: 10,
                },
            ],
            vec![data],
            vec![Capability {
                base: data_base,
                length: 12,
                read: true,
                write: true,
            }],
        );
        vm.set_register(r1, data_base);
        vm.set_register(r4, 4);
        step(&mut vm);
        assert_eq!(vm.register(r2), 0x8000_0000);
        step(&mut vm);
        assert_eq!(vm.register(r3), 0);
        step(&mut vm);
        assert_eq!(vm.register(r3), 0x0800_0000);
        step(&mut vm);
        assert_eq!(vm.register(r3), 0);
        step(&mut vm);
        assert_eq!(vm.register(r3), 0xF800_0000);
        step(&mut vm);
        assert_eq!(vm.register(r2), 0xFFFF_FF80);
        step(&mut vm);
        assert_eq!(vm.register(r3), 0x1234_5678);
        step(&mut vm);
        assert_eq!(vm.register(r4), 0x80);
        step(&mut vm);
        step(&mut vm);
        assert_eq!(
            vm.slot_bytes(data_base).and_then(|bytes| bytes.get(8..12)),
            Some(&[0x78, 0, 0x78, 0x56][..])
        );
    }

    #[test]
    fn data_trap_precedence_and_load_to_r0_are_observable() {
        let data_base = 0x1000_0000;
        let data = MemorySlot::new(data_base, vec![1, 2, 3, 4], Permissions::READ)
            .unwrap_or_else(|error| panic!("{error}"));
        let r1 = register(1);
        let mut misaligned = machine_with_slots(
            &[Instruction::Load {
                kind: LoadKind::Word,
                target: Register::ZERO,
                base: r1,
                offset: 1,
            }],
            vec![data.clone()],
            Vec::new(),
        );
        misaligned.set_register(r1, data_base);
        step(&mut misaligned);
        assert_eq!(
            misaligned.status(),
            &MachineStatus::Trapped(Trap::DataMisaligned)
        );
        assert_eq!(misaligned.register(Register::ZERO), 0);

        let mut no_capability = machine_with_slots(
            &[Instruction::Load {
                kind: LoadKind::ByteUnsigned,
                target: Register::ZERO,
                base: r1,
                offset: 0,
            }],
            vec![data],
            Vec::new(),
        );
        no_capability.set_register(r1, data_base);
        step(&mut no_capability);
        assert_eq!(
            no_capability.status(),
            &MachineStatus::Trapped(Trap::CapabilityViolation)
        );
    }

    #[test]
    fn manifested_stack_bounds_limit_a_larger_mapping_and_capability() {
        let r1 = register(1);
        let program = MemorySlot::new(
            0,
            words(&[Instruction::Store {
                width: StoreWidth::Word,
                source: Register::ZERO,
                base: r1,
                offset: 0,
            }]),
            Permissions::READ_EXECUTE,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        let stack = MemorySlot::new(STACK_BASE, vec![0; 512], Permissions::READ_WRITE)
            .unwrap_or_else(|error| panic!("{error}"));
        let mut vm = Machine::new(
            ImageManifest {
                entry_point: 0,
                stack_low: STACK_BASE,
                stack_high: STACK_BASE + STACK_SIZE as u32,
                capabilities: vec![Capability {
                    base: STACK_BASE,
                    length: 512,
                    read: true,
                    write: true,
                }],
            },
            vec![program, stack],
        )
        .unwrap_or_else(|error| panic!("{error}"));
        vm.set_register(r1, STACK_BASE + STACK_SIZE as u32);
        step(&mut vm);
        assert_eq!(
            vm.status(),
            &MachineStatus::Trapped(Trap::CapabilityViolation)
        );
    }

    #[test]
    fn arithmetic_and_division_traps_are_atomic_and_charge_cost() {
        let r1 = register(1);
        let r2 = register(2);
        let r3 = register(3);
        let mut overflow = machine(&[Instruction::AluRegister {
            operation: AluOperation::Add,
            destination: r3,
            left: r1,
            right: r2,
        }]);
        overflow.set_register(r1, i32::MAX as u32);
        overflow.set_register(r2, 1);
        overflow.set_register(r3, 55);
        step(&mut overflow);
        assert_eq!(overflow.register(r3), 55);
        assert_eq!(overflow.pc(), 0);
        assert_eq!(overflow.cycles(), 1);
        assert_eq!(
            overflow.status(),
            &MachineStatus::Trapped(Trap::ArithmeticOverflow)
        );

        let mut division = machine(&[Instruction::Divide {
            signed: true,
            dividend: r1,
            divisor: r2,
        }]);
        division.set_register(r1, i32::MIN as u32);
        division.set_register(r2, u32::MAX);
        step(&mut division);
        assert_eq!(division.pc(), 0);
        assert_eq!(division.cycles(), 12);
        assert_eq!(
            division.status(),
            &MachineStatus::Trapped(Trap::DivisionOverflow)
        );
    }

    #[test]
    fn fetch_decode_target_halt_and_explicit_traps_are_stable() {
        let mut illegal = machine(&[Instruction::Halt]);
        illegal.slots[0].bytes[0..4].copy_from_slice(&0x4800_0000_u32.to_le_bytes());
        step(&mut illegal);
        assert_eq!(
            illegal.status(),
            &MachineStatus::Trapped(Trap::IllegalInstruction)
        );

        let mut target = machine(&[Instruction::JumpRegister {
            target: register(1),
            link: None,
        }]);
        target.set_register(register(1), 2);
        step(&mut target);
        assert_eq!(
            target.status(),
            &MachineStatus::Trapped(Trap::TargetMisaligned)
        );

        let mut explicit = machine(&[Instruction::Trap { code: 42 }]);
        step(&mut explicit);
        assert_eq!(explicit.pc(), 0);
        assert_eq!(
            explicit.status(),
            &MachineStatus::Trapped(Trap::Explicit { code: 42 })
        );

        let mut halted = machine(&[Instruction::Halt]);
        step(&mut halted);
        assert_eq!(halted.pc(), 4);
        assert_eq!(halted.status(), &MachineStatus::Halted);
        assert!(matches!(
            halted.step(10),
            Err(StepError::NotRunning(MachineStatus::Halted))
        ));
    }

    #[test]
    fn fetch_control_and_division_trap_precedence_is_stable() {
        let mut pc_misaligned = machine(&[Instruction::Halt]);
        pc_misaligned.pc = 2;
        step(&mut pc_misaligned);
        assert_eq!(
            pc_misaligned.status(),
            &MachineStatus::Trapped(Trap::PcMisaligned)
        );

        let mut fetch_unmapped = machine(&[Instruction::Halt]);
        fetch_unmapped.pc = 0x1000;
        step(&mut fetch_unmapped);
        assert_eq!(
            fetch_unmapped.status(),
            &MachineStatus::Trapped(Trap::ExecuteUnmapped)
        );

        let data_base = 0x1000_0000;
        let data = MemorySlot::new(data_base, vec![0; 4], Permissions::READ)
            .unwrap_or_else(|error| panic!("{error}"));
        let mut execute_denied = machine_with_slots(&[Instruction::Halt], vec![data], Vec::new());
        execute_denied.pc = data_base;
        step(&mut execute_denied);
        assert_eq!(
            execute_denied.status(),
            &MachineStatus::Trapped(Trap::ExecutePermission)
        );

        let mut reserved = machine(&[Instruction::Halt]);
        reserved.slots[0].bytes[0..4].copy_from_slice(&0xF800_0001_u32.to_le_bytes());
        step(&mut reserved);
        assert_eq!(
            reserved.status(),
            &MachineStatus::Trapped(Trap::ReservedEncoding)
        );

        let mut branch_overflow = machine(&[Instruction::BranchEqual {
            equal: true,
            left: Register::ZERO,
            right: Register::ZERO,
            displacement: -2,
        }]);
        step(&mut branch_overflow);
        assert_eq!(
            branch_overflow.status(),
            &MachineStatus::Trapped(Trap::ExecuteAddressOverflow)
        );

        let mut target_unmapped = machine(&[Instruction::JumpRegister {
            target: register(1),
            link: None,
        }]);
        target_unmapped.set_register(register(1), 0x1000);
        step(&mut target_unmapped);
        assert_eq!(
            target_unmapped.status(),
            &MachineStatus::Trapped(Trap::TargetUnmapped)
        );

        let data = MemorySlot::new(data_base, vec![0; 4], Permissions::READ)
            .unwrap_or_else(|error| panic!("{error}"));
        let mut target_denied = machine_with_slots(
            &[Instruction::JumpRegister {
                target: register(1),
                link: None,
            }],
            vec![data],
            Vec::new(),
        );
        target_denied.set_register(register(1), data_base);
        step(&mut target_denied);
        assert_eq!(
            target_denied.status(),
            &MachineStatus::Trapped(Trap::TargetExecutePermission)
        );

        let mut division_by_zero = machine(&[Instruction::Divide {
            signed: true,
            dividend: register(1),
            divisor: register(2),
        }]);
        division_by_zero.set_register(register(1), i32::MIN as u32);
        division_by_zero.set_register(register(2), 0);
        step(&mut division_by_zero);
        assert_eq!(
            division_by_zero.status(),
            &MachineStatus::Trapped(Trap::DivisionByZero)
        );
    }

    #[test]
    fn every_data_access_trap_class_is_stable() {
        let r1 = register(1);
        let load_word = Instruction::Load {
            kind: LoadKind::Word,
            target: register(2),
            base: r1,
            offset: 0,
        };
        let mut address_overflow = machine(&[Instruction::Load {
            kind: LoadKind::Word,
            target: register(2),
            base: r1,
            offset: -1,
        }]);
        step(&mut address_overflow);
        assert_eq!(
            address_overflow.status(),
            &MachineStatus::Trapped(Trap::DataAddressOverflow)
        );

        let mut unmapped = machine(&[load_word]);
        unmapped.set_register(r1, 0x1000_0000);
        step(&mut unmapped);
        assert_eq!(
            unmapped.status(),
            &MachineStatus::Trapped(Trap::DataUnmapped)
        );

        let data_base = 0x1000_0000;
        let short_data = MemorySlot::new(data_base, vec![0; 2], Permissions::READ)
            .unwrap_or_else(|error| panic!("{error}"));
        let mut crosses = machine_with_slots(
            &[load_word],
            vec![short_data],
            vec![Capability {
                base: data_base,
                length: 4,
                read: true,
                write: false,
            }],
        );
        crosses.set_register(r1, data_base);
        step(&mut crosses);
        assert_eq!(
            crosses.status(),
            &MachineStatus::Trapped(Trap::DataCrossesMapping)
        );

        let request_base = 0x5000_0000;
        let write_only = MemorySlot::new(request_base, vec![0; 4], Permissions::WRITE)
            .unwrap_or_else(|error| panic!("{error}"));
        let mut read_denied = machine_with_slots(
            &[load_word],
            vec![write_only],
            vec![Capability {
                base: request_base,
                length: 4,
                read: true,
                write: true,
            }],
        );
        read_denied.set_register(r1, request_base);
        step(&mut read_denied);
        assert_eq!(
            read_denied.status(),
            &MachineStatus::Trapped(Trap::ReadPermission)
        );

        let safety_base = 0xF000_0000;
        let safety = MemorySlot::new(safety_base, vec![0; 4], Permissions::READ_WRITE)
            .unwrap_or_else(|error| panic!("{error}"));
        let mut supervisor = machine_with_slots(&[load_word], vec![safety], Vec::new());
        supervisor.set_register(r1, safety_base);
        step(&mut supervisor);
        assert_eq!(
            supervisor.status(),
            &MachineStatus::Trapped(Trap::SupervisorAccess)
        );
    }

    #[test]
    fn cycle_budget_refuses_whole_instruction_without_mutation() {
        let mut vm = machine(&[Instruction::Multiply {
            signed: false,
            left: register(1),
            right: register(2),
        }]);
        vm.set_register(register(1), 2);
        vm.set_register(register(2), 3);
        assert_eq!(
            vm.step(3),
            Err(StepError::CycleBudgetExceeded {
                cycles: 0,
                instruction_cost: 4,
                budget: 3,
            })
        );
        assert_eq!((vm.pc(), vm.cycles(), vm.hi(), vm.lo()), (0, 0, 0, 0));
        assert_eq!(vm.status(), &MachineStatus::Running);
    }

    #[test]
    fn image_validation_rejects_overlap_bad_stack_and_excess_permissions() {
        assert!(matches!(
            MemorySlot::new(0x5000_0000, vec![0; 4], Permissions::READ),
            Err(ImageError::RegionPermission { .. })
        ));

        let program = MemorySlot::new(0, words(&[Instruction::Halt]), Permissions::READ_EXECUTE)
            .unwrap_or_else(|error| panic!("{error}"));
        let overlapping = MemorySlot::new(0, vec![0; 4], Permissions::READ_EXECUTE)
            .unwrap_or_else(|error| panic!("{error}"));
        let stack = MemorySlot::new(STACK_BASE, vec![0; 4], Permissions::READ_WRITE)
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(matches!(
            Machine::new(
                ImageManifest {
                    entry_point: 0,
                    stack_low: STACK_BASE,
                    stack_high: STACK_BASE + 4,
                    capabilities: Vec::new(),
                },
                vec![program, overlapping, stack],
            ),
            Err(ImageError::OverlappingSlots { .. })
        ));
    }

    #[test]
    fn sample_analysis_program_runs_deterministically_to_halt() {
        let source = include_str!("../../../examples/sample-analysis.asm");
        let assembly = crate::assembler::assemble(source, 0)
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let entry = assembly.entry.unwrap_or(assembly.origin);
        let mut vm = machine(&[Instruction::Halt]);
        vm.slots[0] = MemorySlot::new(assembly.origin, assembly.bytes, Permissions::READ_EXECUTE)
            .unwrap_or_else(|error| panic!("{error}"));
        vm.pc = entry;
        let result = vm.run(256).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(result.status, MachineStatus::Halted);
        assert_eq!(result.steps, 91);
        assert_eq!(result.cycles, 112);
        assert_eq!(vm.register(register(2)), 66);
        assert_eq!(vm.register(register(3)), 25);
        assert_eq!(vm.register(register(7)), 3);
        assert_eq!(vm.register(register(8)), 13);
        assert_eq!(vm.register(register(9)), 1);
        assert_eq!(
            vm.register(register(29)),
            STACK_BASE + u32::try_from(STACK_SIZE).unwrap_or(0)
        );
        assert_eq!((vm.hi(), vm.lo(), vm.pc()), (1, 13, 0x70));
    }
}
