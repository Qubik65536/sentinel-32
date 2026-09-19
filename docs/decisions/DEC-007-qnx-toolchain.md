# DEC-007: Official QNX-modified Rust toolchain

- **Status:** Accepted; execution evidence pending BUILD-001
- **Decision:** Cross-compile with the QNX SDP 8.0 modified Rust compiler registered as a custom rustup toolchain, targeting `aarch64-unknown-nto-qnx800`.
- **Consequences:** Ordinary upstream Rust or older QNX targets are insufficient. Target-facing dependencies and cfg assumptions require early proof on a supported x86 Linux/Windows host and execution on QNX 8.0/Raspberry Pi 5.
- **Revisit when:** Official QNX documentation changes the supported toolchain or target. Never substitute an unverified target silently.
