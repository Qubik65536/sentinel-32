# S32 ISA v0 working specification

**Status:** design baseline only. `ISA-001` must complete encodings and executable examples before this becomes an implementation contract.

S32 is project-specific and not binary compatible with MIPS. It uses 32 fixed-width little-endian general registers (`R0..R31`), with writes to `R0` discarded; `R29`, `R30`, and `R31` may be named `SP`, `FP`, and `RA`. `HI`, `LO`, and a four-byte-aligned 32-bit `PC` are dedicated registers. Instructions are 32-bit words. There are no flags or branch delay slots. Every instruction has a deterministic virtual cycle cost.

## Formats

```text
R: opcode[31:26] rs[25:21] rt[20:16] rd[15:11] shamt[10:6] funct[5:0]
I: opcode[31:26] rs[25:21] rt[20:16] immediate[15:0]
J: opcode[31:26] target[25:0]
```

The initial instruction families are `ADD/ADDU/SUB/SUBU/ADDI/ADDIU`; boolean operations; signed/unsigned comparisons; fixed and variable shifts; `LUI`; signed/unsigned multiply/divide with `HI:LO`; byte/halfword/word loads and stores; `BEQ/BNE/BLTZ/BGEZ`; `J/JAL/JR/JALR`; and `NOP/HALT/TRAP`.

Signed arithmetic traps on overflow where specified; unsigned arithmetic wraps. Division by zero, unknown encodings, misalignment, invalid execute targets, unmapped access, permission violations, and supervisor-region access trap deterministically. Shift masking, division corner cases, jump high-bit behavior, immediate extension rules, exact `PC` update ordering, trap precision, cycle costs, and all opcode/funct values remain unresolved until `ISA-001`.

## Memory classes

| Range | Use | Firmware access |
|---|---|---|
| `0x0000_0000..=0x000F_FFFF` | Program | execute/read |
| `0x1000_0000..=0x100F_FFFF` | Data/heap | read/write |
| `0x2000_0000..=0x200F_FFFF` | Stack | read/write within manifest bound |
| `0x4000_0000..=0x400F_FFFF` | Scenario telemetry | read |
| `0x5000_0000..=0x500F_FFFF` | Actuator requests | write |
| `0x6000_0000..=0x600F_FFFF` | Feedback/diagnostics | read |
| `0x7000_0000..=0x7000_FFFF` | Supervisor state/policy | read |
| `0xF000_0000..=0xF000_FFFF` | Safety/system control | supervisor only |

Only explicitly mapped slots are accessible. Scenario-generated symbols, rather than hand-coded addresses, are the firmware interface.

## ISA-001 completion gate

The task must specify the complete encoding table, reserved bits, assembler grammar and pseudo-instructions, reset state, signedness and extension behavior, branch/jump target calculation, precise traps and precedence, memory access semantics, `HI/LO`, cycle table, and golden encoding/execution vectors. Any later incompatible change requires an ISA version and decision record.
