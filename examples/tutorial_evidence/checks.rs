//! The literal check and receipt interpretation selected by this tutorial's caller.

use super::{invalid, read, stage_dir, write_json};
use gdd::host::{
    process::{Execution, run_checked},
    snapshot_code,
};
use serde_json::{Value, json};
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub const FILES: &[&str] = &["Cargo.toml", "Cargo.lock", "src/lib.rs", "src/main.rs"];
const ARGV: &[&str] = &[
    "cargo",
    "run",
    "--quiet",
    "--offline",
    "--locked",
    "--manifest-path",
    "Cargo.toml",
    "--target-dir",
    "target",
    "--bin",
    "gdd-tutorial-add",
];

#[derive(Clone, Copy)]
enum Case {
    Current,
    Mutant,
}

impl Case {
    fn source(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Mutant => "mutant",
        }
    }

    fn receipt(self) -> &'static str {
        match self {
            Self::Current => "pass.receipt.json",
            Self::Mutant => "negative.receipt.json",
        }
    }

    fn expected(self) -> (&'static str, i32, &'static str, &'static str) {
        match self {
            Self::Current => ("PASS", 0, "PASS: all five integer sums matched\n", ""),
            Self::Mutant => ("FAIL", 1, "", "FAIL: -7 + 7: expected 0, got -14\n"),
        }
    }
}

pub fn copy_sources(from: &Path, to: &Path) -> io::Result<()> {
    let original = snapshot_code(from, FILES)?;
    fs::create_dir(to)?;
    fs::create_dir(to.join("src"))?;
    for name in FILES {
        fs::copy(from.join(name), to.join(name))?;
    }
    if original != snapshot_code(to, FILES)? {
        return Err(invalid("The selected sources changed during copying."));
    }
    Ok(())
}

fn subtraction(current: &Path) -> io::Result<String> {
    let lib = fs::read_to_string(current.join("src/lib.rs"))?;
    if lib.matches("left + right").count() != 1 {
        return Err(invalid(
            "Expected exactly one tutorial addition expression.",
        ));
    }
    Ok(lib.replace("left + right", "left - right"))
}

fn version(tool: &str, source: &Path) -> io::Result<String> {
    let output = Command::new(tool)
        .arg("--version")
        .current_dir(source)
        .output()?;
    let version = String::from_utf8(output.stdout).map_err(io::Error::other)?;
    if !output.status.success() || !version.starts_with(&format!("{tool} ")) {
        return Err(invalid(&format!("Could not observe {tool} --version.")));
    }
    Ok(version.trim_end().to_owned())
}

fn runner(source: &Path) -> io::Result<Value> {
    Ok(json!({
        "name": "gdd tutorial_evidence",
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "cargo": version("cargo", source)?,
        "rustc": version("rustc", source)?,
    }))
}

fn timestamp(time: SystemTime) -> io::Result<String> {
    OffsetDateTime::from(time)
        .format(&Rfc3339)
        .map_err(io::Error::other)
}

fn read_time(value: &Value) -> io::Result<OffsetDateTime> {
    let text = value
        .as_str()
        .ok_or_else(|| invalid("Expected a UTC timestamp."))?;
    OffsetDateTime::parse(text, &Rfc3339).map_err(io::Error::other)
}

fn execute(runs: &Path, case: Case) -> io::Result<()> {
    let source = runs.join(case.source());
    let runner = runner(&source)?;
    // This target is exclusive to this build. Keep sources and receipts for review.
    let target = stage_dir(&source, "target")?;
    let mut cargo = Command::new(ARGV[0]);
    cargo
        .args(&ARGV[1..])
        .env("RUSTC", "rustc")
        .env("CARGO_BUILD_BUILD_DIR", target.path().canonicalize()?);
    let checked = run_checked(
        &source,
        FILES,
        &runs.join(case.receipt()),
        cargo,
        Duration::from_secs(120),
    )?;
    let (executed, code, stdout, stderr, mut reason) = match checked.execution {
        Execution::Completed(output) => (
            true,
            output.status.code(),
            output.stdout,
            output.stderr,
            "Cargo and checker completed.".to_owned(),
        ),
        Execution::TimedOut { stdout, stderr } => {
            (true, None, stdout, stderr, "Cargo timed out.".to_owned())
        }
        Execution::NotStarted(error) => (
            false,
            None,
            Vec::new(),
            Vec::new(),
            format!("Could not start Cargo: {error}"),
        ),
    };
    let stdout = String::from_utf8_lossy(&stdout);
    let stderr = String::from_utf8_lossy(&stderr);
    let mut result = "UNKNOWN";
    match &checked.after {
        Ok(after) if *after == checked.before => {
            for recognized in [Case::Current, Case::Mutant] {
                let (label, expected_code, out, err) = recognized.expected();
                if code == Some(expected_code) && stdout == out && stderr == err {
                    result = label;
                }
            }
            if result == "UNKNOWN" && code.is_some() {
                reason = "Cargo did not return a recognized tutorial checker result.".to_owned();
            }
        }
        Ok(_) => reason = "Selected sources changed during execution.".to_owned(),
        Err(error) => reason = format!("Selected sources became unresolved: {error}"),
    }
    write_json(
        checked.receipt,
        &json!({
            "format": "gdd-host-check/0.1", "runner": runner, "command": ARGV, "cwd": source,
            "started_at": timestamp(checked.started_at)?, "finished_at": timestamp(checked.finished_at)?,
            "snapshot": checked.before, "code_revision": checked.before.revision,
            "code_revision_after": checked.after.ok().map(|after| after.revision),
            "executed": executed, "result": result, "exit_code": code, "reason": reason,
            "stdout": stdout, "stderr": stderr,
        }),
    )?;
    if result != case.expected().0 {
        return Err(invalid(&format!(
            "Unexpected {result} for {}. Inspect {}",
            case.source(),
            runs.join(case.receipt()).display()
        )));
    }
    Ok(())
}

pub fn run(plan: &Path, package: &Path) -> io::Result<()> {
    let runs = stage_dir(plan, "runs")?;
    let current = runs.path().join(Case::Current.source());
    let mutant = runs.path().join(Case::Mutant.source());
    copy_sources(package, &current)?;
    copy_sources(&current, &mutant)?;
    fs::write(mutant.join("src/lib.rs"), subtraction(&current)?)?;
    // Preserve every attempted checker execution, including an unsuccessful build.
    let output = runs.keep();
    execute(&output, Case::Mutant)?;
    execute(&output, Case::Current)?;
    println!(
        "Review the two source trees and negative.receipt.json and pass.receipt.json in {}",
        output.display()
    );
    Ok(())
}

fn review_case(runs: &Path, case: Case, runner: &Value) -> io::Result<()> {
    let source = runs.join(case.source());
    let receipt = read(&runs.join(case.receipt()))?;
    let snapshot = snapshot_code(&source, FILES)?;
    let (result, code, stdout, stderr) = case.expected();
    let started = read_time(&receipt["started_at"])?;
    let finished = read_time(&receipt["finished_at"])?;
    if !started.offset().is_utc()
        || !finished.offset().is_utc()
        || started > finished
        || receipt["format"] != "gdd-host-check/0.1"
        || receipt["runner"] != *runner
        || receipt["command"] != json!(ARGV)
        || receipt["cwd"] != json!(source)
        || receipt["snapshot"] != json!(snapshot)
        || receipt["code_revision"] != snapshot.revision
        || receipt["code_revision_after"] != snapshot.revision
        || receipt["executed"] != true
        || receipt["result"] != result
        || receipt["exit_code"] != code
        || receipt["stdout"] != stdout
        || receipt["stderr"] != stderr
    {
        return Err(invalid(&format!(
            "{} does not match its current sources, command, toolchain or expected result.",
            case.receipt()
        )));
    }
    Ok(())
}

pub fn review(runs: &Path) -> io::Result<Value> {
    let current = runs.join(Case::Current.source());
    let mutant = runs.join(Case::Mutant.source());
    let environment = runner(&current)?;
    review_case(runs, Case::Current, &environment)?;
    review_case(runs, Case::Mutant, &environment)?;
    for name in FILES {
        let expected = if *name == "src/lib.rs" {
            subtraction(&current)?.into_bytes()
        } else {
            fs::read(current.join(name))?
        };
        if fs::read(mutant.join(name))? != expected {
            return Err(invalid(
                "The control must change only the addition expression.",
            ));
        }
    }
    Ok(environment)
}

#[test]
fn failed_cargo_build_retains_its_real_diagnostic_and_refuses_a_repeat() {
    let temp = tempfile::tempdir().unwrap();
    let package = temp.path().join("broken");
    fs::create_dir_all(package.join("src")).unwrap();
    fs::write(
        package.join("Cargo.toml"),
        "[package]\nname = \"gdd-tutorial-add\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(package.join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(package.join("src/lib.rs"), "fn add(left: i32, right: i32) -> i32 { left + right }\ncompile_error!(\"broken tutorial build\");").unwrap();
    assert!(
        Command::new("cargo")
            .args(["generate-lockfile", "--offline"])
            .current_dir(&package)
            .status()
            .unwrap()
            .success()
    );
    assert!(run(temp.path(), &package).is_err());
    let path = temp.path().join("runs/negative.receipt.json");
    let receipt = read(&path).unwrap();
    assert_eq!(receipt["result"], "UNKNOWN");
    assert_eq!(receipt["executed"], true);
    assert_eq!(receipt["exit_code"], 101);
    assert!(
        receipt["stderr"]
            .as_str()
            .unwrap()
            .contains("broken tutorial build")
    );
    assert_eq!(
        run(temp.path(), &package).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(read(&path).unwrap(), receipt);
    assert!(review(&temp.path().join("runs")).is_err());
}
