use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use gdd::validation::{self, Profile, ValidationOptions};
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
    /// Extract revision-bound read context for the selected model nodes.
    #[command(args_override_self = true)]
    Context {
        #[arg(allow_negative_numbers = true)]
        path: PathBuf,
        #[arg(long, required = true, allow_negative_numbers = true)]
        scope: Vec<String>,
        #[arg(long, allow_negative_numbers = true)]
        output: Option<PathBuf>,
    },
    /// Validate a GDD document or model bundle.
    Validate {
        path: PathBuf,
        #[arg(long, value_enum, default_value = "model")]
        profile: Vec<Profile>,
        #[arg(long)]
        scope: Vec<String>,
        #[arg(long = "unsupported-extension")]
        unsupported_extensions: Vec<String>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

fn write_json(writer: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *writer, value)?;
    writeln!(writer)
}

fn write_output(output: Option<&Path>, value: &impl Serialize) -> io::Result<()> {
    if let Some(path) = output {
        write_json(&mut File::create_new(path)?, value)
    } else {
        write_json(&mut io::stdout().lock(), value)
    }
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
            let report = validation::validate(&bundle, Profile::Model);
            let code = report.exit_code();
            if code == 0
                && let Some(snapshot) = bundle.snapshot()
            {
                write_output(output.as_deref(), snapshot)?;
                return Ok(ExitCode::from(code));
            }
            write_output(output.as_deref(), &report)?;
            return Ok(ExitCode::from(if code == 0 { 2 } else { code }));
        }
        Command::Context {
            path,
            scope,
            output,
        } => {
            let bundle = bundle::load(&path);
            let report = validation::validate(&bundle, Profile::Model);
            let code = report.exit_code();
            if code != 0 {
                write_output(output.as_deref(), &report)?;
                return Ok(ExitCode::from(code));
            }
            write_output(
                output.as_deref(),
                &gdd::context::context_manifest(&bundle, &scope)?,
            )?;
        }
        Command::Validate {
            path,
            profile,
            scope,
            unsupported_extensions,
            output,
        } => {
            let bundle = bundle::load(&path);
            let report = validation::validate_with_options(
                &bundle,
                ValidationOptions {
                    profiles: &profile,
                    scope: &scope,
                    unsupported_extensions: &unsupported_extensions,
                },
            )?;
            write_output(output.as_deref(), &report)?;
            return Ok(ExitCode::from(report.exit_code()));
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
