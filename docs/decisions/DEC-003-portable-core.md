# DEC-003: Portable core with isolated QNX code

- **Status:** Accepted
- **Decision:** Keep ISA, assembler, VM, scenario compiler/runtime, and policy/evidence logic portable. Put QNX FFI, timing, scheduling, affinity, IPC, and process control in `sentinel-qnx`.
- **Consequences:** Most logic can be tested rapidly on the host. The adapter owns nearly all unsafe code and exposes safe wrappers. Target semantics still require QNX tests and cannot be inferred from host success.
- **Revisit when:** A measured QNX requirement cannot be represented behind the adapter without compromising deterministic semantics. Human review is required.
