# Standalone S32 sample-analysis demonstration.
# Assemble with: cargo run -p sentinel-app -- assemble examples/sample-analysis.asm
# Run with:      cargo run -p sentinel-app -- run examples/sample-analysis.asm 256
#
# The program builds a five-word sample buffer on the stack, calls a function
# to calculate its sum, maximum, and threshold count, then uses HI/LO to retain
# the integer average and remainder. It needs no scenario, MMIO, or AI service.
#
# Final results:
#   r2 = 66   sum of [12, 7, 25, 4, 18]
#   r3 = 25   maximum sample
#   r7 = 3    samples greater than or equal to the threshold 10
#   r8 = 13   integer average
#   r9 = 1    division remainder

.entry start

start:
    # Reserve an aligned 32-byte stack frame. The standalone runner maps a
    # writable stack and initializes sp to the byte immediately above it.
    addiu sp, sp, -32

    # Build the input array in the frame. Canonical li expands to lui + ori;
    # sw then writes each completed 32-bit value to stack memory.
    li    r1, 12
    sw    r1, 0(sp)
    li    r1, 7
    sw    r1, 4(sp)
    li    r1, 25
    sw    r1, 8(sp)
    li    r1, 4
    sw    r1, 12(sp)
    li    r1, 18
    sw    r1, 16(sp)

    # Pass pointer, count, and threshold in r4-r6. Preserve the original count
    # in r11 because analyze_samples consumes r5 while walking the buffer.
    move  r4, sp
    li    r5, 5
    move  r11, r5
    li    r6, 10
    jal   analyze_samples

    # Unsigned division writes quotient and remainder to the special LO and HI
    # registers. Copy them into general registers so all results stay visible.
    divu  r2, r11
    mflo  r8
    mfhi  r9

    # Release the frame and finish with the stack pointer restored.
    addiu sp, sp, 32
    halt

# Inputs:
#   r4 = address of the next sample
#   r5 = number of samples remaining
#   r6 = inclusive threshold
# Outputs:
#   r2 = sum, r3 = maximum, r7 = threshold count
analyze_samples:
    li    r2, 0
    li    r3, 0
    li    r7, 0

sample_loop:
    beq   r5, r0, analysis_done
    lw    r1, 0(r4)
    addu  r2, r2, r1

    # Replace the maximum only when the current sample is larger.
    sltu  r10, r3, r1
    beq   r10, r0, maximum_done
    move  r3, r1

maximum_done:
    # slt reports sample < threshold. Values for which that is false meet the
    # inclusive threshold and increment r7.
    slt   r10, r1, r6
    bne   r10, r0, threshold_done
    addiu r7, r7, 1

threshold_done:
    addiu r4, r4, 4
    addiu r5, r5, -1
    b     sample_loop

analysis_done:
    ret
