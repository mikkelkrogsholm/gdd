//! Run local Rust contributor checks in the selected checkout.

use anyhow::{Context, Result, ensure};
use serde_json::{Map, Value, json};
use std::{
    env,
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::{Component, Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    time::Instant,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

mod plan;

// JSON strings cannot represent every native path or command argument.
fn os_value(value: &OsStr) -> Value {
    match value.to_str() {
        Some(text) => json!(text),
        None => {
            json!({ "display": value.to_string_lossy(), "os_encoded_bytes": value.as_encoded_bytes() })
        }
    }
}

fn timestamp() -> Result<String> {
    Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
}

fn new_output(output: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = output {
        let path = std::path::absolute(path)?;
        fs::create_dir_all(path.parent().context("output must name a new directory")?)?;
        fs::create_dir(&path)
            .with_context(|| format!("create new output directory {}", path.display()))?;
        return Ok(path.canonicalize()?);
    }
    Ok(tempfile::Builder::new()
        .prefix("gdd-local-check-")
        .tempdir()?
        .keep()
        .canonicalize()?)
}

// Resolve existing ancestor symlinks even when the target directory is still missing.
fn resolve_target(path: &Path) -> Result<PathBuf> {
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(name) => {
                resolved.push(name);
                if resolved.try_exists()? {
                    resolved = resolved.canonicalize()?;
                }
            }
            _ => resolved.push(component),
        }
    }
    Ok(resolved)
}

struct Run<'a> {
    root: &'a Path,
    output: &'a Path,
    target: PathBuf,
    cli: Option<PathBuf>,
    steps: Vec<Value>,
    toolchain: Map<String, Value>,
}

impl Run<'_> {
    fn step(&mut self, name: &str, mut command: Command) -> Result<PathBuf> {
        let stdout_name = format!("{name}.stdout.txt");
        let stderr_name = format!("{name}.stderr.txt");
        let stdout = self.output.join(&stdout_name);
        let mut stderr = File::create_new(self.output.join(&stderr_name))?;
        command
            .current_dir(self.root)
            .env("CARGO_TARGET_DIR", &self.target)
            .env_remove("GDD_NATIVE_CLI")
            .stdin(Stdio::null())
            .stdout(File::create_new(&stdout)?)
            .stderr(stderr.try_clone()?);
        if let Some(cli) = &self.cli {
            command.env("GDD_NATIVE_CLI", cli);
        }
        let argv: Vec<_> = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(os_value)
            .collect();
        let mut record = json!({
            "name": name, "command": argv, "cwd": os_value(self.root.as_os_str()),
            "started_at": timestamp()?, "stdout": stdout_name, "stderr": stderr_name,
        });
        let start = Instant::now();
        let (passed, code) = match command.status() {
            Ok(status) => {
                record["exit_status"] = json!(status.to_string());
                (status.success(), status.code())
            }
            Err(error) => {
                writeln!(stderr, "Could not run {name}: {error}")?;
                record["error"] = json!(error.to_string());
                (false, None)
            }
        };
        record["finished_at"] = json!(timestamp()?);
        record["seconds"] = json!(start.elapsed().as_secs_f64());
        record["exit_code"] = json!(code);
        record["passed"] = json!(passed);
        self.steps.push(record);
        println!(
            "{} {name} ({:.2}s)",
            if passed { "PASS" } else { "FAIL" },
            start.elapsed().as_secs_f64()
        );
        ensure!(passed, "{name} failed; see {stdout_name} and {stderr_name}");
        Ok(stdout)
    }

    fn checks(&mut self, quick: bool) -> Result<()> {
        self.target = resolve_target(&std::path::absolute(&self.target)?)?;
        ensure!(
            !self
                .target
                .starts_with(resolve_target(&self.root.join("skills/gdd"))?),
            "choose a Cargo target directory outside the portable skill"
        );
        for (name, program) in [
            ("cargo", OsString::from("cargo")),
            (
                "rustc",
                env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()),
            ),
        ] {
            let mut command = Command::new(program);
            command.args(["--version", "--verbose"]);
            let path = self.step(name, command)?;
            self.toolchain
                .insert(name.into(), json!(fs::read_to_string(path)?.trim_end()));
        }
        for (name, command) in plan::commands(quick, self.root, self.output) {
            let log = self.step(name, command)?;
            if name == "workspace-build" {
                self.cli = Some(cli_artifact(&log)?);
            }
        }
        Ok(())
    }
}

fn cli_artifact(log: &Path) -> Result<PathBuf> {
    for line in BufReader::new(File::open(log)?).lines() {
        let message: Value = serde_json::from_str(&line?)?;
        if message["reason"] == "compiler-artifact"
            && message["target"]["name"] == "gdd"
            && let Some(exe) = message["executable"].as_str()
        {
            return Ok(PathBuf::from(exe));
        }
    }
    anyhow::bail!(
        "Cargo did not report the built gdd binary in {}",
        log.display()
    )
}

pub(super) fn run(root: &Path, quick: bool, output: Option<PathBuf>) -> Result<ExitCode> {
    let target = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let output = new_output(output)?;
    println!("Local check output: {}", output.display());
    let started_at = timestamp()?;
    let start = Instant::now();
    let mut run = Run {
        root,
        output: &output,
        target,
        cli: None,
        steps: Vec::new(),
        toolchain: Map::new(),
    };
    let result = run.checks(quick);
    let mut summary = json!({
        "mode": if quick { "quick" } else { "full" },
        "checkout": os_value(root.as_os_str()), "output": os_value(output.as_os_str()),
        "cargo_target_dir": os_value(run.target.as_os_str()),
        "gdd_native_cli": run.cli.as_ref().map(|path| os_value(path.as_os_str())),
        "host": { "os": env::consts::OS, "arch": env::consts::ARCH }, "toolchain": run.toolchain,
        "started_at": started_at, "finished_at": timestamp()?, "seconds": start.elapsed().as_secs_f64(),
        "steps": run.steps, "passed": result.is_ok(),
    });
    if let Err(error) = &result {
        summary["error"] = json!(format!("{error:#}"));
    }
    let mut file = File::create_new(output.join("summary.json"))?;
    serde_json::to_writer_pretty(&mut file, &summary)?;
    writeln!(file)?;
    result?;
    Ok(ExitCode::SUCCESS)
}
