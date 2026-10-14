# otlptools

Small command-line tools for looking at OpenTelemetry data locally.

`otlpdump` receives OTLP from your app and writes it out as JSON lines. Save its output with `tee`, then use the other tools to search it:

```sh
otlpdump | tee capture.jsonl
otlpfind capture.jsonl --service checkout --status error
otlpfind capture.jsonl --trace 03a3fd794863282240627fd3ad8135d2 --tree
```

| Tool | What it does |
| --- | --- |
| [otlpdump](crates/otlpdump/README.md) | Receives OTLP over HTTP and prints each request as a line of JSON. The only tool that listens for data. |
| [otlpfind](crates/otlpfind/README.md) | Finds spans, logs, and metrics in otlpdump output by trace, service, name, attributes, status, severity, duration, or time. |

## Building

This is a Cargo workspace. Each tool is its own crate under `crates/`, and the code they share lives in `crates/otlp-core`. You need a Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
cargo build --release                      # builds every tool into target/release/
cargo run --release -p otlpfind -- --help  # run one tool without installing
cargo test                                 # run the tests
```

## Installing

```sh
cargo install --path crates/otlpdump
cargo install --path crates/otlpfind
```

## Examples

Two small agents in `examples/` send traces to otlpdump:

- [examples/langchain_agent](examples/langchain_agent/README.md): a LangChain agent
- [examples/strands_agent](examples/strands_agent/README.md): a Strands agent
