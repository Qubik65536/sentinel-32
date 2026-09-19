# S32 ISA v0

**Identifier:** `s32-isa-v0`
**Status:** Accepted under `ISA-001` after human review on 2026-09-19.

S32 is a project-specific, MIPS-shaped ISA and is not MIPS-compatible. This
document is the shared contract for its assembler, decoder, interpreter,
validator, traces, and tests. Anything not granted here is reserved.

## State, reset, and representation

The machine has 32-bit registers `R0..R31`, `HI`, `LO`, and `PC`, a sparse
byte-addressed 32-bit address space, a non-wrapping 64-bit virtual cycle count,
and a `running`, `halted`, or `trapped` state. `R0` always reads zero and
discards writes. `SP`, `FP`, and `RA` alias `R29`, `R30`, and `R31`.

Reset uses a validated image manifest. It zeros every register and the cycle
count, then sets `SP` to the aligned exclusive stack upper bound and `PC` to the
entry point. Reset fails if either value is invalid or the entry point is not
aligned, mapped, and executable.

Instructions are four-byte-aligned 32-bit words stored little-endian:

```text
R: opcode[31:26] rs[25:21] rt[20:16] rd[15:11] shamt[10:6] funct[5:0]
I: opcode[31:26] rs[25:21] rt[20:16] immediate[15:0]
J: opcode[31:26] target[25:0]
```

A recognized instruction with a nonzero field required to be zero traps as
`reserved_encoding`. An unassigned opcode, function, or `REGIMM` selector traps
as `illegal_instruction`.

## Encodings

All R instructions use opcode `0x00`.

| Mnemonic | Function | Operands | Required zero | Result |
|---|---:|---|---|---|
| `SLL/SRL/SRA` | `00/02/03` | `rd,rt,shamt` | `rs` | fixed shift |
| `SLLV/SRLV/SRAV` | `04/06/07` | `rd,rt,rs` | `shamt` | shift by `rs[4:0]` |
| `JR` | `08` | `rs` | `rt,rd,shamt` | jump to `rs` |
| `JALR` | `09` | `rd,rs` | `rt,shamt` | link then jump |
| `MFHI/MFLO` | `10/12` | `rd` | `rs,rt,shamt` | read special register |
| `MTHI/MTLO` | `11/13` | `rs` | `rt,rd,shamt` | write special register |
| `MULT/MULTU` | `18/19` | `rs,rt` | `rd,shamt` | 64-bit product in `HI:LO` |
| `DIV/DIVU` | `1A/1B` | `rs,rt` | `rd,shamt` | remainder `HI`, quotient `LO` |
| `ADD/ADDU` | `20/21` | `rd,rs,rt` | `shamt` | checked signed/wrapping add |
| `SUB/SUBU` | `22/23` | `rd,rs,rt` | `shamt` | checked signed/wrapping subtract |
| `AND/OR/XOR/NOR` | `24/25/26/27` | `rd,rs,rt` | `shamt` | bitwise result |
| `SLT/SLTU` | `2A/2B` | `rd,rs,rt` | `shamt` | signed/unsigned comparison |

Word zero is canonical `NOP` (`SLL R0,R0,0`). I/J opcodes are:

| Mnemonic | Opcode | Operands | Immediate |
|---|---:|---|---|
| `BLTZ/BGEZ` | `01`, `rt=0/1` | `rs,target` | signed branch displacement |
| `J/JAL` | `02/03` | `target` | 26-bit word target |
| `BEQ/BNE` | `04/05` | `rs,rt,target` | signed branch displacement |
| `ADDI/ADDIU` | `08/09` | `rt,rs,imm` | sign-extended |
| `SLTI/SLTIU` | `0A/0B` | `rt,rs,imm` | sign-extended; chosen comparison |
| `ANDI/ORI/XORI` | `0C/0D/0E` | `rt,rs,imm` | zero-extended |
| `LUI` | `0F` | `rt,imm` | zero-extended; `rs=0` required |
| `LB/LH/LW` | `20/21/23` | `rt,offset(rs)` | sign-extended offset and load |
| `LBU/LHU` | `24/25` | `rt,offset(rs)` | sign-extended offset, unsigned load |
| `SB/SH/SW` | `28/29/2B` | `rt,offset(rs)` | sign-extended offset |
| `HALT` | `3E` | none | all 26 low bits must be zero |
| `TRAP` | `3F` | `code` | unsigned 26-bit application code |

## Execution semantics

Signed values use two's complement. `ADD`, `ADDI`, and `SUB` trap on signed
overflow. Unsigned-named arithmetic wraps modulo 2^32; `ADDIU` still
sign-extends its immediate. `SLTIU` sign-extends the immediate and then compares
unsigned. Fixed shifts accept `0..31`; variable shifts use only the low five
bits. Arithmetic right shift propagates the sign bit.

`MULT` produces a signed 64-bit product; `MULTU` an unsigned product. Bits
63:32 go to `HI`, bits 31:0 to `LO`. `DIV` truncates toward zero and its
remainder has the dividend's sign. Division by zero traps; signed
`0x80000000 / 0xFFFFFFFF` traps as `division_overflow`. A trapping operation
does not alter destinations or `HI/LO`.

For an instruction at `P`, checked `next=P+4`. A branch target is
`next + (sign_extend(imm)<<2)`, evaluated without 32-bit wrap. A jump target is
`(next & 0xF0000000) | (target<<2)`. `JR/JALR` use all bits of `rs`. Only a
taken target is checked. It must be aligned, mapped, and executable. Link value
is `next`; target validation occurs before the link register changes. There are
no delay slots.

Effects commit atomically after validation. A trap leaves registers, memory,
`HI`, `LO`, and `PC` unchanged but charges cycles. `HALT` sets `PC=next`, charges
one cycle, and enters `halted`.

## Memory and traps

Memory is little-endian. Byte accesses are unaligned; halfwords require
two-byte and words four-byte alignment. Effective address calculation is
checked signed-offset addition and never wraps. An access must fit one mapped
slot and region. Loads sign-extend only for `LB/LH`. Stores use the low 8, 16,
or 32 register bits. A load to `R0` still performs all checks.

| Range | Use | Ordinary firmware permission |
|---|---|---|
| `00000000..000FFFFF` | program | execute/read |
| `10000000..100FFFFF` | data | manifested read/write |
| `20000000..200FFFFF` | stack | manifested-bound read/write |
| `40000000..400FFFFF` | telemetry | manifested read |
| `50000000..500FFFFF` | actuator requests | manifested write |
| `60000000..600FFFFF` | feedback | manifested read |
| `70000000..7000FFFF` | supervisor state | manifested read |
| `F0000000..F000FFFF` | safety control | supervisor only |

Region membership does not map a slot or grant a capability. Compiled scenario
slots, manifest requests, and system policy intersect; policy can only reduce
access. All gaps are unmapped.

Step precedence is: running-state API check; PC alignment; fetch mapping;
execute permission; decode; reserved fields; checked `next`; instruction checks;
atomic commit. Data checks are: effective-address range, alignment, whole-width
range, mapping, single-slot containment, supervisor prohibition, mapping
permission, manifest capability. Control targets check arithmetic, alignment,
mapping, then execute permission. Division checks zero before signed overflow.

Stable traps are `pc_misaligned`, `execute_unmapped`, `execute_permission`,
`illegal_instruction`, `reserved_encoding`, `execute_address_overflow`,
`target_misaligned`, `target_unmapped`, `target_execute_permission`,
`arithmetic_overflow`, `division_by_zero`, `division_overflow`,
`data_address_overflow`, `data_misaligned`, `data_unmapped`,
`data_crosses_mapping`, `supervisor_access`, `read_permission`,
`write_permission`, `capability_violation`, and `explicit_trap(code)`.

## Virtual cycles

Cycles are deterministic model units, not time or WCET. Simple ALU, shifts,
special-register moves, branches, jumps, `LUI`, `HALT`, and `TRAP` cost 1;
loads/stores cost 2; multiply costs 4; divide costs 12. A decoded fault charges
its instruction cost; a fetch/decode fault costs 1. A VM budget check occurs
before an instruction and refuses to start one whose full cost would exceed it.

## Assembly contract

Source is UTF-8; identifiers are ASCII `[A-Za-z_][A-Za-z0-9_.]*`. Mnemonics
and registers are case-insensitive; labels are case-sensitive. `#` and `;`
start comments. Literals are decimal, `0x` hex, or `0b` binary with optional
internal underscores and a sign where allowed. Memory syntax is `offset(base)`.
Expressions are a literal or label with one optional literal addend/subtrahend;
`hi16(expr)` and `lo16(expr)` select halves. All arithmetic and field fitting is
checked.

The assembler receives an aligned origin. Labels are absolute byte addresses.
Branch labels must produce an aligned signed-16-bit word displacement; J targets
must share the 256 MiB region selected by `PC+4`. Directives are `.word expr`,
`.zero count` (nonnegative multiple of four), and one `.entry label`.

Pseudo-instructions expand canonically: `NOP` to `SLL R0,R0,0`; `MOVE rd,rs`
to `ADDU rd,rs,R0`; `B target` to `BEQ R0,R0,target`; `RET` to `JR RA`; and
`LI/LA rt,expr` to `LUI rt,hi16(expr)` followed by `ORI rt,rt,lo16(expr)`.
`LI/LA` always use two words, avoiding layout relaxation. Diagnostics require
line, column, and a stable actionable code.

## Golden vectors

Each word and its stored bytes is normative and covers one real instruction.

```text
SLL  00021900 00 19 02 00   SRL  00021902 02 19 02 00
SRA  00021903 03 19 02 00   SLLV 00221804 04 18 22 00
SRLV 00221806 06 18 22 00   SRAV 00221807 07 18 22 00
JR   00200008 08 00 20 00   JALR 00201809 09 18 20 00
MFHI 00001810 10 18 00 00   MTHI 00200011 11 00 20 00
MFLO 00001812 12 18 00 00   MTLO 00200013 13 00 20 00
MULT 00220018 18 00 22 00   MULTU 00220019 19 00 22 00
DIV  0022001A 1A 00 22 00   DIVU 0022001B 1B 00 22 00
ADD  00221820 20 18 22 00   ADDU 00221821 21 18 22 00
SUB  00221822 22 18 22 00   SUBU 00221823 23 18 22 00
AND  00221824 24 18 22 00   OR   00221825 25 18 22 00
XOR  00221826 26 18 22 00   NOR  00221827 27 18 22 00
SLT  0022182A 2A 18 22 00   SLTU 0022182B 2B 18 22 00
BLTZ 04200001 01 00 20 04   BGEZ 04210001 01 00 21 04
J    08000004 04 00 00 08   JAL  0C000004 04 00 00 0C
BEQ  10220001 01 00 22 10   BNE  14220001 01 00 22 14
ADDI 2022FFFF FF FF 22 20   ADDIU 2422FFFF FF FF 22 24
SLTI 2822FFFF FF FF 22 28   SLTIU 2C22FFFF FF FF 22 2C
ANDI 302200FF FF 00 22 30   ORI  342200FF FF 00 22 34
XORI 382200FF FF 00 22 38   LUI  3C021234 34 12 02 3C
LB   80220004 04 00 22 80   LH   84220004 04 00 22 84
LW   8C220004 04 00 22 8C   LBU  90220004 04 00 22 90
LHU  94220004 04 00 22 94   SB   A0220004 04 00 22 A0
SH   A4220004 04 00 22 A4   SW   AC220004 04 00 22 AC
HALT F8000000 00 00 00 F8   TRAP FC000001 01 00 00 FC
```

Operands are `R3,R2,4` for fixed shifts; `R3,R2,R1` for variable shifts;
`R1` for `JR/MTHI/MTLO`; `R3,R1` for `JALR`; `R3` for moves from `HI/LO`;
`R1,R2` for multiply/divide; `R3,R1,R2` for ALU; `R1,+1` for `BLTZ/BGEZ`;
target `0x10` for jumps; `R1,R2,+1` for equality branches; `R2,R1,-1` for
signed-immediate arithmetic/comparison; `R2,R1,0xFF` for logical immediates;
`R2,0x1234` for `LUI`; `R2,4(R1)` for memory; and code 1 for `TRAP`.

Execution goldens: shifts of `R2=80000001` by 4 yield `00000010`, `08000000`,
and `F8000000`; signed/unsigned `MULT` of `FFFFFFFE*3` yields
`FFFFFFFF:FFFFFFFA`/`00000002:FFFFFFFA`; signed `-7/3` yields quotient `-2`,
remainder `-1`; unsigned `7/3` yields `2`, remainder `1`. ALU inputs `R1=1,
R2=2` produce add 3 and subtract `FFFFFFFF`; comparison inputs `FFFFFFFF,1`
produce signed 1 and unsigned 0. Taken `+1` branches from zero reach `PC=8`;
jumps reach `0x10` and links contain 4. Loads from bytes `80`, `00 80`, and
`78 56 34 12` produce the specified signed/unsigned extensions and `12345678`;
stores write the corresponding low bytes. `HALT` ends with `PC=4`; `TRAP 1`
leaves `PC=0`. Tests must also cover all overflow, division, reserved-field,
unknown-word, target, memory-precedence, `R0`, backward-branch, and cycle edges.

Any incompatible encoding, semantic, trap-priority, reset, cycle, or expansion
change requires a new ISA identifier and accepted decision record. Assigning a
previously illegal word is incompatible. Editorial clarifications may retain v0.
