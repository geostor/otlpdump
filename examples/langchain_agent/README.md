# langchain_agent

A minimal LangChain agent with two tools (`get_current_time` and `add`) that sends traces to [otlpdump](../../crates/otlpdump/README.md) using OpenTelemetry zero-code auto-instrumentation. `main.py` has no tracing code; everything is configured with environment variables in [.env](.env).

## Run

Start otlpdump from the repo root:

```sh
cargo run --release -p otlpdump
```

Then run the agent:

```sh
cd examples/langchain_agent
export OPENAI_API_KEY=sk-...
uv run --env-file .env opentelemetry-instrument python main.py
```

A helper script is also available with:
```sh
cd examples/langchain_agent
export OPENAI_API_KEY=sk-...
./run.sh
```

The spans show up as JSON lines in the otlpdump output.

## What gets traced

- **LangChain**: agent, model, and tool spans from LangSmith's OpenTelemetry integration (`LANGSMITH_OTEL_ENABLED`)
- **OpenAI**: GenAI spans from `opentelemetry-instrumentation-openai-v2`, including prompt and completion content
- **HTTP**: outgoing `httpx` requests to the OpenAI API

## Configuration

To change any setting, edit `.env`:

| Variable | Purpose |
| --- | --- |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | Where to send data (otlpdump listens on `:4318`) |
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf`; the distro defaults to gRPC otherwise |
| `OTEL_EXPORTER_OTLP_HEADERS` | Add one if your backend needs auth, e.g. `api-key=...` |
| `OTEL_METRICS_EXPORTER` / `OTEL_LOGS_EXPORTER` | Set to `otlp` to also send metrics and logs |
| `OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT` | Set to `false` to leave prompts and completions out of spans |
| `LANGSMITH_OTEL_ONLY` | Keeps LangChain traces in OTel only, so nothing goes to the LangSmith API |
