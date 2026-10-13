//! Reading otlpdump output from files or stdin.

use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::PathBuf;

use serde_json::Value;

use crate::record::Batch;

/// Opens the inputs in order, or stdin when there are none (`-` also means stdin).
pub fn open(paths: &[PathBuf]) -> io::Result<Box<dyn Read>> {
    if paths.is_empty() {
        return Ok(Box::new(io::stdin().lock()));
    }
    let mut reader: Box<dyn Read> = Box::new(io::empty());
    for path in paths {
        let next: Box<dyn Read> = if path.as_os_str() == "-" {
            Box::new(io::stdin().lock())
        } else {
            let file = File::open(path)
                .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;
            Box::new(file)
        };
        reader = Box::new(reader.chain(next));
    }
    Ok(reader)
}

/// Parses a stream of OTLP export requests as JSON, as written by otlpdump.
///
/// Values may be one per line or pretty-printed (e.g. after `jq`). Records are yielded
/// as soon as they're read, so piping from `tail -f` works. Reading stops after the
/// first error.
pub fn batches(reader: impl Read) -> impl Iterator<Item = serde_json::Result<Batch>> {
    let mut stream = serde_json::Deserializer::from_reader(BufReader::new(reader)).into_iter::<Value>();
    let mut failed = false;
    std::iter::from_fn(move || {
        if failed {
            return None;
        }
        let item = stream.next()?.and_then(Batch::from_json);
        failed = item.is_err();
        Some(item)
    })
}
