//! Exercise an installed native CLI from a temporary recipient with real checkout fixtures.

mod models;
mod observer;
mod project;

use crate::receipt;
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Output, Stdio},
};

pub(super) const TOTAL: usize = 33;

const FIXTURES: &[&str] = &[
    "examples/concert.gdd.yaml",
    "examples/concert.records.yaml",
    "examples/concert-0.3.gdd.yaml",
    "examples/concert-0.3.records.yaml",
    "examples/sources/legacy-concert.gdd.yaml",
    "examples/concert-0.3.boundaries.json",
    "examples/design-review.planned.json",
    "examples/product/greenfield.gdd.json",
    "examples/product/partial.gdd.json",
    "examples/product/refactoring.gdd.json",
    "examples/product/extensions/product-0.1.json",
    "examples/agent-native/model.gdd.json",
    "examples/agent-native/model.records.json",
    "examples/agent-native/REQUIREMENTS.md",
    "examples/agent-native/host.acceptance.json",
    "docs/tutorial/refined.gdd.json",
    "schemas/perspectives.schema.json",
];

fn stage(checkout: &Path, recipient: &Path) -> Result<()> {
    for &name in FIXTURES {
        let target = recipient.join(name);
        fs::create_dir_all(target.parent().context("fixture has no parent")?)?;
        fs::copy(checkout.join(name), target).with_context(|| format!("stage {name}"))?;
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<Value> {
    serde_json::from_slice(&fs::read(path).with_context(|| format!("read {}", path.display()))?)
        .with_context(|| format!("parse {}", path.display()))
}

#[derive(Serialize)]
struct Invocation {
    arguments: Vec<String>,
    exit_code: Option<i32>,
}

#[derive(Serialize)]
struct Control {
    name: String,
    expected_exit: i32,
    installed: Option<Invocation>,
    reference: Option<Invocation>,
    passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Clone, Copy)]
enum Program {
    Installed,
    Reference,
}

impl Program {
    fn name(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::Reference => "reference",
        }
    }
}

struct Programs {
    installed: PathBuf,
    reference: PathBuf,
}

struct Case<'a> {
    recipient: &'a Path,
    programs: &'a Programs,
    control: &'a mut Control,
}

fn excerpt(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).chars().take(1200).collect()
}

impl Case<'_> {
    fn execute(&mut self, program: Program, args: &[&str]) -> Result<Output> {
        let role = program.name();
        let (path, observation) = match program {
            Program::Installed => (&self.programs.installed, &mut self.control.installed),
            Program::Reference => (&self.programs.reference, &mut self.control.reference),
        };
        let observation = observation.insert(Invocation {
            arguments: args.iter().map(|arg| (*arg).to_owned()).collect(),
            exit_code: None,
        });
        let mut command = Command::new(path);
        command
            .current_dir(self.recipient)
            .env_clear()
            .env("HOME", self.recipient)
            .env("PATH", self.recipient.join("empty-path"))
            .stdin(Stdio::null())
            .args(args);
        #[cfg(windows)]
        if let Some(system_root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", system_root);
        }
        let output = command
            .output()
            .with_context(|| format!("start {role} {}", path.display()))?;
        observation.exit_code = output.status.code();
        ensure!(
            observation.exit_code == Some(self.control.expected_exit),
            "{role} exited with {} (expected {}); stderr: {}; stdout: {}",
            output.status,
            self.control.expected_exit,
            excerpt(&output.stderr),
            excerpt(&output.stdout)
        );
        Ok(output)
    }

    fn json(&mut self, program: Program, args: &[&str]) -> Result<Value> {
        let role = program.name();
        let output = self.execute(program, args)?;
        serde_json::from_slice(&output.stdout).with_context(|| {
            format!(
                "invalid {role} JSON; stdout: {}; stderr: {}",
                excerpt(&output.stdout),
                excerpt(&output.stderr)
            )
        })
    }

    fn compare(&mut self, args: &[&str]) -> Result<Value> {
        let observed = self.json(Program::Installed, args)?;
        let reference = self.json(Program::Reference, args)?;
        check_json("installed and reference JSON", &reference, &observed)?;
        Ok(observed)
    }
}

fn check_json(label: &str, expected: &Value, observed: &Value) -> Result<()> {
    if expected != observed {
        let diff = json_patch::diff(expected, observed);
        let first = diff.0.first().context("JSON values differ")?;
        bail!(
            "{label} differ; first difference: {}",
            excerpt(first.to_string().as_bytes())
        );
    }
    Ok(())
}

struct Runner<'a> {
    recipient: &'a Path,
    programs: &'a Programs,
    controls: &'a mut Vec<Control>,
}

impl Runner<'_> {
    fn case<T>(
        &mut self,
        name: &str,
        expected_exit: i32,
        check: impl FnOnce(&mut Case<'_>) -> Result<T>,
    ) -> Result<T> {
        let mut control = Control {
            name: name.to_owned(),
            expected_exit,
            installed: None,
            reference: None,
            passed: false,
            error: None,
        };
        let result = check(&mut Case {
            recipient: self.recipient,
            programs: self.programs,
            control: &mut control,
        })
        .with_context(|| format!("control {name}"));
        control.passed = result.is_ok();
        if let Err(error) = &result {
            control.error = Some(format!("{error:#}"));
        }
        self.controls.push(control);
        result
    }

    fn compare(&mut self, name: &str, expected_exit: i32, args: &[&str]) -> Result<Value> {
        self.case(name, expected_exit, |case| case.compare(args))
    }
}

fn identity(path: &Path, program: Program, report: &mut Value) -> Result<PathBuf> {
    let role = program.name();
    let path = path
        .canonicalize()
        .with_context(|| format!("locate {role} executable {}", path.display()))?;
    let metadata = fs::metadata(&path)
        .with_context(|| format!("inspect {role} executable {}", path.display()))?;
    ensure!(
        metadata.is_file(),
        "{role} executable must be a regular file: {}",
        path.display()
    );
    let bytes =
        fs::read(&path).with_context(|| format!("read {role} executable {}", path.display()))?;
    report[format!("{role}_sha256")] = gdd::sha256(&bytes).into();
    Ok(path)
}

pub(super) fn run(
    checkout: &Path,
    installed: &Path,
    reference: &Path,
    output: Option<PathBuf>,
) -> Result<ExitCode> {
    if let Some(path) = &output {
        match path.symlink_metadata() {
            Ok(_) => bail!("receipt already exists: {}", path.display()),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let mut report = json!({
        "format": "gdd-verify-install/0.1", "passed": false, "total_controls": TOTAL,
        "installed_sha256": null, "reference_sha256": null,
    });
    let mut controls = Vec::new();
    let result = (|| -> Result<()> {
        let installed = identity(installed, Program::Installed, &mut report);
        let reference = identity(reference, Program::Reference, &mut report);
        let programs = Programs {
            installed: installed?,
            reference: reference?,
        };
        ensure!(
            programs.installed != programs.reference,
            "supply separate installed and reference executables"
        );
        let folder = tempfile::Builder::new()
            .prefix("gdd-installed-")
            .tempdir()?;
        let recipient = folder.path().canonicalize()?;
        ensure!(
            !recipient.starts_with(checkout),
            "the temporary recipient must be outside the selected checkout"
        );
        stage(checkout, &recipient)?;
        let mut runner = Runner {
            recipient: &recipient,
            programs: &programs,
            controls: &mut controls,
        };
        models::check(&mut runner)?;
        project::check(&mut runner)?;
        observer::check(&mut runner)?;
        ensure!(
            controls.len() == TOTAL,
            "expected {TOTAL} installed controls"
        );
        Ok(())
    })();
    report["passed"] = result.is_ok().into();
    if let Err(error) = result {
        eprintln!("{error:#}");
        report["error"] = format!("{error:#}").into();
    }
    let matched = controls.iter().filter(|control| control.passed).count();
    report["controls"] = serde_json::to_value(controls)?;
    receipt::publish(&report, output, "installed CLI", matched, TOTAL)
}
