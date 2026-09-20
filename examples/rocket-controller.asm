# Single-invocation normalized rocket controller.
#
# An operator starts this educational controller once. The same S32 invocation
# remains active through loading, stabilization, arming, terminal count,
# ignition confirmation, and shutdown. The QNX runner supplies the declarative
# software twin and simulated supervisor approvals; this firmware owns the
# operational thresholds, waits, loops, interlock checks, and actuator requests.
# It is not a real launch procedure and its normalized values are not physical
# launch parameters.
#
# Valve enum values: closed=0, open=1.
# Ignition enum values: safe=0, armed=1, firing=2.
# Pressure uses integer milli-units. The firmware target is 50000.
# A complete command frame ends with the ignition write. The runner applies one
# scenario tick after that write and refreshes readable telemetry and feedback.

.entry start

start:
    la    r10, S32_TELEMETRY_FUEL_PRESSURE
    la    r11, S32_TELEMETRY_OXIDIZER_PRESSURE
    la    r12, S32_TELEMETRY_SEQUENCE_TIMER
    la    r13, S32_TELEMETRY_CONTROLLER_POWER
    la    r14, S32_TELEMETRY_PRIMARY_POWER
    la    r15, S32_TELEMETRY_BACKUP_POWER
    la    r16, S32_TELEMETRY_IGNITION_CONTINUITY
    la    r17, S32_TELEMETRY_FLIGHT_READY
    la    r18, S32_TELEMETRY_RANGE_CLEAR
    la    r19, S32_TELEMETRY_PAD_CLEAR
    la    r20, S32_TELEMETRY_REMOTE_INHIBIT

    la    r21, S32_ACTUATOR_FUEL_FILL_VALVE
    la    r22, S32_ACTUATOR_OXIDIZER_FILL_VALVE
    la    r23, S32_ACTUATOR_FUEL_MAIN_VALVE
    la    r24, S32_ACTUATOR_OXIDIZER_MAIN_VALVE
    la    r25, S32_ACTUATOR_VENT_VALVE
    la    r26, S32_ACTUATOR_IGNITION
    la    r27, S32_FEEDBACK_IGNITION_FEEDBACK

    li    r3, 50000

loading:
    # Fill each normalized pressure channel only while it is below the
    # firmware-owned target. Main valves, vent, and ignition stay safe.
    lw    r1, 0(r10)
    lw    r2, 0(r11)
    slt   r4, r1, r3
    slt   r5, r2, r3
    sw    r4, 0(r21)
    sw    r5, 0(r22)
    sw    r0, 0(r23)
    sw    r0, 0(r24)
    sw    r0, 0(r25)
    sw    r0, 0(r26)
    bne   r4, r0, loading
    bne   r5, r0, loading

    # Two closed command frames allow the simulated one-tick valve feedback
    # delay to settle before requesting the armed state.
    li    r6, 2
stabilize:
    sw    r0, 0(r21)
    sw    r0, 0(r22)
    sw    r0, 0(r23)
    sw    r0, 0(r24)
    sw    r0, 0(r25)
    sw    r0, 0(r26)
    addiu r6, r6, -1
    bne   r6, r0, stabilize

    # Every required electrical, continuity, readiness, and clearance input is
    # checked by firmware. Any missing input takes the assembly abort path.
    lw    r1, 0(r13)
    beq   r1, r0, abort
    lw    r1, 0(r14)
    lw    r2, 0(r15)
    bne   r1, r0, buses_ready
    beq   r2, r0, abort
buses_ready:
    lw    r1, 0(r16)
    beq   r1, r0, abort
    lw    r1, 0(r17)
    beq   r1, r0, abort
    lw    r1, 0(r18)
    beq   r1, r0, abort
    lw    r1, 0(r19)
    beq   r1, r0, abort
    lw    r1, 0(r20)
    bne   r1, r0, abort

    # Armed ignition requests signal readiness for the runner's explicit
    # simulated arm and terminal-count supervisor approvals.
request_arm:
    li    r7, 1
    sw    r0, 0(r21)
    sw    r0, 0(r22)
    sw    r0, 0(r23)
    sw    r0, 0(r24)
    sw    r0, 0(r25)
    sw    r7, 0(r26)

terminal_count:
    # The scenario owns the observable timer. Firmware waits for five ticks and
    # keeps all valves closed throughout the terminal count.
    lw    r1, 0(r12)
    slti  r4, r1, 5
    beq   r4, r0, ignite
    sw    r0, 0(r21)
    sw    r0, 0(r22)
    sw    r0, 0(r23)
    sw    r0, 0(r24)
    sw    r0, 0(r25)
    sw    r7, 0(r26)
    b     terminal_count

ignite:
    # Open both main valves and request firing only after the observed terminal
    # count completes. Repeat until simulated ignition feedback confirms it.
    li    r8, 1
    li    r9, 2
ignition_wait:
    sw    r0, 0(r21)
    sw    r0, 0(r22)
    sw    r8, 0(r23)
    sw    r8, 0(r24)
    sw    r0, 0(r25)
    sw    r9, 0(r26)
    lw    r1, 0(r27)
    bne   r1, r9, ignition_wait

shutdown:
    # End the bounded demonstration in the declared safe command state. Two
    # frames let the one-tick simulated feedback delay confirm shutdown.
    li    r6, 2
shutdown_wait:
    sw    r0, 0(r21)
    sw    r0, 0(r22)
    sw    r0, 0(r23)
    sw    r0, 0(r24)
    sw    r0, 0(r25)
    sw    r0, 0(r26)
    addiu r6, r6, -1
    bne   r6, r0, shutdown_wait
    halt

abort:
    # Assembly abort request: close propellant paths, open vent, safe ignition.
    # The independent deterministic policy layer may further override outputs.
    li    r8, 1
    sw    r0, 0(r21)
    sw    r0, 0(r22)
    sw    r0, 0(r23)
    sw    r0, 0(r24)
    sw    r8, 0(r25)
    sw    r0, 0(r26)
    halt
