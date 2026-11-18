mod graphify;
mod visualization;

use anyhow::Result;
use clap::Parser;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Execute named GDD conformance cases from the checkout.
#[derive(Parser)]
#[command(name = "gdd-conformance")]
enum Command {
    /// Run the Graphify adapter cases and optionally save the JSON receipt.
    Graphify {
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
    /// Verify named native view artifact cases and print or save the JSON receipt.
    Visualization {
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<ExitCode> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance");
    match Command::parse() {
        Command::Graphify { output } => graphify::command(&directory, output),
        Command::Visualization { output } => visualization::command(&directory, output),
    }
}
