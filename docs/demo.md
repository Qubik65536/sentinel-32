# Demonstration plan

**Status:** planned. No Sentinel-32 feature or QNX target step in this narrative has been implemented yet.

The intended demonstration runs approved S32 firmware in the rocket launch digital twin; steps through source, registers, `HI`, `LO`, `PC`, memory, and cycles; rejects unsafe firmware with a counterexample; compares a validated candidate in shadow; and activates it through the deterministic deployment gate. An authenticated deployment-server `llama.cpp` checker separately compares a bounded current-state snapshot with the versioned written rules and reports a rule-linked possible violation. The demonstration then shows the deterministic safety monitor containing that condition regardless of the AI result, corrupts a byte to show hash rejection, stops the checker to show control independence, and hangs the controller to show watchdog containment and compatible rollback.

`DOC-001` must first provide the setup guide for the host, QNX target, deployment-server `llama.cpp` service, pinned GGUF model and hash, isolated-network launch, OpenAI test-only configuration, and smoke checks. `DOC-002` then records target transfer and startup, prepared rule/scenario/artifact hashes, operator commands, expected visible results, timing, reset/recovery steps, and fallback behavior. Every segment must link to passing evidence. The presentation must call the rocket a normalized digital twin, bounded exploration finite testing, target timing observed rather than proven WCET, and AI findings advisory.

The fallback interface is versioned text/NDJSON. The browser dashboard and Scenario Studio enhance presentation but cannot become dependencies of active control or safety monitoring.
