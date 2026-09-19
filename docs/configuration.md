# Configuration contract

Configuration handling is scheduled for implementation with the relevant runtime tasks. This document fixes the intended boundary without claiming a general configuration parser. The current `sentinel-app run` lab harness accepts its required positive virtual-cycle budget as a positional command-line argument; it does not yet read `S32_VIRTUAL_CYCLE_BUDGET` from the environment or a configuration file. The scenario commands likewise use explicit source, output, and positive tick-count arguments. Their library API consumes bytes or typed values, so configuration precedence cannot silently alter compiled semantics.

Precedence will be command-line argument, environment variable, versioned configuration file, then documented default. Safety-relevant ambiguity, invalid values, unknown enum variants, and numeric overflow must fail closed. Scenario-specific channel definitions, pressure bands, timeouts, and safe states belong in the compiled scenario and cannot be overridden by process environment.

The Raspberry Pi 5 deployment convention uses
`/data/home/qnxuser/sentinel-32`, with `release/`, `examples/`, and `artifacts/`
subdirectories. This is an operator-managed target directory, not a scenario
field or environment override. Commands use absolute paths so their behavior
does not depend on the login directory.

| Name | Purpose | Initial policy |
|---|---|---|
| `S32_AI_CHECK_MODE` | `llama_cpp`, `openai_test`, `fixture`, or `disabled` | `disabled` by default; hackathon profile must select `llama_cpp`; deployed config must reject `openai_test` |
| `S32_AI_RULES_PATH` | Versioned written rule-set input | Required when checking is enabled; content is bounded and its hash is recorded with every finding |
| `S32_AI_CHECK_TIMEOUT_MS` | Advisory check deadline | Positive bounded integer; timeout produces checker-unavailable status and never delays control |
| `S32_AI_MAX_SNAPSHOT_BYTES` | Serialized state-snapshot ceiling | Positive bounded integer; oversize snapshots are rejected before provider access |
| `S32_AI_MAX_OUTPUT_BYTES` | Structured finding response ceiling | Positive bounded integer; oversize responses are rejected |
| `S32_LLAMA_BASE_URL` | Local `llama.cpp` server endpoint | Loopback HTTP only in the hackathon profile; no remote endpoint fallback |
| `S32_LLAMA_MODEL_ID` | Operator-readable local model identity | Required in `llama_cpp` mode and recorded with findings |
| `S32_LLAMA_MODEL_SHA256` | Expected GGUF file identity | Required in `llama_cpp` mode; mismatch prevents checker startup |
| `OPENAI_API_KEY` | OpenAI test credential | No default; test adapter only; never logged, committed, or sent over Sentinel IPC |
| `S32_OPENAI_TEST_MODEL` | Pinned checker test model | Required only in `openai_test` mode and recorded in test results |
| `S32_CONTROL_PERIOD_US` | Controller period | Positive integer; target value requires QNX measurement |
| `S32_HOST_DEADLINE_US` | Host test deadline | Observational test limit only, never described as WCET |
| `S32_VIRTUAL_CYCLE_BUDGET` | Deterministic S32 execution budget | Required positive integer once VM exists |
| `S32_HEARTBEAT_PERIOD_MS` | Controller heartbeat period | Must be below timeout; target value requires QNX measurement |
| `S32_HEARTBEAT_TIMEOUT_MS` | Watchdog timeout | Must exceed period and have a documented safety response |
| `S32_EXPLORATION_DEPTH` | Bounded search depth | Required finite integer; recorded in evidence |
| `S32_MAX_EXPLORED_STATES` | Bounded search state cap | Required finite integer; recorded in evidence |
| `S32_TRACE_PATH` | Versioned trace output | Must reject unsafe/unwritable destinations cleanly |
| `S32_DASHBOARD_BIND` | UI listen address | Loopback default unless deployment explicitly chooses otherwise |
| `S32_SCENARIO_SOURCE_PATH` | Authoring source input | Compiler/Studio only; never loaded as active runtime policy |
| `S32_SCENARIO_BUNDLE_PATH` | Immutable compiled bundle | Hash and schema version must verify before use |

Never commit `.env` files, API keys, or downloaded model weights. Each enabled adapter must validate its structured-response capability at startup. Every finding records the backend, model identity, prompt-contract version, written-rule version/hash, and snapshot identity without recording authorization headers or unnecessary prompt content. The setup guide will define model acquisition, hash verification, `llama.cpp` launch, OpenAI test configuration, and smoke checks under `DOC-001`.
