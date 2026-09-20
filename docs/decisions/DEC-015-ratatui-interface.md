# DEC-015: Ratatui is the primary interactive interface

- **Status:** Accepted and implemented on 2026-09-20
- **Decision:** Retire the unfinished browser, Scenario Studio, NDJSON,
  lifecycle, verifier, evidence-report, QNX process-splitting, watchdog, and
  provider-parity roadmap. Build one Ratatui terminal application with
  Assemble, Run, Mission, and Advisory views on top of typed application
  sessions. Pin Ratatui 0.29.0 without optional backends and use a small safe
  ANSI/`stty` backend on the host and QNX. Preserve existing CLI workflows for
  automation and diagnosis.
- **Consequences:** Work concentrates on making implemented behavior visible
  and usable. Ratatui code stays in `sentinel-app`; domain crates remain UI
  independent. The custom backend avoids terminal-library FFI, compiles with
  the QNX-modified Rust 1.85.1 toolchain, and requires a console or allocated
  SSH pseudo-terminal. Retired safety and lifecycle work is not implied by the
  TUI.
- **Revisit when:** required terminal capabilities cannot be expressed through
  the safe ANSI backend, or a future Ratatui release changes the Rust/QNX
  compatibility boundary.
