mod code;
mod concert_archive;
mod design;
mod diff;
mod fixture;
mod graphify;
mod impact;
mod named;
mod receipt;
mod standard;
mod status;
mod visualization;
mod workflow_demo;

use anyhow::Result;
use clap::Parser;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Execute named GDD conformance cases from the checkout.
#[derive(Parser)]
#[command(name = "gdd-conformance")]
enum Command {
    /// Verify or restore an indexed concert experiment archive.
    ConcertArchive {
        #[command(subcommand)]
        action: concert_archive::Action,
    },
    /// Execute the standard cases and print or save their full JSON receipt.
    Standard {
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
    /// Execute the named static code observation cases.
    Code {
        #[arg(long, value_name = "NEW_FILE")]
        output: Option<PathBuf>,
    },
    /// Execute the named design boundary and advisory review cases.
    Design {
        #[arg(long, value_name = "NEW_FILE")]
        output: Option<PathBuf>,
    },
    /// Execute the named stable-identity model diff cases.
    Diff {
        #[arg(long, value_name = "NEW_FILE")]
        output: Option<PathBuf>,
    },
    /// Execute the named declared-model impact cases.
    Impact {
        #[arg(long, value_name = "NEW_FILE")]
        output: Option<PathBuf>,
    },
    /// Execute the named project status cases.
    Status {
        #[arg(long, value_name = "NEW_FILE")]
        output: Option<PathBuf>,
    },
    /// Replay the Rust quote repair and publish its journal and real Cargo evidence.
    WorkflowDemo {
        #[arg(long, value_name = "NEW_DIR")]
        output: PathBuf,
    },
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
        Command::ConcertArchive { action } => concert_archive::command(action),
        Command::Code { output } => code::command(&directory, output),
        Command::Design { output } => design::command(&directory, output),
        Command::Diff { output } => diff::command(&directory, output),
        Command::Impact { output } => impact::command(&directory, output),
        Command::Status { output } => status::command(&directory, output),
        Command::Standard { output } => standard::command(&directory, output),
        Command::WorkflowDemo { output } => {
            let summary = workflow_demo::replay(&output)?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            Ok(ExitCode::SUCCESS)
        }
        Command::Graphify { output } => graphify::command(&directory, output),
        Command::Visualization { output } => visualization::command(&directory, output),
    }
}
