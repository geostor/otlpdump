# otlpdump

Receives OTLP over HTTP and prints traces, metrics, and logs to stdout

It accepts OTLP/HTTP protobuf requests, decodes them, and prints each one as a line of JSON.

## Usage

```sh
cargo run --release -p otlpdump  # listens on 0.0.0.0:4318
cargo run --release -p otlpdump -- 4319  # custom port
```

Point your app's OpenTelemetry SDK at it:

```sh
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
```

Save it to a file with `tee` to search later with [otlpfind](../otlpfind/README.md):

```sh
cargo run --release -p otlpdump | tee capture.jsonl
```

Pipe the output through `jq` for readable JSON:

```sh
cargo run --release -p otlpdump | jq
```

## Endpoints

| Path          | Signal  |
| ------------- | ------- |
| `/v1/traces`  | Traces  |
| `/v1/metrics` | Metrics |
| `/v1/logs`    | Logs    |

## Limitations

- HTTP/protobuf only: no gRPC and no JSON request bodies
- No gzip: set `OTEL_EXPORTER_OTLP_COMPRESSION=none` if your SDK compresses by default
- Single-threaded, with no request size limit. It's meant for local debugging, not as an exposed service
