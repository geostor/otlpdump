use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use otlp_core::{FilterArgs, input};

mod tree;

/// Find spans, logs, and metrics in otlpdump output.
///
/// Prints the matching records in the same JSON format otlpdump writes, so the output can
/// be saved, piped to jq, or searched again with otlpfind.
#[derive(Parser)]
#[command(version, after_help = "\
All the filters given must match. Repeating --trace, --span, --service, --kind, --name,
--status, or --severity matches any of the values. Repeating --has-attr, --attr, or
--attr-value requires all of them.

Examples:
  otlpfind capture.jsonl --trace 03a3fd794863282240627fd3ad8135d2 --tree
  otlpfind capture.jsonl --service checkout --status error
  otlpfind capture.jsonl --attr gen_ai.request.model=gpt-4o --min-duration 2s
  tail -f capture.jsonl | otlpfind --severity '>=WARN'")]
struct Cli {
    /// Files written by otlpdump; reads stdin when none are given
    files: Vec<PathBuf>,

    #[command(flatten)]
    filter: FilterArgs,

    /// Print each trace containing a match as a tree of spans
    #[arg(long)]
    tree: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        // Stop quietly when the reader goes away, e.g. `otlpfind ... | head`
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("otlpfind: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> io::Result<()> {
    let filter = cli.filter.build().map_err(io::Error::other)?;
    let reader = input::open(&cli.files)?;
    let mut out = io::stdout().lock();
    let mut traces = tree::Traces::default();

    for batch in input::batches(reader) {
        let mut batch = batch.map_err(io::Error::other)?;
        if cli.tree {
            traces.add(&batch, &filter);
        } else if batch.retain(|r| filter.matches(r)) {
            writeln!(out, "{}", batch.to_json())?;
        }
    }

    if cli.tree {
        traces.print(&mut out)?;
    }
    Ok(())
}
