//! The existing version 0.1 wire format, explicitly selected by a trusted host.

use super::invalid;
use super::process::{Execution, run_checked};
use serde_json::{Map, Value, json};
use std::{
    ffi::OsStr,
    io::{self, BufWriter, Write},
    path::Path,
    process::{Command, ExitStatus},
    time::{Duration, SystemTime},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

fn timestamp(time: SystemTime) -> io::Result<String> {
    OffsetDateTime::from(time)
        .format(&Rfc3339)
        .map_err(io::Error::other)
}

fn exit_code(status: ExitStatus) -> Option<i64> {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status
            .code()
            .or_else(|| status.signal().map(|s| -s))
            .map(i64::from)
    }
    #[cfg(windows)]
    {
        // A Windows process returns an unsigned DWORD.
        status.code().map(|code| i64::from(code as u32))
    }
    #[cfg(not(any(unix, windows)))]
    status.code().map(i64::from)
}

fn completed_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

/// Literal UTF-8 argv and overrides to the inherited process environment.
#[derive(Clone, Copy)]
pub struct CheckCommand<'a> {
    /// The first entry names the executable. Each remaining entry is one argument.
    pub argv: &'a [&'a str],
    /// Native environment keys and values set for this check only.
    pub env: &'a [(&'a OsStr, &'a OsStr)],
}

/// Run a trusted caller's literal command and publish a fresh `gdd-host-check/0.1`
/// receipt. Supply the actual runner metadata for this invocation. The command's
/// program and arguments must be nonempty UTF-8 strings without NUL, and its
/// timeout must be positive and at most one day. The checker runs in the canonical
/// source root with stdout and stderr captured separately.
///
/// The receipt reports exit 0 as PASS and exit 1 as FAIL for unchanged sources.
/// The caller independently verifies assertion coverage and any negative controls.
/// A reserved output survives subsequent I/O errors, preventing accidental reruns.
pub fn execute_check_v01(
    root: &Path,
    files: &[&str],
    output: &Path,
    command: CheckCommand<'_>,
    timeout: Duration,
    runner: &Map<String, Value>,
) -> io::Result<Value> {
    let argv = command.argv;
    if argv.is_empty() || argv.iter().any(|arg| arg.is_empty() || arg.contains('\0')) {
        return Err(invalid("Select nonempty UTF-8 arguments without NUL."));
    }
    if timeout.is_zero() || timeout > Duration::from_secs(86400) {
        return Err(invalid("Choose a positive timeout of at most one day."));
    }
    let root = root.canonicalize()?;
    let cwd = crate::receipt_path(&root)?;
    let mut process = Command::new(argv[0]);
    process.args(&argv[1..]).envs(command.env.iter().copied());
    let checked = run_checked(&root, files, output, process, timeout)?;
    let (executed, code, stdout, stderr, mut reason) = match checked.execution {
        Execution::Completed(output) => (
            true,
            exit_code(output.status),
            completed_text(&output.stdout),
            completed_text(&output.stderr),
            "Actual caller-selected checker process completed.".to_owned(),
        ),
        Execution::TimedOut { stdout, stderr } => (
            true,
            None,
            String::from_utf8_lossy(&stdout).into_owned(),
            String::from_utf8_lossy(&stderr).into_owned(),
            "The checker timed out; no completed result is established.".to_owned(),
        ),
        Execution::NotStarted(error) => (
            false,
            None,
            String::new(),
            String::new(),
            format!("The checker could not be started: {error}"),
        ),
    };
    let result = match &checked.after {
        Ok(after) if *after == checked.before => match code {
            Some(0) => "PASS",
            Some(1) => "FAIL",
            _ => "UNKNOWN",
        },
        Ok(_) => {
            reason = "Selected source bytes changed during execution.".to_owned();
            "UNKNOWN"
        }
        Err(error) => {
            reason = format!("Selected source bytes became unresolved: {error}");
            "UNKNOWN"
        }
    };
    let mut receipt = json!({
        "format": "gdd-host-check/0.1", "command": argv, "cwd": cwd,
        "started_at": timestamp(checked.started_at)?, "finished_at": timestamp(checked.finished_at)?,
        "snapshot": checked.before, "code_revision": checked.before.revision,
        "code_revision_after": checked.after.ok().map(|after| after.revision),
        "executed": executed, "exit_code": code, "result": result, "reason": reason,
        "stdout": stdout, "stderr": stderr,
    });
    receipt["runner"] = Value::Object(runner.clone());
    let mut file = BufWriter::new(checked.receipt);
    serde_json::to_writer_pretty(&mut file, &receipt)?;
    writeln!(file)?;
    file.flush()?;
    Ok(receipt)
}
