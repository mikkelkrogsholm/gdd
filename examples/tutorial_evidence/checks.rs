//! The literal check and receipt interpretation selected by this tutorial's caller.

use super::{invalid, read, stage_dir};
use gdd::host::{CheckCommand, execute_check_v01, snapshot_code};
use serde_json::{Value, json};
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
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

    fn matches(self, receipt: &Value) -> bool {
        let (result, code, stdout, stderr) = self.expected();
        receipt["executed"] == true
            && receipt["result"] == result
            && receipt["exit_code"] == code
            && receipt["stdout"] == stdout
            && receipt["stderr"] == stderr
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
    let build_dir = target.path().canonicalize()?;
    let cargo = CheckCommand {
        argv: ARGV,
        env: &[
            ("RUSTC".as_ref(), "rustc".as_ref()),
            ("CARGO_BUILD_BUILD_DIR".as_ref(), build_dir.as_os_str()),
        ],
    };
    let path = runs.join(case.receipt());
    let receipt = execute_check_v01(
        &source,
        FILES,
        &path,
        cargo,
        Duration::from_secs(120),
        runner.as_object().expect("tutorial runner object"),
    )?;
    if !case.matches(&receipt) {
        return Err(invalid(&format!(
            "Unexpected checker result for {}. Inspect {}",
            case.source(),
            path.display()
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
    let started = read_time(&receipt["started_at"])?;
    let finished = read_time(&receipt["finished_at"])?;
    let cwd = receipt["cwd"]
        .as_str()
        .map(Path::new)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| invalid("Expected an absolute checker working directory."))?;
    if !started.offset().is_utc()
        || !finished.offset().is_utc()
        || started > finished
        || receipt["format"] != "gdd-host-check/0.1"
        || receipt["runner"] != *runner
        || receipt["command"] != json!(ARGV)
        || cwd.canonicalize()? != source.canonicalize()?
        || receipt["snapshot"] != json!(snapshot)
        || receipt["code_revision"] != snapshot.revision
        || receipt["code_revision_after"] != snapshot.revision
        || !case.matches(&receipt)
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

#[cfg(test)]
fn fixture_package(package: &Path, library: &str, main: &str) {
    fs::create_dir_all(package.join("src")).unwrap();
    fs::write(
        package.join("Cargo.toml"),
        "[package]\nname = \"gdd-tutorial-add\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(package.join("src/main.rs"), main).unwrap();
    fs::write(package.join("src/lib.rs"), library).unwrap();
    assert!(
        Command::new("cargo")
            .args(["generate-lockfile", "--offline"])
            .current_dir(package)
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn failed_cargo_build_retains_its_real_diagnostic_and_refuses_a_repeat() {
    let temp = tempfile::tempdir().unwrap();
    let package = temp.path().join("broken");
    fixture_package(
        &package,
        "fn add(left: i32, right: i32) -> i32 { left + right }\ncompile_error!(\"broken tutorial build\");",
        "fn main() {}",
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

#[test]
fn expected_exit_without_the_expected_assertion_is_refused_at_run_and_review() {
    let temp = tempfile::tempdir().unwrap();
    for case in [Case::Current, Case::Mutant] {
        let source = temp.path().join(case.source());
        let (label, code, _, _) = case.expected();
        fixture_package(
            &source,
            "",
            &format!(
                "fn main() {{ println!(\"unexpected checker output\"); std::process::exit({code}); }}"
            ),
        );
        assert!(execute(temp.path(), case).is_err());
        let receipt = read(&temp.path().join(case.receipt())).unwrap();
        assert_eq!(receipt["result"], label);
        assert_eq!(receipt["exit_code"], code);
        assert_eq!(receipt["stdout"], "unexpected checker output\n");
        assert!(review_case(temp.path(), case, &runner(&source).unwrap()).is_err());
    }
}
