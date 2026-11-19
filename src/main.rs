use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use gdd::validation::{self, Activity, Profile, ValidationOptions};
use gdd::{DEFAULT_PROJECT_ID, DEFAULT_TITLE, bundle, init_project};
use serde::Serialize;

#[derive(Parser)]
#[command(about = "Graph-Driven Development project tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
enum DisplayFormat {
    Json,
    Text,
}

#[derive(clap::Args)]
struct WorkflowArgs {
    #[arg(allow_negative_numbers = true)]
    path: PathBuf,
    #[arg(long, allow_negative_numbers = true)]
    host_context: Option<PathBuf>,
    #[arg(long, allow_negative_numbers = true)]
    output: Option<PathBuf>,
}

#[derive(Subcommand)]
enum WorkflowCommand {
    /// Derive complete development packets and assess package readiness.
    #[command(args_override_self = true)]
    Plan {
        #[command(flatten)]
        args: WorkflowArgs,
        #[arg(long, allow_negative_numbers = true)]
        request: PathBuf,
    },
    /// Re-derive a supplied workplan and compare its entire payload.
    #[command(args_override_self = true)]
    Verify {
        #[command(flatten)]
        args: WorkflowArgs,
        #[arg(long, allow_negative_numbers = true)]
        plan: PathBuf,
    },
    /// Publish a new draft run from a verified workplan.
    #[command(args_override_self = true)]
    Create {
        #[command(flatten)]
        args: WorkflowArgs,
        #[arg(long, allow_negative_numbers = true)]
        plan: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        run: PathBuf,
    },
    /// Publish or replay one host-reported workflow transition.
    #[command(args_override_self = true)]
    Event {
        #[command(flatten)]
        args: WorkflowArgs,
        #[arg(long, allow_negative_numbers = true)]
        run: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        event: PathBuf,
    },
    /// Replay a run journal and assess its current gates.
    #[command(args_override_self = true)]
    Status {
        #[command(flatten)]
        args: WorkflowArgs,
        #[arg(long, allow_negative_numbers = true)]
        run: PathBuf,
    },
}

#[derive(Subcommand)]
enum GraphifyCommand {
    /// Preserve a raw artifact and publish a new observation bundle.
    #[command(args_override_self = true)]
    Import {
        #[arg(allow_negative_numbers = true)]
        path: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        output: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        profile: String,
        #[arg(long, allow_negative_numbers = true)]
        producer_version: Option<String>,
        #[arg(long, allow_negative_numbers = true)]
        code_revision: Option<String>,
        #[arg(long, default_value = "source.graphify", allow_negative_numbers = true)]
        source_id: String,
    },
    /// Assess a raw artifact or revalidate an observation bundle.
    #[command(args_override_self = true)]
    Validate {
        #[arg(allow_negative_numbers = true)]
        path: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        profile: Option<String>,
        #[arg(long, allow_negative_numbers = true)]
        producer_version: Option<String>,
    },
    /// Recover the original bytes to a new file.
    #[command(args_override_self = true)]
    Export {
        #[arg(allow_negative_numbers = true)]
        path: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        output: PathBuf,
    },
    /// Build an unaccepted model sketch from an explicit observation mapping.
    #[command(args_override_self = true)]
    Propose {
        #[arg(allow_negative_numbers = true)]
        path: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        mapping: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        output: PathBuf,
        #[arg(long, default_value = "gdd/0.3", value_parser = ["gdd/0.2", "gdd/0.3"])]
        format: String,
    },
}

#[derive(Subcommand)]
enum Command {
    /// Check explicit architecture constraints against static Python imports.
    #[command(args_override_self = true)]
    CheckCode {
        #[arg(allow_negative_numbers = true)]
        path: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        root: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        implementation: String,
        #[arg(long, allow_negative_numbers = true)]
        profile: String,
        /// Override the shipped CPython 3.11.15 Linux aarch64 target with a JSON descriptor.
        #[arg(long, allow_negative_numbers = true)]
        python_runtime: Option<PathBuf>,
        #[arg(long, allow_negative_numbers = true)]
        records: Option<PathBuf>,
        #[arg(long, allow_negative_numbers = true)]
        scope: Vec<String>,
        #[arg(long, allow_negative_numbers = true)]
        exclude: Vec<String>,
        #[arg(long, allow_negative_numbers = true)]
        output: Option<PathBuf>,
    },
    /// Import, verify, recover or propose from Graphify observations.
    Graphify {
        #[command(subcommand)]
        command: GraphifyCommand,
    },
    /// Prepare and verify development packets, manage runs, and publish run events.
    Workflow {
        #[command(subcommand)]
        command: WorkflowCommand,
    },
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
    /// Summarize scoped assessments and open questions from one model capture.
    #[command(args_override_self = true)]
    Status {
        #[arg(allow_negative_numbers = true)]
        path: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        scope: Vec<String>,
        #[arg(long, allow_negative_numbers = true)]
        host_context: Option<PathBuf>,
        #[arg(long, allow_negative_numbers = true)]
        run: Option<PathBuf>,
        #[arg(long, allow_negative_numbers = true)]
        implementation: Option<String>,
        #[arg(long, allow_negative_numbers = true)]
        code_revision: Option<String>,
        #[arg(long, value_enum, default_value = "implementation")]
        activity: Activity,
        #[arg(long, value_enum, default_value = "json")]
        format: DisplayFormat,
        #[arg(long, allow_negative_numbers = true)]
        output: Option<PathBuf>,
    },
    /// Compare two model snapshots by stable identity.
    #[command(args_override_self = true)]
    Diff {
        #[arg(allow_negative_numbers = true)]
        base: PathBuf,
        #[arg(allow_negative_numbers = true)]
        head: PathBuf,
        #[arg(long, value_enum, default_value = "json")]
        format: DisplayFormat,
        #[arg(long, allow_negative_numbers = true)]
        output: Option<PathBuf>,
    },
    /// Explain declared change consumers and supporting model context.
    #[command(args_override_self = true)]
    Impact {
        #[arg(allow_negative_numbers = true)]
        path: PathBuf,
        #[arg(long, required = true, allow_negative_numbers = true)]
        scope: Vec<String>,
        #[arg(long, allow_negative_numbers = true)]
        implementation: Option<String>,
        #[arg(long, value_enum, default_value = "json")]
        format: DisplayFormat,
        #[arg(long, allow_negative_numbers = true)]
        output: Option<PathBuf>,
    },
    /// Save a self-contained, read-only native graph view.
    #[command(args_override_self = true)]
    View {
        #[arg(allow_negative_numbers = true)]
        path: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        output: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        scope: Vec<String>,
    },
    /// Inspect advisory journey, component and data perspectives.
    #[command(args_override_self = true)]
    Perspectives {
        #[arg(allow_negative_numbers = true)]
        path: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        output: Option<PathBuf>,
    },
    /// Derive component interfaces, ownership and community call boundaries.
    #[command(args_override_self = true)]
    Boundaries {
        #[arg(allow_negative_numbers = true)]
        path: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        output: Option<PathBuf>,
    },
    /// Assess the applicability of an advisory design review.
    #[command(args_override_self = true)]
    DesignReview {
        #[arg(allow_negative_numbers = true)]
        path: PathBuf,
        #[arg(long, allow_negative_numbers = true)]
        review: Option<PathBuf>,
        #[arg(long, allow_negative_numbers = true)]
        implementation: String,
        #[arg(long, allow_negative_numbers = true)]
        code_revision: String,
        #[arg(long, allow_negative_numbers = true)]
        scope: Vec<String>,
        #[arg(long, allow_negative_numbers = true)]
        output: Option<PathBuf>,
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
    #[command(args_override_self = true)]
    Validate {
        #[arg(allow_negative_numbers = true)]
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
        /// Portable view file to verify against the current model snapshot.
        #[arg(long, allow_negative_numbers = true)]
        view: Option<PathBuf>,
        /// Raw Graphify artifact or observations bundle for model-bound integration.
        #[arg(long, allow_negative_numbers = true)]
        graphify: Option<PathBuf>,
        #[arg(long, allow_negative_numbers = true)]
        graphify_profile: Option<String>,
        #[arg(long, allow_negative_numbers = true)]
        graphify_producer_version: Option<String>,
        /// Compare the original and reconstructed values in an import audit.
        #[arg(long, allow_negative_numbers = true)]
        import_audit: Option<PathBuf>,
        /// Explain readiness diagnostics and their scope paths on stderr.
        #[arg(long)]
        explain: bool,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

// Match the CLI's pathlib argument spelling without resolving symlinks or
// cancelling parent-directory components, which can change which file is read.
fn cli_path(path: PathBuf) -> PathBuf {
    let mut normalized: PathBuf = path
        .components()
        .filter(|part| *part != std::path::Component::CurDir)
        .collect();
    if normalized.as_os_str().is_empty() {
        normalized.push(".");
    }
    #[cfg(unix)]
    if path.as_os_str().as_encoded_bytes().starts_with(b"//")
        && !path.as_os_str().as_encoded_bytes().starts_with(b"///")
        && let Ok(relative) = normalized.strip_prefix("/")
    {
        return Path::new("//").join(relative);
    }
    normalized
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

fn write_text(output: Option<&Path>, text: &str) -> io::Result<()> {
    if let Some(path) = output {
        File::create_new(path)?.write_all(text.as_bytes())
    } else {
        io::stdout().lock().write_all(text.as_bytes())
    }
}

fn tool_input(path: Option<PathBuf>) -> io::Result<Option<gdd::input::Value>> {
    let Some(path) = path else {
        return Ok(None);
    };
    match read_input(path)? {
        gdd::input::Value::Null => Ok(None),
        value => Ok(Some(value)),
    }
}

fn read_input(path: PathBuf) -> io::Result<gdd::input::Value> {
    gdd::input::parse(&std::fs::read(path)?).map_err(io::Error::other)
}

fn workflow_command(command: WorkflowCommand) -> io::Result<ExitCode> {
    let args = match &command {
        WorkflowCommand::Plan { args, .. }
        | WorkflowCommand::Verify { args, .. }
        | WorkflowCommand::Create { args, .. }
        | WorkflowCommand::Event { args, .. }
        | WorkflowCommand::Status { args, .. } => args,
    };
    let path = cli_path(args.path.clone());
    let output = args.output.clone().map(cli_path);
    if let Some(output) = &output
        && output.exists()
    {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Output already exists: {}", output.display()),
        ));
    }
    let host = tool_input(args.host_context.clone().map(cli_path))?;
    let code = match command {
        WorkflowCommand::Plan { request, .. } => {
            let input = read_input(cli_path(request))?;
            let receipt = gdd::workflow::prepare_workflow(&path, &input, host.as_ref())?;
            if let Some(output) = &output {
                write_output(Some(output), &receipt.artifact)?;
            }
            write_output(None, &receipt)?;
            receipt.report.exit_code()
        }
        WorkflowCommand::Verify { plan, .. } => {
            let input = read_input(cli_path(plan))?;
            let report = gdd::workflow::verify_workflow(&path, &input, host.as_ref())?;
            write_output(output.as_deref(), &report)?;
            report.exit_code()
        }
        WorkflowCommand::Create { plan, run, .. } => {
            let plan = read_input(cli_path(plan))?;
            let result =
                gdd::workflow_run::create_run(&path, &plan, &cli_path(run), host.as_ref())?;
            write_output(output.as_deref(), &result)?;
            result.report.exit_code()
        }
        WorkflowCommand::Event { run, event, .. } => {
            let event = read_input(cli_path(event))?;
            let result =
                gdd::workflow_run::advance_run(&path, &cli_path(run), &event, host.as_ref())?;
            write_output(output.as_deref(), &result)?;
            result.exit_code()
        }
        WorkflowCommand::Status { run, .. } => {
            let result = gdd::workflow_run::run_status(&path, &cli_path(run), host.as_ref())?;
            write_output(output.as_deref(), &result)?;
            result.report.exit_code()
        }
    };
    Ok(ExitCode::from(code))
}

fn graphify_command(command: GraphifyCommand) -> io::Result<ExitCode> {
    use gdd::graphify::{GraphifyOptions, export_graphify, import_graphify, verify_graphify};
    let report = match command {
        GraphifyCommand::Import {
            path,
            output,
            profile,
            producer_version,
            code_revision,
            source_id,
        } => import_graphify(
            &cli_path(path),
            &cli_path(output),
            GraphifyOptions {
                profile: Some(&profile),
                producer_version: producer_version.as_deref(),
                code_revision: code_revision.as_deref(),
                source_id: &source_id,
                ..Default::default()
            },
        )?,
        GraphifyCommand::Validate {
            path,
            profile,
            producer_version,
        } => verify_graphify(
            &cli_path(path),
            profile.as_deref(),
            producer_version.as_deref(),
        )?,
        GraphifyCommand::Propose {
            path,
            mapping,
            output,
            format,
        } => {
            let mapping = validation::to_json(&read_input(cli_path(mapping))?)?;
            let proposal = gdd::graphify::propose_graphify_to_file(
                &cli_path(path),
                &mapping,
                &cli_path(output),
                &format,
            )?;
            write_output(None, &proposal)?;
            return Ok(ExitCode::SUCCESS);
        }
        GraphifyCommand::Export { path, output } => {
            export_graphify(&cli_path(path), &cli_path(output))?
        }
    };
    write_output(None, &report)?;
    Ok(ExitCode::from(report.exit_code()))
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
        Command::Workflow { command } => return workflow_command(command),
        Command::CheckCode {
            path,
            root,
            implementation,
            profile,
            python_runtime,
            records,
            scope,
            exclude,
            output,
        } => {
            use gdd::code_check::{CodeCheckOptions, PythonRuntime, check_code};
            let runtime = match python_runtime {
                Some(path) => PythonRuntime::from_json(&std::fs::read(cli_path(path))?)?,
                None => PythonRuntime::reference()?,
            };
            let records = records.map(cli_path);
            let report = check_code(
                &cli_path(path),
                CodeCheckOptions {
                    root: &cli_path(root),
                    implementation: &implementation,
                    profile: &profile,
                    records: records.as_deref(),
                    scope: &scope,
                    exclude: &exclude,
                    runtime: &runtime,
                },
            )?;
            write_output(output.map(cli_path).as_deref(), &report)?;
            let code = match report["result"].as_str() {
                Some("PASS") => 0,
                Some("FAIL") => 1,
                _ => 2,
            };
            return Ok(ExitCode::from(code));
        }
        Command::Graphify { command } => return graphify_command(command),
        Command::Status {
            path,
            run,
            scope,
            host_context,
            implementation,
            code_revision,
            activity,
            format,
            output,
        } => {
            let path = cli_path(path);
            let output = output.map(cli_path);
            let host = tool_input(host_context.map(cli_path))?;
            let options = gdd::status::StatusOptions {
                scope: &scope,
                host_context: host.as_ref(),
                implementation: implementation.as_deref(),
                code_revision: code_revision.as_deref(),
                activity,
            };
            let status = match run {
                Some(directory) => {
                    gdd::status::project_status_with_run(&path, &cli_path(directory), options)?
                }
                None => gdd::status::project_status(&path, options)?,
            };
            match format {
                DisplayFormat::Json => write_output(output.as_deref(), &status)?,
                DisplayFormat::Text => {
                    let text = gdd::status::status_text(&status);
                    write_text(output.as_deref(), &text)?;
                }
            }
            return Ok(ExitCode::from(status.exit_code));
        }
        Command::Diff {
            base,
            head,
            format,
            output,
        } => {
            let result = gdd::model_diff::diff_models(&cli_path(base), &cli_path(head))?;
            let output = output.map(cli_path);
            match format {
                DisplayFormat::Json => write_output(output.as_deref(), &result)?,
                DisplayFormat::Text => {
                    write_text(output.as_deref(), &gdd::model_diff::diff_text(&result))?
                }
            }
            return Ok(ExitCode::from(result.exit_code));
        }
        Command::Impact {
            path,
            scope,
            implementation,
            format,
            output,
        } => {
            let report =
                gdd::impact::analyze_impact(&cli_path(path), &scope, implementation.as_deref())?;
            let output = output.map(cli_path);
            match format {
                DisplayFormat::Json => write_output(output.as_deref(), &report)?,
                DisplayFormat::Text => {
                    write_text(output.as_deref(), &gdd::impact::impact_text(&report))?
                }
            }
            return Ok(ExitCode::from(report.exit_code));
        }
        Command::View {
            path,
            output,
            scope,
        } => {
            let report = gdd::view::render_view(&cli_path(path), &cli_path(output), &scope)?;
            write_output(None, &report)?;
        }
        Command::Perspectives { path, output } => {
            let views = gdd::perspectives::perspectives(&cli_path(path))?;
            if let Some(output) = output.map(cli_path) {
                let parent = output
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                let mut staging = tempfile::Builder::new();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    staging.permissions(std::fs::Permissions::from_mode(0o666));
                }
                let mut staged = staging.prefix(".gdd-perspectives-").tempfile_in(parent)?;
                let mut writer = io::BufWriter::new(staged.as_file_mut());
                write_json(&mut writer, &views)?;
                writer.flush()?;
                drop(writer);
                staged.persist_noclobber(output)?;
            } else {
                write_output(None, &views)?;
            }
        }
        Command::Boundaries { path, output } => {
            let model = gdd::projection::load_projection_bundle(&cli_path(path))?;
            let view = gdd::boundaries::boundary_view(&model, None)?;
            write_output(output.map(cli_path).as_deref(), &view)?;
        }
        Command::DesignReview {
            path,
            review,
            implementation,
            code_revision,
            scope,
            output,
        } => {
            let model = gdd::projection::load_projection_bundle(&cli_path(path))?;
            let review = tool_input(review.map(cli_path))?;
            let status = gdd::design_review::review_design(
                &model,
                review.as_ref(),
                Some(&implementation),
                Some(&code_revision),
                (!scope.is_empty()).then_some(scope.as_slice()),
            )?;
            write_output(output.map(cli_path).as_deref(), &status)?;
            return Ok(ExitCode::from(status.exit_code()));
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
            view,
            graphify,
            graphify_profile,
            graphify_producer_version,
            import_audit,
            explain,
            output,
        } => {
            let host = tool_input(host_context)?;
            let package = tool_input(package)?;
            let parallel = tool_input(parallel_plan)?;
            let audit = tool_input(import_audit.map(cli_path))?;
            let graphify = graphify.map(cli_path);
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
            let report = validation::validate_paths_with_integrations(
                &cli_path(path),
                model.map(cli_path).as_deref(),
                options,
                validation::GraphifyValidationOptions {
                    path: graphify.as_deref(),
                    profile: graphify_profile.as_deref(),
                    producer_version: graphify_producer_version.as_deref(),
                    import_audit: audit.as_ref(),
                },
                view.map(cli_path).as_deref(),
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
            if let Some(journal) = error
                .get_ref()
                .and_then(|e| e.downcast_ref::<gdd::workflow_run::JournalError>())
            {
                return ExitCode::from(journal.exit_code());
            }
            if let Some(projection) = error
                .get_ref()
                .and_then(|e| e.downcast_ref::<gdd::projection::ProjectionError>())
            {
                return ExitCode::from(projection.exit_code);
            }
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
