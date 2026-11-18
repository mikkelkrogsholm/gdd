use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use gdd::{DEFAULT_PROJECT_ID, DEFAULT_TITLE, init_project};

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
}

fn run() -> io::Result<()> {
    let Command::Init {
        path,
        project_id,
        title,
    } = Cli::parse().command;
    let receipt = init_project(&path, &project_id, &title)?;
    let stdout = io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer_pretty(&mut output, &receipt)?;
    writeln!(output)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gdd: {error}");
            ExitCode::FAILURE
        }
    }
}
