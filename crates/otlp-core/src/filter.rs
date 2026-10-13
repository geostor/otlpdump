//! Selecting records by trace, service, name, attributes, time, and so on.
//!
//! All the flags given must match. Repeating a flag that a record has one value for
//! (`--trace`, `--service`, `--kind`, `--name`, `--status`, `--severity`) matches any of
//! the values given. Repeating an attribute flag requires all of them to match.

use std::time::Duration;

use regex::Regex;

use crate::attrs;
use crate::format::now_ns;
use crate::record::{Kind, Record, severity_from_name};

#[derive(clap::Args, Debug, Default, Clone)]
pub struct FilterArgs {
    /// Trace ID (hex)
    #[arg(long, value_name = "ID")]
    pub trace: Vec<String>,

    /// Span ID (hex); also matches logs emitted inside that span
    #[arg(long, value_name = "ID")]
    pub span: Vec<String>,

    /// Resource service.name; supports * and ? wildcards
    #[arg(long, value_name = "NAME")]
    pub service: Vec<String>,

    /// Only spans, logs, or metrics
    #[arg(long, value_enum)]
    pub kind: Vec<Kind>,

    /// Span name, metric name, or log body; supports * and ? wildcards
    #[arg(long, value_name = "GLOB")]
    pub name: Vec<String>,

    /// Span status (only matches spans)
    #[arg(long, value_enum)]
    pub status: Vec<Status>,

    /// Log severity: TRACE, DEBUG, INFO, WARN, ERROR, FATAL, optionally prefixed with >= or <=
    /// (only matches logs)
    #[arg(long, value_name = "LEVEL")]
    pub severity: Vec<String>,

    /// Has an attribute with this name (on the record, its scope, or its resource)
    #[arg(long = "has-attr", value_name = "KEY")]
    pub has_attr: Vec<String>,

    /// Has an attribute with this name and value; the value supports * and ? wildcards,
    /// and KEY~REGEX matches it by regex
    #[arg(long, value_name = "KEY=VALUE")]
    pub attr: Vec<String>,

    /// Has any attribute with this value; supports * and ? wildcards, e.g. '*timeout*'
    #[arg(long = "attr-value", value_name = "VALUE")]
    pub attr_value: Vec<String>,

    /// Spans that took at least this long, e.g. 500ms or 2s (only matches spans)
    #[arg(long = "min-duration", value_name = "DURATION")]
    pub min_duration: Option<String>,

    /// Spans that took at most this long (only matches spans)
    #[arg(long = "max-duration", value_name = "DURATION")]
    pub max_duration: Option<String>,

    /// At or after this time: a duration ago (10m, 2h) or a UTC timestamp (2026-10-13T07:00:00Z)
    #[arg(long, value_name = "TIME")]
    pub since: Option<String>,

    /// Before this time, in the same formats as --since
    #[arg(long, value_name = "TIME")]
    pub until: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Status {
    Unset,
    Ok,
    Error,
}

impl Status {
    fn code(self) -> i32 {
        match self {
            Status::Unset => 0,
            Status::Ok => 1,
            Status::Error => 2,
        }
    }
}

#[derive(Debug)]
enum AttrMatch {
    Glob(String, String),
    Regex(String, Regex),
}

/// A parsed set of [`FilterArgs`].
#[derive(Debug, Default)]
pub struct Filter {
    trace: Vec<String>,
    span: Vec<String>,
    service: Vec<String>,
    kind: Vec<Kind>,
    name: Vec<String>,
    status: Vec<i32>,
    severity: Vec<(i32, i32)>,
    has_attr: Vec<String>,
    attr: Vec<AttrMatch>,
    attr_value: Vec<String>,
    min_duration: Option<u64>,
    max_duration: Option<u64>,
    since: Option<u64>,
    until: Option<u64>,
}

impl FilterArgs {
    pub fn build(&self) -> Result<Filter, String> {
        Ok(Filter {
            trace: self.trace.iter().map(|s| s.to_ascii_lowercase()).collect(),
            span: self.span.iter().map(|s| s.to_ascii_lowercase()).collect(),
            service: self.service.clone(),
            kind: self.kind.clone(),
            name: self.name.clone(),
            status: self.status.iter().map(|s| s.code()).collect(),
            severity: self.severity.iter().map(|s| parse_severity(s)).collect::<Result<_, _>>()?,
            has_attr: self.has_attr.clone(),
            attr: self.attr.iter().map(|s| parse_attr(s)).collect::<Result<_, _>>()?,
            attr_value: self.attr_value.clone(),
            min_duration: self.min_duration.as_deref().map(parse_duration).transpose()?,
            max_duration: self.max_duration.as_deref().map(parse_duration).transpose()?,
            since: self.since.as_deref().map(parse_time).transpose()?,
            until: self.until.as_deref().map(parse_time).transpose()?,
        })
    }
}

impl Filter {
    pub fn matches(&self, r: &Record) -> bool {
        any_of(&self.kind, |k| *k == r.kind())
            && any_of(&self.trace, |id| r.trace_id().as_deref() == Some(id))
            && any_of(&self.span, |id| r.span_id().as_deref() == Some(id))
            && any_of(&self.service, |g| r.service().is_some_and(|s| glob(g, s)))
            && (self.name.is_empty() || {
                let name = r.name();
                self.name.iter().any(|g| glob(g, &name))
            })
            && any_of(&self.status, |c| r.status_code() == Some(*c))
            && any_of(&self.severity, |(lo, hi)| r.severity().is_some_and(|n| (*lo..=*hi).contains(&n)))
            && self.attributes_match(r)
            && self.min_duration.is_none_or(|min| r.duration_ns().is_some_and(|d| d >= min))
            && self.max_duration.is_none_or(|max| r.duration_ns().is_some_and(|d| d <= max))
            && self.since.is_none_or(|t| r.time_ns().is_some_and(|rt| rt >= t))
            && self.until.is_none_or(|t| r.time_ns().is_some_and(|rt| rt < t))
    }

    fn attributes_match(&self, r: &Record) -> bool {
        if self.has_attr.is_empty() && self.attr.is_empty() && self.attr_value.is_empty() {
            return true;
        }
        let all = r.all_attributes();
        let values = |key: &String| -> Vec<String> {
            all.iter()
                .filter(|kv| kv.key == *key)
                .filter_map(|kv| kv.value.as_ref().map(attrs::to_string))
                .collect()
        };
        self.has_attr.iter().all(|key| all.iter().any(|kv| kv.key == *key))
            && self.attr.iter().all(|m| match m {
                AttrMatch::Glob(key, want) => values(key).iter().any(|v| glob(want, v)),
                AttrMatch::Regex(key, re) => values(key).iter().any(|v| re.is_match(v)),
            })
            && self.attr_value.iter().all(|want| {
                all.iter().any(|kv| kv.value.as_ref().is_some_and(|v| glob(want, &attrs::to_string(v))))
            })
    }
}

/// True if `options` is empty (no constraint) or any option matches.
fn any_of<T>(options: &[T], f: impl FnMut(&T) -> bool) -> bool {
    options.is_empty() || options.iter().any(f)
}

fn parse_attr(s: &str) -> Result<AttrMatch, String> {
    let Some(i) = s.find(['=', '~']) else {
        return Err(format!("--attr {s:?}: expected KEY=VALUE or KEY~REGEX"));
    };
    let (key, value) = (s[..i].to_string(), &s[i + 1..]);
    if s.as_bytes()[i] == b'=' {
        Ok(AttrMatch::Glob(key, value.to_string()))
    } else {
        Regex::new(value).map(|re| AttrMatch::Regex(key, re)).map_err(|e| format!("--attr {s:?}: {e}"))
    }
}

/// Parses LEVEL, >=LEVEL, or <=LEVEL into an inclusive range of severity numbers.
fn parse_severity(s: &str) -> Result<(i32, i32), String> {
    let (op, level) = match s.get(..2) {
        Some(">=" | "<=") => (&s[..2], &s[2..]),
        _ => ("", s),
    };
    let lo = severity_from_name(level.trim())
        .ok_or_else(|| format!("--severity {s:?}: expected TRACE, DEBUG, INFO, WARN, ERROR, or FATAL"))?;
    let hi = lo + 3;
    Ok(match op {
        ">=" => (lo, 24),
        "<=" => (1, hi),
        _ => (lo, hi),
    })
}

fn parse_duration(s: &str) -> Result<u64, String> {
    humantime::parse_duration(s)
        .map(|d| d.as_nanos() as u64)
        .map_err(|e| format!("{s:?}: {e} (expected e.g. 500ms or 2s)"))
}

/// Parses a duration ago or an RFC 3339 timestamp into nanoseconds since the epoch.
fn parse_time(s: &str) -> Result<u64, String> {
    if let Ok(ago) = humantime::parse_duration(s) {
        return Ok(now_ns().saturating_sub(ago.as_nanos() as u64));
    }
    let t = humantime::parse_rfc3339_weak(s)
        .map_err(|_| format!("{s:?}: expected a duration ago (10m) or a timestamp (2026-10-13T07:00:00Z)"))?;
    Ok(t.duration_since(std::time::UNIX_EPOCH).unwrap_or(Duration::ZERO).as_nanos() as u64)
}

/// Shell-style wildcard match: `*` matches any run of characters, `?` any one.
pub fn glob(pattern: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut pi, mut ti) = (0, 0);
    let mut backtrack = None;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            backtrack = Some((pi, ti));
            pi += 1;
        } else if let Some((bp, bt)) = backtrack {
            pi = bp + 1;
            ti = bt + 1;
            backtrack = Some((bp, bt + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matching() {
        assert!(glob("Chat*", "ChatOpenAI"));
        assert!(glob("*Open*", "ChatOpenAI"));
        assert!(glob("POS?", "POST"));
        assert!(glob("*", ""));
        assert!(glob("a*b*c", "aXXbYYc"));
        assert!(!glob("Chat", "ChatOpenAI"));
        assert!(!glob("a*b", "aXXc"));
    }

    #[test]
    fn severity_ranges() {
        assert_eq!(parse_severity("ERROR"), Ok((17, 20)));
        assert_eq!(parse_severity("warn"), Ok((13, 16)));
        assert_eq!(parse_severity(">=WARN"), Ok((13, 24)));
        assert_eq!(parse_severity("<=INFO"), Ok((1, 12)));
        assert!(parse_severity("LOUD").is_err());
    }

    #[test]
    fn attr_parsing() {
        assert!(matches!(parse_attr("a.b=c=d"), Ok(AttrMatch::Glob(k, v)) if k == "a.b" && v == "c=d"));
        assert!(matches!(parse_attr("model~^gpt-4"), Ok(AttrMatch::Regex(k, _)) if k == "model"));
        assert!(parse_attr("nothing").is_err());
    }
}
