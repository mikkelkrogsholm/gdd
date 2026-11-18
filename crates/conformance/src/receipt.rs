//! Publish complete native case receipts without replacing existing files.

use anyhow::{Context, Result};
use serde::Serialize;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub(super) fn publish(
    run: &impl Serialize,
    output: Option<PathBuf>,
    suite: &str,
    matched: usize,
    total: usize,
) -> Result<ExitCode> {
    let mut bytes = serde_json::to_vec_pretty(run)?;
    bytes.push(b'\n');
    if let Some(path) = output {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut builder = tempfile::Builder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(std::fs::Permissions::from_mode(0o666));
        }
        let mut staging = builder.tempfile_in(parent)?;
        staging.write_all(&bytes)?;
        staging
            .persist_noclobber(&path)
            .with_context(|| format!("create receipt {}", path.display()))?;
        println!("{matched}/{total} native {suite} expectations matched");
    } else {
        io::stdout().lock().write_all(&bytes)?;
    }
    Ok(if matched == total {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
