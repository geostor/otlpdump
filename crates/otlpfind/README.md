# otlpfind

Finds spans, logs, and metrics in [otlpdump](../otlpdump/README.md) output, like `find` for telemetry.

It reads the files you give it, or stdin, and prints the matching records in the same JSON format otlpdump writes. That means you can save the output, pipe it to `jq`, or search it again with otlpfind.

## Usage

```sh
otlpdump | tee capture.jsonl          # in one terminal

otlpfind capture.jsonl --trace 03a3fd794863282240627fd3ad8135d2 --tree
otlpfind capture.jsonl --service checkout --status error
otlpfind capture.jsonl --attr gen_ai.request.model=gpt-4o --min-duration 2s
otlpfind capture.jsonl --has-attr exception.type --since 10m
otlpfind capture.jsonl --attr-value '*PARTIAL*' --tree
tail -f capture.jsonl | otlpfind --severity '>=WARN'
```

## Filters

| Flag | Matches |
| --- | --- |
| `--trace ID` | Trace ID (hex) |
| `--span ID` | Span ID; also matches logs emitted inside that span |
| `--service NAME` | The resource's `service.name`; supports `*` and `?` |
| `--kind span\|log\|metric` | Only that signal |
| `--name GLOB` | Span name, metric name, or log body; supports `*` and `?` |
| `--status unset\|ok\|error` | Span status |
| `--severity LEVEL` | Log severity: `TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`, or `FATAL`, optionally prefixed with `>=` or `<=` |
| `--has-attr KEY` | Has an attribute with this name |
| `--attr KEY=VALUE` | Has this attribute with this value; supports `*` and `?`. `KEY~REGEX` matches the value with a regex |
| `--attr-value VALUE` | Has any attribute with this value; supports `*` and `?`, e.g. `'*timeout*'` |
| `--min-duration`, `--max-duration` | Span duration, e.g. `500ms` or `2s` |
| `--since`, `--until` | A duration ago (`10m`, `2h`) or a UTC timestamp (`2026-10-13T07:00:00Z`) |

All the filters you give must match. Repeating `--trace`, `--span`, `--service`, `--kind`, `--name`, `--status`, or `--severity` matches any of the values you give. Repeating `--has-attr`, `--attr`, or `--attr-value` requires all of them to match.

Attribute filters look at the record's own attributes, then its scope's, then its resource's. For spans, that includes attributes on span events, such as `exception.type`. For metrics, it includes the attributes on every data point.

A filter that only applies to one signal excludes the others. For example, `--status error` matches only spans, and `--severity ERROR` matches only logs.

## Trees

`--tree` prints each trace that contains a match as a tree of spans, with durations, errors, and a few useful attributes such as the model, token counts, and HTTP status. When only some spans in a trace match, those are marked `<- match`. Matching logs are shown under the span they were emitted in.

```
trace 03a3fd794863282240627fd3ad8135d2  3 spans, 1.55s
└─ RunnableSequence  1.55s  [example-agent]  gen_ai.usage.input_tokens=13  gen_ai.usage.output_tokens=14
   ├─ ChatPromptTemplate  303.1µs
   └─ ChatOpenAI  1.53s  gen_ai.request.model=gpt-3.5-turbo  gen_ai.usage.input_tokens=13  gen_ai.usage.output_tokens=14
```

`--tree` waits until all the input has been read, because a trace's spans can arrive in any order.
