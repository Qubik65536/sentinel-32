# Single-invocation tank controller.
#
# An operator starts this program once. The program remains active and loops
# through the complete operation until the tank has been filled, held, and
# unloaded. The runner updates physical pressure between valve-command pairs;
# it never restarts this program or chooses the next controller state.
#
# Complete assembly-owned sequence:
#   1. Keep inlet open and outlet closed until pressure reaches 50.000.
#   2. Keep both valves closed for exactly 10 simulated seconds.
#   3. Keep inlet closed and outlet open until pressure reaches zero.
#   4. Close both valves and halt.
#
# YAML-generated MMIO symbols:
#   S32_TELEMETRY_TANK_PRESSURE  current tank pressure, read-only
#   S32_ACTUATOR_INLET_VALVE     inlet request, write-only
#   S32_ACTUATOR_OUTLET_VALVE    outlet request, write-only
#
# Valve values come from YAML enum order: closed=0 and open=1.
# Pressure uses integer milli-units, so 50000 means 50.000 pressure units.
# Each completed pair of valve writes represents one simulated second of tank
# response. The ten-second counter is r8, entirely inside this firmware.
#
# Registers:
#   r1  = current pressure
#   r3  = pressure target 50000
#   r4  = comparison result
#   r5  = inlet request
#   r6  = outlet request
#   r8  = remaining hold seconds
#   r10 = pressure-register address
#   r12 = inlet-request address
#   r13 = outlet-request address

.entry start

start:
    # Resolve hardware addresses once when the operator starts the program.
    la    r10, S32_TELEMETRY_TANK_PRESSURE
    la    r12, S32_ACTUATOR_INLET_VALVE
    la    r13, S32_ACTUATOR_OUTLET_VALVE
    li    r3, 50000

load:
    # Re-read the live pressure after every simulated second. Continue filling
    # while pressure is below the firmware-owned 50.000 target.
    lw    r1, 0(r10)
    slt   r4, r1, r3
    beq   r4, r0, begin_hold

    li    r5, 1
    li    r6, 0
    sw    r5, 0(r12)
    sw    r6, 0(r13)
    b     load

begin_hold:
    # The target has been reached. r8 belongs to this still-running program and
    # counts the ten requested hold seconds; YAML contains no hold instruction.
    li    r8, 10

hold:
    # One closed/closed command pair represents one simulated hold second.
    li    r5, 0
    li    r6, 0
    sw    r5, 0(r12)
    sw    r6, 0(r13)
    addiu r8, r8, -1
    bne   r8, r0, hold

unload:
    # Poll pressure after every outlet-flow second. Continue unloading while
    # pressure is greater than zero.
    lw    r1, 0(r10)
    slti  r4, r1, 1
    bne   r4, r0, stopped

    li    r5, 0
    li    r6, 1
    sw    r5, 0(r12)
    sw    r6, 0(r13)
    b     unload

stopped:
    # The complete operation is finished. Leave both valves closed, then halt
    # this one operator-triggered execution.
    li    r5, 0
    li    r6, 0
    sw    r5, 0(r12)
    sw    r6, 0(r13)
    halt
