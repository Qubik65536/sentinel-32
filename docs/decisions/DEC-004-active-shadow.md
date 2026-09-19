# DEC-004: Separate active and shadow controllers

- **Status:** Accepted
- **Decision:** Candidates execute without output authority in shadow before activation. Evidence binds exact firmware and compiled scenario hashes; active and last-known-good pairs are retained separately.
- **Consequences:** Deployment becomes an explicit transactional state machine and consumes more resources. Divergence is observable before authority changes. Rollback cannot mix incompatible artifacts or clear policy latches.
- **Revisit when:** Target capacity measurements require a different isolation schedule while preserving the same authority separation and evidence guarantees.
