# Advisory AI user guide

Sentinel-32 uses AI only to compare a bounded state snapshot with versioned
written rules. The result is untrusted operator guidance. Deterministic
scenario rules, safe-state resolution, output gating, firmware validation, and
deployment remain authoritative and continue when the AI service is absent.

## Deployment layout

Run `llama-server` and `sentinel-app ai-check` on the QNX deployment host. The
server listens on all interfaces at port 8080, while the checker connects to
`127.0.0.1:8080`. `0.0.0.0` is only a bind address and the client rejects it.

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
value is available. The repository includes
`config/ai-llama-default.env.example`. The effective configuration is the
mode-0600, gitignored `config/ai-llama-default.env`, which contains the
localhost endpoint, absolute QNX sample paths, supplied server credential, and
model identity. Start the server from the repository root:

```sh
./scripts/start-llama-server.sh
```

For a fresh checkout, create the ignored default file without placing the key
in shell history:

```sh
umask 077
read -rsp 'llama.cpp API key: ' S32_LLAMA_API_KEY; echo
cp config/ai-llama-default.env.example config/ai-llama-default.env
sed -i "s/S32_LLAMA_API_KEY=replace-locally/S32_LLAMA_API_KEY=$S32_LLAMA_API_KEY/" \
  config/ai-llama-default.env
chmod 600 config/ai-llama-default.env
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
start, it computes the GGUF SHA-256 and updates it in the local default file.
Later starts reject a different model digest. Startup prints the model identity
and digest, never the key.

## Run the rocket advisory sample

On the same host, the CLI automatically loads the default configuration, so the
default health and sample commands are:

```sh
cargo run -p sentinel-app -- ai-health deployment
S32_AI_RULES_PATH=examples/ai/rocket-written-rules.json \
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

The upload helper transfers the release binary, non-secret default example,
rocket scenario, assembly controller, and AI sample files to
`/data/home/qnxuser/sentinel-32`. Securely provision the generated default file
at `/data/home/qnxuser/sentinel-32/config/ai-llama-default.env` with mode 0600;
it is deliberately excluded from the upload artifact. From the release
directory on QNX, use the absolute configuration path:

```sh
cd /data/home/qnxuser/sentinel-32/release
export S32_AI_CONFIG_PATH=/data/home/qnxuser/sentinel-32/config/ai-llama-default.env

./sentinel-app ai-health deployment
./sentinel-app ai-check deployment \
  /data/home/qnxuser/sentinel-32/examples/ai/rocket-pressure-snapshot.json
./sentinel-app rocket-run \
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
- connection error: verify that local `llama-server` is listening on port 8080;
  use `127.0.0.1`, never `0.0.0.0`, as the client host.
- model identity mismatch from `/props`: the configured alias does not match
  the running server.
- contract error: the model returned structurally or semantically invalid
  findings; discard the whole advisory result.
- `InvalidProviderResponse` after a `stop` completion: capture the raw response
  without its credential. The client accepts plain schema JSON and the exact
  whole-response Markdown JSON fence observed from the QNX server; any
  other preamble, trailing text, or malformed wrapper remains invalid.
- timeout or response-size error: reduce load or investigate the server. Do not
  increase limits without reviewing the bounded-data assumptions.

Qwen2.5 1.5B can fail the strict output contract. Such output is rejected and
reported as checker unavailable; it is never repaired into an authoritative
result. The deployment request disables model reasoning, caps citation-array
and rationale lengths, and reports a token-limited completion separately from
malformed output.

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
