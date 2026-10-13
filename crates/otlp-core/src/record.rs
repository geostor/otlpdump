//! OTLP export requests and the individual spans, logs, and metrics inside them.

use opentelemetry_proto::tonic::collector::{
    logs::v1::ExportLogsServiceRequest, metrics::v1::ExportMetricsServiceRequest,
    trace::v1::ExportTraceServiceRequest,
};
use opentelemetry_proto::tonic::common::v1::{AnyValue, InstrumentationScope, KeyValue};
use opentelemetry_proto::tonic::logs::v1::LogRecord;
use opentelemetry_proto::tonic::metrics::v1::{Metric, metric::Data};
use opentelemetry_proto::tonic::resource::v1::Resource;
use opentelemetry_proto::tonic::trace::v1::Span;
use serde::de::Error as _;
use serde_json::Value;

use crate::attrs;

/// One export request: a line of otlpdump output.
#[derive(Debug, Clone)]
pub enum Batch {
    Traces(ExportTraceServiceRequest),
    Metrics(ExportMetricsServiceRequest),
    Logs(ExportLogsServiceRequest),
}

impl Batch {
    pub fn from_json(v: Value) -> serde_json::Result<Batch> {
        // The request types default every field, so any object would deserialize as any
        // of them. Pick by the top-level key instead.
        let key = |k| v.get(k).is_some();
        if key("resourceSpans") {
            serde_json::from_value(v).map(Batch::Traces)
        } else if key("resourceLogs") {
            serde_json::from_value(v).map(Batch::Logs)
        } else if key("resourceMetrics") {
            serde_json::from_value(v).map(Batch::Metrics)
        } else {
            Err(serde_json::Error::custom(
                "not an OTLP export request (expected resourceSpans, resourceLogs, or resourceMetrics)",
            ))
        }
    }

    pub fn to_json(&self) -> String {
        let s = match self {
            Batch::Traces(r) => serde_json::to_string(r),
            Batch::Metrics(r) => serde_json::to_string(r),
            Batch::Logs(r) => serde_json::to_string(r),
        };
        s.expect("OTLP types serialize to JSON")
    }

    pub fn records(&self) -> Vec<Record<'_>> {
        let mut out = Vec::new();
        match self {
            Batch::Traces(r) => {
                for rs in &r.resource_spans {
                    for ss in &rs.scope_spans {
                        for s in &ss.spans {
                            out.push(Record::new(rs.resource.as_ref(), ss.scope.as_ref(), Item::Span(s)));
                        }
                    }
                }
            }
            Batch::Logs(r) => {
                for rl in &r.resource_logs {
                    for sl in &rl.scope_logs {
                        for l in &sl.log_records {
                            out.push(Record::new(rl.resource.as_ref(), sl.scope.as_ref(), Item::Log(l)));
                        }
                    }
                }
            }
            Batch::Metrics(r) => {
                for rm in &r.resource_metrics {
                    for sm in &rm.scope_metrics {
                        for m in &sm.metrics {
                            out.push(Record::new(rm.resource.as_ref(), sm.scope.as_ref(), Item::Metric(m)));
                        }
                    }
                }
            }
        }
        out
    }

    /// Keeps only the records `keep` returns true for, dropping any resource and scope
    /// groups left empty. Returns false if nothing is left.
    pub fn retain(&mut self, mut keep: impl FnMut(&Record) -> bool) -> bool {
        match self {
            Batch::Traces(r) => {
                r.resource_spans.retain_mut(|rs| {
                    let resource = rs.resource.as_ref();
                    rs.scope_spans.retain_mut(|ss| {
                        let scope = ss.scope.as_ref();
                        ss.spans.retain(|s| keep(&Record::new(resource, scope, Item::Span(s))));
                        !ss.spans.is_empty()
                    });
                    !rs.scope_spans.is_empty()
                });
                !r.resource_spans.is_empty()
            }
            Batch::Logs(r) => {
                r.resource_logs.retain_mut(|rl| {
                    let resource = rl.resource.as_ref();
                    rl.scope_logs.retain_mut(|sl| {
                        let scope = sl.scope.as_ref();
                        sl.log_records.retain(|l| keep(&Record::new(resource, scope, Item::Log(l))));
                        !sl.log_records.is_empty()
                    });
                    !rl.scope_logs.is_empty()
                });
                !r.resource_logs.is_empty()
            }
            Batch::Metrics(r) => {
                r.resource_metrics.retain_mut(|rm| {
                    let resource = rm.resource.as_ref();
                    rm.scope_metrics.retain_mut(|sm| {
                        let scope = sm.scope.as_ref();
                        sm.metrics.retain(|m| keep(&Record::new(resource, scope, Item::Metric(m))));
                        !sm.metrics.is_empty()
                    });
                    !rm.scope_metrics.is_empty()
                });
                !r.resource_metrics.is_empty()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Kind {
    Span,
    Log,
    Metric,
}

#[derive(Debug, Clone, Copy)]
pub enum Item<'a> {
    Span(&'a Span),
    Log(&'a LogRecord),
    Metric(&'a Metric),
}

/// A single span, log record, or metric, along with the resource and scope it came from.
#[derive(Debug, Clone, Copy)]
pub struct Record<'a> {
    pub resource: Option<&'a Resource>,
    pub scope: Option<&'a InstrumentationScope>,
    pub item: Item<'a>,
}

impl<'a> Record<'a> {
    pub fn new(resource: Option<&'a Resource>, scope: Option<&'a InstrumentationScope>, item: Item<'a>) -> Self {
        Record { resource, scope, item }
    }

    pub fn kind(&self) -> Kind {
        match self.item {
            Item::Span(_) => Kind::Span,
            Item::Log(_) => Kind::Log,
            Item::Metric(_) => Kind::Metric,
        }
    }

    pub fn service(&self) -> Option<&'a str> {
        let v = attrs::find(&self.resource?.attributes, "service.name")?;
        match &v.value {
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(s)) => Some(s),
            _ => None,
        }
    }

    /// The span name, metric name, or log body.
    pub fn name(&self) -> String {
        match self.item {
            Item::Span(s) => s.name.clone(),
            Item::Metric(m) => m.name.clone(),
            Item::Log(l) => l.body.as_ref().map(attrs::to_string).unwrap_or_default(),
        }
    }

    /// The record's own attributes: a span's (including its events'), a log's, or those of
    /// every data point in a metric.
    pub fn own_attributes(&self) -> Vec<&'a KeyValue> {
        match self.item {
            Item::Span(s) => s.attributes.iter().chain(s.events.iter().flat_map(|e| &e.attributes)).collect(),
            Item::Log(l) => l.attributes.iter().collect(),
            Item::Metric(m) => data_points(m).into_iter().flat_map(|(a, _)| a).collect(),
        }
    }

    /// The record's own attributes, then its scope's, then its resource's.
    pub fn all_attributes(&self) -> Vec<&'a KeyValue> {
        let mut out = self.own_attributes();
        out.extend(self.scope.into_iter().flat_map(|s| &s.attributes));
        out.extend(self.resource.into_iter().flat_map(|r| &r.attributes));
        out
    }

    /// Looks up an attribute on the record, falling back to its scope and resource.
    pub fn attr(&self, key: &str) -> Option<&'a AnyValue> {
        self.all_attributes().into_iter().find(|kv| kv.key == key)?.value.as_ref()
    }

    pub fn trace_id(&self) -> Option<String> {
        let id = match self.item {
            Item::Span(s) => &s.trace_id,
            Item::Log(l) => &l.trace_id,
            Item::Metric(_) => return None,
        };
        (!id.is_empty()).then(|| attrs::hex(id))
    }

    pub fn span_id(&self) -> Option<String> {
        let id = match self.item {
            Item::Span(s) => &s.span_id,
            Item::Log(l) => &l.span_id,
            Item::Metric(_) => return None,
        };
        (!id.is_empty()).then(|| attrs::hex(id))
    }

    /// When the record happened: a span's start, a log's timestamp (or observed time),
    /// or a metric's latest data point.
    pub fn time_ns(&self) -> Option<u64> {
        let t = match self.item {
            Item::Span(s) => s.start_time_unix_nano,
            Item::Log(l) if l.time_unix_nano != 0 => l.time_unix_nano,
            Item::Log(l) => l.observed_time_unix_nano,
            Item::Metric(m) => data_points(m).into_iter().map(|(_, t)| t).max().unwrap_or(0),
        };
        (t != 0).then_some(t)
    }

    pub fn duration_ns(&self) -> Option<u64> {
        match self.item {
            Item::Span(s) => Some(s.end_time_unix_nano.saturating_sub(s.start_time_unix_nano)),
            _ => None,
        }
    }

    /// The span status code: 0 unset, 1 ok, 2 error.
    pub fn status_code(&self) -> Option<i32> {
        match self.item {
            Item::Span(s) => Some(s.status.as_ref().map_or(0, |st| st.code)),
            _ => None,
        }
    }

    /// The log severity number (1-24), using the severity text when the number isn't set.
    pub fn severity(&self) -> Option<i32> {
        match self.item {
            Item::Log(l) if l.severity_number > 0 => Some(l.severity_number),
            Item::Log(l) => severity_from_name(&l.severity_text),
            _ => None,
        }
    }
}

/// Maps a level name to the lowest severity number in its range (ERROR is 17-20).
pub fn severity_from_name(name: &str) -> Option<i32> {
    match name.to_ascii_uppercase().as_str() {
        "TRACE" => Some(1),
        "DEBUG" => Some(5),
        "INFO" => Some(9),
        "WARN" | "WARNING" => Some(13),
        "ERROR" => Some(17),
        "FATAL" | "CRITICAL" => Some(21),
        _ => None,
    }
}

pub fn severity_name(n: i32) -> &'static str {
    match n {
        1..=4 => "TRACE",
        5..=8 => "DEBUG",
        9..=12 => "INFO",
        13..=16 => "WARN",
        17..=20 => "ERROR",
        21..=24 => "FATAL",
        _ => "UNSPECIFIED",
    }
}

/// Each data point's attributes and timestamp.
fn data_points(m: &Metric) -> Vec<(&[KeyValue], u64)> {
    match &m.data {
        None => Vec::new(),
        Some(Data::Gauge(d)) => d.data_points.iter().map(|p| (&p.attributes[..], p.time_unix_nano)).collect(),
        Some(Data::Sum(d)) => d.data_points.iter().map(|p| (&p.attributes[..], p.time_unix_nano)).collect(),
        Some(Data::Histogram(d)) => d.data_points.iter().map(|p| (&p.attributes[..], p.time_unix_nano)).collect(),
        Some(Data::ExponentialHistogram(d)) => {
            d.data_points.iter().map(|p| (&p.attributes[..], p.time_unix_nano)).collect()
        }
        Some(Data::Summary(d)) => d.data_points.iter().map(|p| (&p.attributes[..], p.time_unix_nano)).collect(),
    }
}
