mod code;
mod concert_archive;
mod design;
mod diff;
mod fixture;
mod graphify;
mod impact;
mod local_check;
mod named;
mod receipt;
mod standard;
mod status;
mod verify_install;
mod visualization;
mod workflow;
mod workflow_demo;

use anyhow::{Context, Result, bail, ensure};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Execute GDD conformance cases and local checkout checks.
#[derive(Parser)]
#[command(name = "gdd-conformance")]
struct Cli {
    /// GDD checkout (or fixture root for a named suite). Defaults to the current checkout.
    #[arg(long, value_name = "PATH")]
    checkout: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run local Rust contributor checks and save their logs in a fresh directory.
    LocalCheck {
        /// Run focused readiness, CLI, viewer and skill checks.
        #[arg(long)]
        quick: bool,
        #[arg(long, value_name = "NEW_DIR")]
        output: Option<PathBuf>,
    },
    /// Verify an installed native CLI against a separate reference executable.
    VerifyInstall {
        #[arg(long, value_name = "FILE")]
        installed: PathBuf,
        #[arg(long, value_name = "FILE")]
        reference: PathBuf,
        #[arg(long, value_name = "NEW_FILE")]
        output: Option<PathBuf>,
    },
    /// Build, verify or restore an indexed concert experiment archive.
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
    /// Execute the named workflow packet, protocol and application replay cases.
    Workflow {
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

fn suite_directory(checkout: Option<&Path>) -> Result<PathBuf> {
    if let Some(root) = checkout {
        let directory = root.join("conformance");
        ensure!(
            directory.is_dir(),
            "expected a conformance directory at {}",
            directory.display()
        );
        return Ok(directory);
    }
    let current = std::env::current_dir().context("locate current directory")?;
    for root in current.ancestors() {
        let directory = root.join("conformance");
        if directory.join("cases.yaml").is_file() {
            return Ok(directory);
        }
        if root.join(".git").exists() {
            break;
        }
    }
    bail!(
        "could not find a GDD checkout from {}: use --checkout PATH",
        current.display()
    )
}

type SuiteRunner = fn(&Path, Option<PathBuf>) -> Result<ExitCode>;

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let (run, output): (SuiteRunner, _) = match cli.command {
        Command::LocalCheck { quick, output } => {
            let directory = suite_directory(cli.checkout.as_deref())?.canonicalize()?;
            return local_check::run(
                directory.parent().context("locate checkout")?,
                quick,
                output,
            );
        }
        Command::VerifyInstall {
            installed,
            reference,
            output,
        } => {
            let directory = suite_directory(cli.checkout.as_deref())?.canonicalize()?;
            return verify_install::run(
                directory.parent().context("locate checkout")?,
                &installed,
                &reference,
                output,
            );
        }
        Command::ConcertArchive { action } => return concert_archive::command(action),
        Command::WorkflowDemo { output } => {
            let summary = workflow_demo::replay(&output)?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            return Ok(ExitCode::SUCCESS);
        }
        Command::Code { output } => (code::command, output),
        Command::Design { output } => (design::command, output),
        Command::Diff { output } => (diff::command, output),
        Command::Impact { output } => (impact::command, output),
        Command::Status { output } => (status::command, output),
        Command::Standard { output } => (standard::command, output),
        Command::Workflow { output } => (workflow::command, output),
        Command::Graphify { output } => (graphify::command, output),
        Command::Visualization { output } => (visualization::command, output),
    };
    run(&suite_directory(cli.checkout.as_deref())?, output)
}
