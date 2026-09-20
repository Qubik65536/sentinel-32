# Minimal S32 control-flow example.
# Assemble with: cargo run -p sentinel-app -- assemble examples/countdown.asm
# Run with:      cargo run -p sentinel-app -- run examples/countdown.asm 9
#
# li and b are assembler pseudo-instructions: they expand into real encoded
# instructions described in docs/s32-isa.md. Labels and .entry are assembler
# metadata and do not occupy instruction memory.

# Begin execution at the instruction labeled `start`.
.entry start

start:
    # Put decimal 3 in r1. Canonical li always expands to lui followed by ori,
    # even for a small value, so this line executes as two real instructions.
    li r1, 3

countdown:
    # Subtract one from r1. addiu wraps on overflow and sign-extends -1.
    addiu r1, r1, -1

    # Branch back while r1 differs from the hardwired-zero register r0.
    # S32 has no branch delay slot.
    bne r1, r0, countdown

    # r1 is now zero. halt ends execution successfully.
    halt
