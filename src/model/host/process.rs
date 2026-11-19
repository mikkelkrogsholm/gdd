//! Collect a caller-selected checker's output and termination status.

use std::{
    fs::File,
    io::{self, Read},
    process::{Child, Command, ExitStatus, Output},
    thread,
    time::{Duration, Instant},
};
use tempfile::NamedTempFile;

#[derive(Debug)]
pub enum Execution {
    Completed(Output),
    TimedOut { stdout: Vec<u8>, stderr: Vec<u8> },
    NotStarted(io::Error),
}

// Use independent file positions for the collector and checker. A descendant
// can inherit the writer without blocking collection or sharing the reader's seek.
fn capture_file() -> io::Result<(File, File)> {
    let reader = NamedTempFile::new()?;
    let writer = reader.reopen()?;
    Ok((reader.into_file(), writer))
}

fn captured(reader: File) -> io::Result<Vec<u8>> {
    let length = reader.metadata()?.len();
    let mut bytes = Vec::new();
    reader.take(length).read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn wait(child: &mut Child, timeout: Duration) -> io::Result<Option<ExitStatus>> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(Some(status)),
            Ok(None) => (),
            Err(error) => {
                if child.kill().is_ok() {
                    child.wait()?;
                }
                return Err(error);
            }
        }
        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            child.kill()?;
            child.wait()?;
            return Ok(None);
        }
        thread::sleep(remaining.min(Duration::from_millis(10)));
    }
}

/// Redirect stdout and stderr to private files. On timeout, kill and reap the
/// selected child, then collect the bytes present even if descendants still run.
/// Capture and wait errors are returned separately from a failure to spawn.
pub fn collect(mut command: Command, timeout: Duration) -> io::Result<Execution> {
    let (stdout, stdout_writer) = capture_file()?;
    let (stderr, stderr_writer) = capture_file()?;
    command.stdout(stdout_writer).stderr(stderr_writer);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return Ok(Execution::NotStarted(error)),
    };
    // The collector has no input to send through a caller-requested stdin pipe.
    drop(child.stdin.take());
    let status = wait(&mut child, timeout)?;
    let stdout = captured(stdout)?;
    let stderr = captured(stderr)?;
    Ok(match status {
        Some(status) => Execution::Completed(Output {
            status,
            stdout,
            stderr,
        }),
        None => Execution::TimedOut { stdout, stderr },
    })
}

#[cfg(test)]
mod tests;
