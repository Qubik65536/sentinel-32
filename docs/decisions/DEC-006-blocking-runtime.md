# DEC-006: Blocking processes and threads for the MVP

- **Status:** Accepted
- **Decision:** Prefer explicit QNX processes, blocking I/O, and small versioned protocols over a large async runtime.
- **Consequences:** Dependency and scheduling behavior stay easier to inspect and cross-compile. Slow operations need dedicated low-priority workers and bounded queues so they never block control.
- **Revisit when:** Measurements show an async design is necessary and its QNX dependency, failure, cancellation, and timing behavior has been demonstrated.
