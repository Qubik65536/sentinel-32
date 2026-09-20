# DEC-013: Use an authenticated deployment-server llama.cpp endpoint

- **Status:** Accepted
- **Decision:** Run the pinned Qwen2.5 1.5B GGUF under `llama-server` on a deployment server bound to `0.0.0.0:8080`. QNX or development clients use the server's real address and an API key. The port stays on a restricted lab network, the model file is checked against an approved SHA-256 before launch, and deployment never falls back to OpenAI.
- **Consequences:** The deployment topology replaces DEC-011's loopback placement while preserving its advisory-only authority boundary. The API key stays outside source, logs, findings, traces, and process arguments. Plaintext HTTP requires network isolation or an authenticated tunnel. Server loss, authentication failure, invalid output, or timeout makes only the advisory checker unavailable.
- **Revisit when:** TLS terminates directly in the client, the server leaves the trusted lab network, the model changes, or advisory output is proposed for an authoritative path.
