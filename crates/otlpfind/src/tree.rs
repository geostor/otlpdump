//! `--tree`: print whole traces as indented span trees.

use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

use otlp_core::attrs::{hex, to_string};
use otlp_core::format::duration;
use otlp_core::record::severity_name;
use otlp_core::{Batch, Filter, Item};

/// Attributes worth showing next to a span, when present.
const NOTABLE: &[&str] = &[
    "gen_ai.request.model",
    "gen_ai.usage.input_tokens",
    "gen_ai.usage.output_tokens",
    "http.request.method",
    "http.method",
    "url.full",
    "http.url",
    "http.response.status_code",
    "http.status_code",
    "db.system",
    "exception.type",
];

struct Span {
    span_id: String,
    parent_id: String,
    name: String,
    service: String,
    start: u64,
    end: u64,
    error: Option<String>,
    attrs: Vec<String>,
    matched: bool,
}

/// A log record that matched, shown under the span it was emitted in.
struct Log {
    span_id: String,
    time: u64,
    severity: String,
    body: String,
}

/// Every span seen so far, by trace ID. Whole traces are kept because a trace's spans
/// can arrive in any order across many requests.
#[derive(Default)]
pub struct Traces {
    spans: HashMap<String, Vec<Span>>,
    logs: HashMap<String, Vec<Log>>,
    matched: HashSet<String>,
}

impl Traces {
    pub fn add(&mut self, batch: &Batch, filter: &Filter) {
        for r in batch.records() {
            match r.item {
                Item::Span(s) => {
                    let trace_id = hex(&s.trace_id);
                    let matched = filter.matches(&r);
                    if matched {
                        self.matched.insert(trace_id.clone());
                    }
                    let error = (r.status_code() == Some(2))
                        .then(|| s.status.as_ref().map(|st| st.message.clone()).unwrap_or_default());
                    let attrs = NOTABLE
                        .iter()
                        .filter_map(|k| otlp_core::attrs::find(&s.attributes, k).map(|v| format!("{k}={}", to_string(v))))
                        .collect();
                    self.spans.entry(trace_id).or_default().push(Span {
                        span_id: hex(&s.span_id),
                        parent_id: hex(&s.parent_span_id),
                        name: s.name.clone(),
                        service: r.service().unwrap_or_default().to_string(),
                        start: s.start_time_unix_nano,
                        end: s.end_time_unix_nano,
                        error,
                        attrs,
                        matched,
                    });
                }
                // Only matching logs are shown, so only those are kept
                Item::Log(l) => {
                    let Some(trace_id) = r.trace_id().filter(|_| filter.matches(&r)) else { continue };
                    self.matched.insert(trace_id.clone());
                    self.logs.entry(trace_id).or_default().push(Log {
                        span_id: hex(&l.span_id),
                        time: r.time_ns().unwrap_or(0),
                        severity: r.severity().map_or("LOG", severity_name).to_string(),
                        body: r.name(),
                    });
                }
                Item::Metric(_) => {}
            }
        }
    }

    pub fn print(&self, out: &mut impl Write) -> io::Result<()> {
        let none_spans = Vec::new();
        let none_logs = Vec::new();
        let mut traces: Vec<(&String, &Vec<Span>, &Vec<Log>)> = self
            .matched
            .iter()
            .map(|id| (id, self.spans.get(id).unwrap_or(&none_spans), self.logs.get(id).unwrap_or(&none_logs)))
            .collect();
        traces.sort_by_key(|(_, spans, logs)| {
            spans.iter().map(|s| s.start).chain(logs.iter().map(|l| l.time)).min()
        });

        for (i, (trace_id, spans, logs)) in traces.into_iter().enumerate() {
            if i > 0 {
                writeln!(out)?;
            }
            let start = spans.iter().map(|s| s.start).min().unwrap_or(0);
            let end = spans.iter().map(|s| s.end).max().unwrap_or(0);
            let plural = if spans.len() == 1 { "" } else { "s" };
            writeln!(out, "trace {trace_id}  {} span{plural}, {}", spans.len(), duration(end.saturating_sub(start)))?;

            let ids: HashSet<&str> = spans.iter().map(|s| s.span_id.as_str()).collect();
            let mut children: HashMap<&str, Vec<&Span>> = HashMap::new();
            let mut roots = Vec::new();
            for s in spans {
                if ids.contains(s.parent_id.as_str()) {
                    children.entry(&s.parent_id).or_default().push(s);
                } else {
                    roots.push(s);
                }
            }
            roots.sort_by_key(|s| s.start);
            for v in children.values_mut() {
                v.sort_by_key(|s| s.start);
            }
            let mut span_logs: HashMap<&str, Vec<&Log>> = HashMap::new();
            let mut loose_logs = Vec::new();
            for l in logs {
                if ids.contains(l.span_id.as_str()) {
                    span_logs.entry(&l.span_id).or_default().push(l);
                } else {
                    loose_logs.push(l);
                }
            }
            for v in span_logs.values_mut() {
                v.sort_by_key(|l| l.time);
            }
            loose_logs.sort_by_key(|l| l.time);

            let tree = Tree {
                children,
                logs: span_logs,
                // Mark matching spans only when they're a subset of the trace
                mark: !spans.iter().all(|s| s.matched),
            };
            for l in loose_logs {
                print_log(out, l, "")?;
            }
            for (j, root) in roots.iter().enumerate() {
                tree.print_span(out, root, "", j + 1 == roots.len(), None)?;
            }
        }
        Ok(())
    }
}

struct Tree<'a> {
    children: HashMap<&'a str, Vec<&'a Span>>,
    logs: HashMap<&'a str, Vec<&'a Log>>,
    mark: bool,
}

impl Tree<'_> {
    fn print_span(
        &self,
        out: &mut impl Write,
        span: &Span,
        prefix: &str,
        last: bool,
        parent_service: Option<&str>,
    ) -> io::Result<()> {
        let branch = if last { "└─ " } else { "├─ " };
        let mut line = format!("{prefix}{branch}{}  {}", span.name, duration(span.end.saturating_sub(span.start)));
        if parent_service != Some(span.service.as_str()) && !span.service.is_empty() {
            line += &format!("  [{}]", span.service);
        }
        if let Some(msg) = &span.error {
            line += if msg.is_empty() { "  ERROR".to_string() } else { format!("  ERROR: {msg}") }.as_str();
        }
        for a in &span.attrs {
            line += "  ";
            line += a;
        }
        if self.mark && span.matched {
            line += "  <- match";
        }
        writeln!(out, "{line}")?;

        let prefix = format!("{prefix}{}", if last { "   " } else { "│  " });
        let kids = self.children.get(span.span_id.as_str()).map(Vec::as_slice).unwrap_or_default();
        let logs = self.logs.get(span.span_id.as_str()).map(Vec::as_slice).unwrap_or_default();
        for l in logs {
            print_log(out, l, &format!("{prefix}{}", if kids.is_empty() { "   " } else { "│  " }))?;
        }
        for (i, child) in kids.iter().enumerate() {
            self.print_span(out, child, &prefix, i + 1 == kids.len(), Some(&span.service))?;
        }
        Ok(())
    }
}

fn print_log(out: &mut impl Write, log: &Log, prefix: &str) -> io::Result<()> {
    writeln!(out, "{prefix}· {} {}  <- match", log.severity, log.body)
}
