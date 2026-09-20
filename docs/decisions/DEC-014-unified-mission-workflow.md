# DEC-014: Use one mission workflow with pre-operation AI advisory

- **Status:** Accepted 2026-09-20 by explicit operator direction.
- **Decision:** Present every supported hardware-inventory or full-scenario
  operation through `mission-run`. Use `mission-advice` to compare a bounded
  mission snapshot, including any typed proposed-action fields, with that
  mission's versioned written rules before an operator considers execution.
  Keep the advisory and execution as separate commands.
- **Consequences:** Tank and rocket remain examples, not runner types. The
  declared document schema selects the validated v0 adapter. AI can explain
  possible concerns for any mission with a valid snapshot and rule set, but it
  cannot return a proceed decision, gate activation, suppress deterministic
  policy, or write outputs. Provider failure leaves deterministic execution
  available and visibly marks advisory review unavailable.
