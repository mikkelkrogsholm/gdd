//! Run the fixed Cargo acceptance target and check the complete named result set.

use super::project;
use anyhow::{Context, Result, bail, ensure};
use gdd::host::{
    CodeSnapshot,
    process::{Execution, collect, run_checked},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    env,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
    time::Duration,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    Baseline,
    Discount,
    Tax,
    Quote,
    Final,
}

impl Stage {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Discount => "discount",
            Self::Tax => "tax",
            Self::Quote => "quote",
            Self::Final => "final",
        }
    }

    fn filter(self) -> Option<&'static str> {
        match self {
            Self::Discount => Some("discount::"),
            Self::Tax => Some("tax::"),
            Self::Quote => Some("quote::"),
            Self::Baseline | Self::Final => None,
        }
    }

    fn expected(self) -> BTreeMap<String, &'static str> {
        let mut tests = BTreeMap::new();
        for group in ["discount", "tax"] {
            for name in [
                "normal",
                "floor_half",
                "floor_fraction",
                "boundaries",
                "invalid",
            ] {
                tests.insert(format!("{group}::test_{name}"), "ok");
            }
        }
        for name in [
            "normal",
            "floor_after_discount",
            "tax_floor_without_discount",
            "zero_and_full_discount",
            "invalid",
            "cli_normal",
            "cli_floor",
            "cli_invalid",
        ] {
            tests.insert(format!("quote::test_{name}"), "ok");
        }
        if self == Self::Baseline {
            for name in [
                "discount::test_floor_fraction",
                "discount::test_floor_half",
                "tax::test_floor_fraction",
                "tax::test_floor_half",
                "quote::test_cli_floor",
                "quote::test_floor_after_discount",
                "quote::test_tax_floor_without_discount",
            ] {
                tests.insert(name.into(), "FAILED");
            }
        } else if matches!(self, Self::Quote | Self::Final) {
            tests.insert(
                "quote::test_cli_large_integer_floor_composition".into(),
                "ok",
            );
        }
        if let Some(filter) = self.filter() {
            tests.retain(|name, _| name.starts_with(filter));
        }
        tests
    }
}

fn tool(program: &str, root: &Path) -> Command {
    let mut command = Command::new(program);
    command.current_dir(root).env_clear().stdin(Stdio::null());
    for key in [
        "PATH",
        "HOME",
        "USERPROFILE",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "SystemRoot",
        "TEMP",
        "TMPDIR",
        "TMP",
    ] {
        if let Some(value) = env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .env("CARGO_TERM_COLOR", "never")
        .env("RUSTC", "rustc");
    command
}

// Allow five minutes for each invocation in the bundled, dependency-free project.
const TOOL_TIMEOUT: Duration = Duration::from_secs(300);

fn tool_output(execution: Execution, timeout: Duration) -> Result<Output> {
    match execution {
        Execution::Completed(output) => Ok(output),
        Execution::NotStarted(error) => Err(error).context("start the selected tool"),
        Execution::TimedOut { stdout, stderr } => bail!(
            "tool timed out after {timeout:?}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr),
        ),
    }
}

pub(super) fn versions(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut versions = BTreeMap::new();
    for program in ["cargo", "rustc"] {
        let mut command = tool(program, root);
        command.args(["--version", "--verbose"]);
        let context = || format!("run {program} for toolchain identity");
        let execution = collect(command, TOOL_TIMEOUT).with_context(context)?;
        let result = tool_output(execution, TOOL_TIMEOUT).with_context(context)?;
        ensure!(
            result.status.success(),
            "{program} version failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let version = String::from_utf8(result.stdout)?;
        ensure!(
            version.starts_with(&format!("{program} ")),
            "missing {program} version"
        );
        versions.insert(program.to_owned(), version.trim_end().to_owned());
    }
    Ok(versions)
}

fn verify(stage: Stage, exit: Option<i32>, stdout: &str) -> Result<BTreeMap<String, &str>> {
    let mut observed = BTreeMap::new();
    for (name, outcome) in stdout
        .lines()
        .filter_map(|line| line.strip_prefix("test ")?.split_once(" ... "))
    {
        ensure!(
            observed.insert(name.to_owned(), outcome).is_none(),
            "duplicate test {name}"
        );
    }
    let expected = stage.expected();
    ensure!(
        expected == observed,
        "unexpected named Cargo results: expected {expected:?}, observed {observed:?}"
    );
    let failed = expected
        .values()
        .filter(|&&value| value == "FAILED")
        .count();
    let passed = expected.len() - failed;
    let verdict = if failed == 0 { "ok" } else { "FAILED" };
    let filtered = match stage {
        Stage::Discount | Stage::Tax => 13,
        Stage::Quote => 10,
        _ => 0,
    };
    let summary = format!(
        "test result: {verdict}. {passed} passed; {failed} failed; 0 ignored; 0 measured; {filtered} filtered out;"
    );
    ensure!(
        stdout
            .lines()
            .filter(|line| line.starts_with("test result:"))
            .count()
            == 1
            && stdout.lines().any(|line| line.starts_with(&summary)),
        "missing completed Cargo test summary: {summary}"
    );
    ensure!(
        exit == Some(if failed == 0 { 0 } else { 101 }),
        "unexpected Cargo process exit {exit:?}"
    );
    Ok(observed)
}

pub(super) struct TestRun {
    pub snapshot: CodeSnapshot,
    pub test_exit: u8,
    pub cargo_exit: i32,
}

pub(super) fn run(
    root: &Path,
    target: &Path,
    stage: Stage,
    versions: &BTreeMap<String, String>,
) -> Result<TestRun> {
    let mut args = vec!["test", "--locked", "--offline", "--test", "acceptance"];
    args.extend(stage.filter());
    args.extend(["--", "--format=pretty", "--test-threads=1"]);
    let mut command = tool("cargo", root);
    command
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_BUILD_BUILD_DIR", target)
        .args(&args);
    let path = root.join(format!("evidence/{}.run.json", stage.name()));
    let context = || format!("run the {} Cargo acceptance target", stage.name());
    let mut run = run_checked(root, &project::source_names(), &path, command, TOOL_TIMEOUT)
        .with_context(context)?;
    let result = tool_output(run.execution, TOOL_TIMEOUT).with_context(context)?;
    let before = run.before;
    let after = run
        .after
        .context("resolve the selected sources after Cargo execution")?;
    let started_at = OffsetDateTime::from(run.started_at).format(&Rfc3339)?;
    let finished_at = OffsetDateTime::from(run.finished_at).format(&Rfc3339)?;
    ensure!(
        before == after,
        "Cargo execution changed the selected sources"
    );
    let stdout = String::from_utf8(result.stdout)?;
    let stderr = String::from_utf8(result.stderr)?;
    let tests = verify(stage, result.status.code(), &stdout)
        .with_context(|| format!("{} Cargo check failed\n{stdout}\n{stderr}", stage.name()))?;
    let cargo_exit = result
        .status
        .code()
        .context("Cargo terminated without an exit code")?;
    let test_exit = u8::from(cargo_exit != 0);
    let command: Vec<_> = std::iter::once("cargo").chain(args).collect();
    let receipt = json!({
        "format": "quote-demo-cargo-run/0.1", "executed": true,
        "argv": command, "cwd": ".", "started_at": started_at, "finished_at": finished_at,
        "result": if test_exit == 0 { "PASS" } else { "FAIL" },
        "exit_code": cargo_exit, "test_exit": test_exit, "tests": tests,
        "stdout": stdout, "stderr": stderr, "runner": versions,
        "snapshot": before, "code_revision": before.revision, "code_revision_after": after.revision,
    });
    let mut bytes = serde_json::to_vec_pretty(&receipt)?;
    bytes.push(b'\n');
    run.receipt
        .write_all(&bytes)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(TestRun {
        snapshot: after,
        test_exit,
        cargo_exit,
    })
}

#[cfg(test)]
mod tests {
    use super::{Stage, collect, tool_output, verify};
    use std::{env, process::Command, thread, time::Duration};

    #[test]
    fn timed_out_tool_reports_partial_streams() {
        // Invoke this test as the selected tool to simulate an incomplete Cargo run.
        const CHILD: &str = "GDD_TEST_INCOMPLETE_CARGO_OUTPUT";
        if env::var_os(CHILD).is_some() {
            println!("test discount::test_normal ... ok");
            eprintln!("pending acceptance tests");
            thread::sleep(Duration::from_secs(15));
            return;
        }
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "workflow_demo::checks::tests::timed_out_tool_reports_partial_streams",
                "--nocapture",
            ])
            .env(CHILD, "1");
        let timeout = Duration::from_secs(1);
        let error = tool_output(collect(command, timeout).unwrap(), timeout)
            .unwrap_err()
            .to_string();
        assert!(error.contains("timed out"));
        assert!(error.contains("test discount::test_normal ... ok"));
        assert!(error.contains("pending acceptance tests"));
    }

    #[test]
    fn cargo_failure_requires_the_exact_completed_negative_control() {
        let mut output = String::new();
        for (name, outcome) in Stage::Baseline.expected() {
            output.push_str(&format!("test {name} ... {outcome}\n"));
        }
        output.push_str("test result: FAILED. 11 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n");
        assert!(verify(Stage::Baseline, Some(101), &output).is_ok());
        assert!(verify(Stage::Baseline, Some(101), "error: compilation failed").is_err());
        assert!(verify(Stage::Baseline, Some(0), &output).is_err());
        assert!(
            verify(
                Stage::Baseline,
                Some(101),
                &output.replace("11 passed", "10 passed")
            )
            .is_err()
        );
        let unrelated = output
            .replace(
                "quote::test_cli_floor ... FAILED",
                "quote::test_cli_floor ... ok",
            )
            .replace("quote::test_normal ... ok", "quote::test_normal ... FAILED");
        assert!(verify(Stage::Baseline, Some(101), &unrelated).is_err());
    }
}
