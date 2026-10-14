# strands_agent

A minimal [Strands](https://strandsagents.com) agent with two tools (`get_current_time` and `add`) that sends traces to [otlpdump](../../crates/otlpdump/README.md). Strands has OpenTelemetry tracing built in, and `opentelemetry-instrument` sets up the exporter, so `main.py` has no tracing code. Everything is configured with environment variables in [.env](.env).

## Run

Start otlpdump from the repo root:

```sh
cargo run --release -p otlpdump
```

Then run the agent:

```sh
cd examples/strands_agent
export OPENAI_API_KEY=sk-...
uv run --env-file .env opentelemetry-instrument python main.py
```

A helper script is also available with:
```sh
cd examples/strands_agent
export OPENAI_API_KEY=sk-...
./run.sh
```

The spans show up as JSON lines in the otlpdump output.

## What gets traced

- **Strands**: agent, event loop cycle, model, and tool spans from Strands' built-in tracing, including prompts, completions, and token usage. Every span in a session has a `session.id` attribute.
- **HTTP**: outgoing `httpx` requests to the OpenAI API

## Configuration

To change any setting, edit `.env`:

| Variable | Purpose |
| --- | --- |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | Where to send data (otlpdump listens on `:4318`) |
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf`; the distro defaults to gRPC otherwise |
| `OTEL_EXPORTER_OTLP_HEADERS` | Add one if your backend needs auth, e.g. `api-key=...` |
| `OTEL_METRICS_EXPORTER` / `OTEL_LOGS_EXPORTER` | Set to `otlp` to also send metrics and logs |
| `OTEL_PYTHON_DISABLED_INSTRUMENTATIONS` | `threading` is turned off because Strands instruments it itself |
| `OTEL_SEMCONV_STABILITY_OPT_IN` | `gen_ai_latest_experimental` makes Strands use the latest GenAI semantic conventions |
