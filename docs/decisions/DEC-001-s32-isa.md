# DEC-001: Custom fixed-width S32 ISA

- **Status:** Accepted
- **Decision:** Use a project-specific MIPS-inspired 32-bit ISA with 32 GPRs, `HI`, `LO`, `PC`, fixed-width instructions, deterministic cycles, and no branch delay slots. It is not MIPS compatible.
- **Consequences:** The project owns encoding, assembler, emulator, traps, tools, and compatibility. Regular formats simplify decoding and display; custom semantics require exhaustive vectors and precise documentation.
- **Revisit when:** ISA-001 finds a requirement that cannot be expressed safely, or versioning data shows an incompatible v1 is justified. Human review is required.
