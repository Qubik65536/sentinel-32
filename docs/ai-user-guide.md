# Advisory AI user guide

Sentinel-32 uses AI only to compare a bounded state snapshot with versioned
written rules. The result is untrusted operator guidance. Deterministic
scenario rules, safe-state resolution, output gating, firmware validation, and
deployment remain authoritative and continue when the AI service is absent.

## Deployment layout

Run `llama-server` on the deployment server and `sentinel-app ai-check` on the
QNX target or a development host. The server listens on all deployment-server
interfaces at port 8080. A client must use that server's real DNS name or IP;
`0.0.0.0` is only a bind address and the client rejects it.

The connection is authenticated HTTP and is not encrypted. Keep port 8080 on
an isolated trusted lab network or carry it through an authenticated VPN or
tunnel. Do not expose it to an untrusted network. There is no OpenAI or other
remote fallback in the deployment profile.

## Start the deployment server

Install a known `llama.cpp` build and record its version:

```sh
llama-server --version
sha256sum "$HOME/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf"
```

Compare the model digest with the expected model when an independently approved
value is available. The repository includes `config/ai-llama-default.env` and
the local checkout has a mode-0600, gitignored
`config/ai-llama-runtime.env` containing the supplied server credential. Start
the server from the repository root:

```sh
./scripts/start-llama-server.sh
```

For a fresh checkout, create the ignored runtime file without placing the key
in shell history:

```sh
umask 077
read -rsp 'llama.cpp API key: ' S32_LLAMA_API_KEY; echo
printf 'S32_LLAMA_API_KEY=%s\n' "$S32_LLAMA_API_KEY" \
  >config/ai-llama-runtime.env
unset S32_LLAMA_API_KEY
```

The wrapper verifies the GGUF hash before launch and starts the equivalent of:

```sh
llama-server \
  -m "$HOME/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf" \
  --device BLAS -ngl 0 -c 4096 \
  --host 0.0.0.0 --port 8080 \
  --alias qwen2.5-1.5b
```

It supplies the API key through llama.cpp's `LLAMA_API_KEY` environment
variable so the value does not appear in the process command line. On first
start, it computes the GGUF SHA-256 and writes it beside the key in the local
runtime file. Later starts reject a different model digest. Startup prints the
model identity and digest, never the key.

## Run the rocket advisory sample

On the same host, the CLI automatically loads both configuration files, so the
default health and sample commands are:

```sh
cargo run -p sentinel-app -- ai-health deployment
cargo run -p sentinel-app -- \
  ai-check deployment examples/ai/rocket-pressure-snapshot.json
```

`ai-health` checks `/health` and authenticated `/props`, then prints the
server build, model alias, and configured model hash. `ai-check` validates the
snapshot and written-rule hashes locally, requests strict JSON from
`/v1/chat/completions`, reconstructs provenance locally, and rejects malformed
output, missing findings, invented rule IDs, invalid field citations, or
over-size output. The sample is tied to
`examples/rocket-launch-default.yaml` bundle hash
`72975636c2b7c0db7931bc09defcd3ce27adc3ab98f76fa4d908128f94c25388`.

To validate the complete path without a model server, run the deterministic
fixture:

```sh
export S32_AI_CHECK_MODE=fixture
export S32_AI_RULES_PATH=examples/ai/rocket-written-rules.json
export S32_AI_FIXTURE_RESPONSE_PATH=examples/ai/rocket-fixture-response.json
cargo run -p sentinel-app -- \
  ai-check development examples/ai/rocket-pressure-snapshot.json
```

The expected fixture output contains two `possible_violation` findings. These
findings do not assert that a deterministic rule fired and cannot change the
rocket runner's output.

## Run on QNX

The upload helper transfers the release binary, non-secret default config,
rocket scenario, assembly controller, and AI sample files to
`/data/home/qnxuser/sentinel-32`. Securely provision the generated runtime file
at `/data/home/qnxuser/sentinel-32/config/ai-llama-runtime.env` with mode 0600;
it is deliberately excluded from the upload artifact. From a QNX shell,
override only the server address and run:

```sh
cd /data/home/qnxuser/sentinel-32
export S32_LLAMA_BASE_URL=http://<deployment-server-ip>:8080

./release/sentinel-app ai-health deployment
./release/sentinel-app ai-check deployment \
  /data/home/qnxuser/sentinel-32/examples/ai/rocket-pressure-snapshot.json
./release/sentinel-app rocket-run \
  /data/home/qnxuser/sentinel-32/examples/rocket-launch-default.yaml \
  /data/home/qnxuser/sentinel-32/examples/rocket-controller.asm 10000
```

Run the AI check and deterministic rocket run as separate operations. Stopping
`llama-server`, using a wrong key, or
blocking port 8080 must make `ai-health`/`ai-check` fail while `rocket-run`
continues to reach its deterministic result.

## OpenAI development check

The OpenAI adapter is compile-time optional and forbidden by the deployment
profile. It sends the same prompt contract through the Responses API with a
strict JSON schema and `store: false`.

```sh
export S32_AI_CHECK_MODE=openai_test
export S32_AI_RULES_PATH=examples/ai/rocket-written-rules.json
export S32_OPENAI_TEST_MODEL=<approved-test-model>
read -rsp 'OpenAI API key: ' OPENAI_API_KEY; echo
export OPENAI_API_KEY
cargo run -p sentinel-app --features openai-test -- \
  ai-check development examples/ai/rocket-pressure-snapshot.json
```

Never enable this feature in a deployment build. The adapter has no path to
activation, control outputs, deterministic policy, or safety evidence.

## Failure interpretation

- `401` or `403`: client and server API keys differ.
- connection or DNS error: verify the deployment-server address and firewall;
  do not use `0.0.0.0` as the client host.
- model identity mismatch from `/props`: the configured alias does not match
  the running server.
- contract error: the model returned structurally or semantically invalid
  findings; discard the whole advisory result.
- timeout or response-size error: reduce load or investigate the server. Do not
  increase limits without reviewing the bounded-data assumptions.

Qwen2.5 1.5B can fail the strict output contract. Such output is rejected and
reported as checker unavailable; it is never repaired into an authoritative
result.

## Cleanup and records

Stop `llama-server` with `Ctrl-C`, then clear any temporary environment
overrides:

```sh
unset S32_LLAMA_API_KEY OPENAI_API_KEY
```

For reproducible evidence, record the `llama-server --version` output, model
source and license, independently approved GGUF SHA-256, Sentinel commit, QNX
image, exact commands, exit status, and observed output. Do not record either
credential.

The protocol choices follow the official
[`llama-server` documentation](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
and the official
[OpenAI Responses API reference](https://developers.openai.com/api/reference/resources/responses/methods/create).
