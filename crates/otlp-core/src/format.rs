//! Durations and clock helpers.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn duration(ns: u64) -> String {
    match ns {
        0..1_000 => format!("{ns}ns"),
        1_000..1_000_000 => format!("{:.1}µs", ns as f64 / 1e3),
        1_000_000..1_000_000_000 => format!("{:.1}ms", ns as f64 / 1e6),
        _ => format!("{:.2}s", ns as f64 / 1e9),
    }
}

pub fn now_ns() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64)
}
