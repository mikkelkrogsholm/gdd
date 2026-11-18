use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use gdd::validation::{self, Activity, Profile, ValidationOptions};
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
        /// Primary GDD model when the positional input is a secondary records file.
        #[arg(long)]
        model: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "model")]
        profile: Vec<Profile>,
        #[arg(long)]
        scope: Vec<String>,
        #[arg(long = "unsupported-extension")]
        unsupported_extensions: Vec<String>,
        #[arg(long)]
        host_context: Option<PathBuf>,
        #[arg(long, value_enum)]
        activity: Option<Activity>,
        #[arg(long)]
        package: Option<PathBuf>,
        #[arg(long)]
        parallel_plan: Option<PathBuf>,
        /// Explain readiness diagnostics and their scope paths on stderr.
        #[arg(long)]
        explain: bool,
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

fn tool_input(path: Option<PathBuf>) -> io::Result<Option<gdd::input::Value>> {
    let Some(path) = path else {
        return Ok(None);
    };
    match gdd::input::parse(&std::fs::read(path)?).map_err(io::Error::other)? {
        gdd::input::Value::Null => Ok(None),
        value => Ok(Some(value)),
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
            let bundle = bundle::load_model_only(&path);
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
            let bundle = bundle::load_model_only(&path);
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
            model,
            profile,
            scope,
            unsupported_extensions,
            host_context,
            activity,
            package,
            parallel_plan,
            explain,
            output,
        } => {
            let host = tool_input(host_context)?;
            let package = tool_input(package)?;
            let parallel = tool_input(parallel_plan)?;
            let options = ValidationOptions {
                profiles: &profile,
                activity,
                scope: &scope,
                host_context: host.as_ref(),
                package: package.as_ref(),
                parallel_plan: parallel.as_ref(),
                unsupported_extensions: &unsupported_extensions,
            };
            let mut stderr = io::stderr().lock();
            let report = validation::validate_paths(
                &path,
                model.as_deref(),
                options,
                explain.then_some(&mut stderr),
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
            if error
                .get_ref()
                .and_then(|e| e.downcast_ref::<gdd::input::InputError>())
                == Some(&gdd::input::InputError::UnsupportedDepth)
            {
                ExitCode::from(2)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}
