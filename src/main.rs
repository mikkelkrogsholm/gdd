use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use gdd::{DEFAULT_PROJECT_ID, DEFAULT_TITLE, bundle, init_project};
use serde::Serialize;

#[derive(Parser)]
#[command(about = "Graph-Driven Development project tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a valid, unready GDD 0.3 project draft.
    #[command(args_override_self = true)]
    Init {
        #[arg(allow_negative_numbers = true)]
        path: PathBuf,
        #[arg(long, default_value = DEFAULT_PROJECT_ID, allow_negative_numbers = true)]
        project_id: String,
        #[arg(long, default_value = DEFAULT_TITLE, allow_negative_numbers = true)]
        title: String,
    },
    /// Compute the portable revision of a GDD 0.2 or 0.3 bundle.
    Snapshot {
        path: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

fn write_json(writer: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *writer, value)?;
    writeln!(writer)
}

fn run() -> io::Result<ExitCode> {
    match Cli::parse().command {
        Command::Init {
            path,
            project_id,
            title,
        } => {
            let receipt = init_project(&path, &project_id, &title)?;
            write_json(&mut io::stdout().lock(), &receipt)?;
        }
        Command::Snapshot { path, output } => {
            let bundle = bundle::load(&path);
            let code = bundle.exit_code();
            if code != 0 {
                for diagnostic in bundle.diagnostics() {
                    eprintln!("gdd: {diagnostic}");
                }
                return Ok(ExitCode::from(code));
            }
            let Some(snapshot) = bundle.snapshot() else {
                eprintln!(
                    "gdd: {}: A snapshot requires a GDD model manifest.",
                    path.display()
                );
                return Ok(ExitCode::from(2));
            };
            if let Some(output) = output {
                write_json(&mut File::create_new(output)?, snapshot)?;
            } else {
                write_json(&mut io::stdout().lock(), snapshot)?;
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("gdd: {error}");
            ExitCode::FAILURE
        }
    }
}
