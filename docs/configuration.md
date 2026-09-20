# Configuration contract

Configuration handling is implemented for the advisory checker CLI and remains planned for the broader runtime. `sentinel-app ai-health` and `ai-check` automatically load `config/ai-llama-default.env` followed by the gitignored `config/ai-llama-runtime.env`; process environment values override both files. `S32_AI_CONFIG_PATH` and `S32_AI_RUNTIME_CONFIG_PATH` select different files. Scenario and VM commands continue to use explicit arguments. Provider configuration cannot alter compiled scenario semantics.

Advisory precedence is process environment, local runtime file, then tracked default file. The parser is bounded, rejects unknown keys and malformed lines, and does not execute file contents. Safety-relevant ambiguity, invalid values, unknown enum variants, and numeric overflow fail closed. Scenario-specific channel definitions, pressure bands, timeouts, and safe states belong in the compiled scenario and cannot be overridden by process environment.

The Raspberry Pi 5 deployment convention uses
`/data/home/qnxuser/sentinel-32`, with `release/`, `examples/`, and `artifacts/`
subdirectories. This is an operator-managed target directory, not a scenario
field or environment override. Commands use absolute paths so their behavior
does not depend on the login directory.

| Name | Purpose | Initial policy |
|---|---|---|
| `S32_AI_CHECK_MODE` | `llama_cpp`, `openai_test`, `fixture`, or `disabled` | Tracked default selects `llama_cpp`; deployment rejects every other mode |
| `S32_AI_RULES_PATH` | Versioned written rule-set input | Required when checking is enabled; content is bounded and its hash is recorded with every finding |
| `S32_AI_CHECK_TIMEOUT_MS` | Advisory check deadline | Positive bounded integer; timeout produces checker-unavailable status and never delays control |
| `S32_AI_MAX_SNAPSHOT_BYTES` | Serialized state-snapshot ceiling | Positive integer no greater than 65536; oversize snapshots are rejected before provider access |
| `S32_AI_MAX_OUTPUT_BYTES` | Structured finding response ceiling | Positive integer no greater than 65536; oversize responses are rejected |
| `S32_LLAMA_BASE_URL` | Authenticated deployment-server `llama.cpp` endpoint | Tracked default is `http://127.0.0.1:8080`; QNX must override it with the server DNS name or IP; never use the `0.0.0.0` bind address |
| `S32_LLAMA_MODEL_ID` | Operator-readable local model identity | Required in `llama_cpp` mode and recorded with findings |
| `S32_LLAMA_MODEL_SHA256` | Expected GGUF file identity | Required in `llama_cpp` mode; mismatch prevents checker startup |
| `S32_LLAMA_API_KEY` | Shared `llama.cpp` credential | Required in `llama_cpp` mode; never logged or included in errors |
| `S32_LLAMA_MODEL_PATH` | Deployment-server GGUF path | Launcher default is `~/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf` |
| `S32_AI_MAX_OUTPUT_TOKENS` | Provider generation limit | Positive integer, default 2048 and maximum 8192 |
| `S32_AI_FIXTURE_RESPONSE_PATH` | Deterministic response fixture | Required only in `fixture` mode |
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

Never commit local runtime files, API keys, or downloaded model weights. The launcher reads the local mode-0600 runtime file, computes the selected GGUF SHA-256, verifies a previously recorded digest when present, and atomically writes the key and observed digest back to that file. Every finding records the backend, server build, model identity/hash, prompt-contract version, written-rule hash, and snapshot identity without authorization headers or unnecessary prompt content. See `docs/ai-user-guide.md` for setup and checks.
