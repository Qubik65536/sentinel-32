# DEC-008: Codex uses repository instructions and independent checks

- **Status:** Accepted
- **Decision:** Codex is the primary development agent and follows root `AGENTS.md`, the dependency-aware plan, canonical docs, and recorded decisions. Its output remains untrusted until reviewed and checked.
- **Consequences:** Task state and constraints survive sessions. Material changes need acceptance criteria and command evidence; trusted-base changes need human review. Agent statements never replace build/test/target evidence.
- **Revisit when:** The development workflow changes, while preserving repository-owned context and independent verification.
