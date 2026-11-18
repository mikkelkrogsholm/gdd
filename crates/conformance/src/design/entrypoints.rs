//! Verify the actual offline view artifact and the public native CLI.
use super::*;
use gdd::view::{read_view, read_view_bytes, render_view, verify_view};
use std::ffi::OsStr;
use std::process::Command;

/// Resolve a binary from this checkout's Cargo build, or exercise a specified installation.
fn cli(directory: &Path) -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("GDD_NATIVE_CLI") {
        let path = PathBuf::from(path);
        ensure!(
            path.is_absolute(),
            "GDD_NATIVE_CLI must be an absolute path"
        );
        return Ok(path);
    }
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let build = Command::new(cargo)
        .args([
            "build",
            "--offline",
            "--locked",
            "--message-format=json",
            "-p",
            "gdd",
            "--bin",
            "gdd",
        ])
        .current_dir(directory.join(".."))
        .output()
        .context("launch Cargo to locate the native gdd binary")?;
    ensure!(
        build.status.success(),
        "native gdd build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    for line in std::str::from_utf8(&build.stdout)?.lines() {
        let message: Value = serde_json::from_str(line)?;
        if message["reason"] == "compiler-artifact"
            && message["target"]["name"] == "gdd"
            && let Some(path) = message["executable"].as_str()
        {
            return Ok(PathBuf::from(path));
        }
    }
    anyhow::bail!("Cargo did not identify the native gdd executable")
}

fn run(cli: &Path, operation: &str, model: &Path, args: &[&OsStr]) -> Result<Value> {
    let output = Command::new(cli)
        .arg(operation)
        .arg(model)
        .args(args)
        .output()
        .with_context(|| format!("launch {} {operation}", cli.display()))?;
    ensure!(
        output.status.success(),
        "gdd {operation} exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).with_context(|| format!("parse gdd {operation} output"))
}

fn integration_pass(report: &Value) -> Result<()> {
    let checks = report["checks"].as_array().context("missing view checks")?;
    ensure!(
        checks.iter().any(
            |c| c["check_id"] == "integration.visualization" && c["dimension"] == "integration"
        )
    );
    ensure!(
        checks
            .iter()
            .filter(|c| c["dimension"] == "integration")
            .all(|c| c["result"] == "PASS"),
        "view integration failed: {report}"
    );
    Ok(())
}

pub(super) fn new_viewer_and_cli_operations(model: &mut Model, _: &str) -> Result<()> {
    let path = model.temp.path().join("graph.gddview");
    integration_pass(&render_view(&model.path, &path, &[])?)?;
    integration_pass(&verify_view(&model.path, Some(&path))?)?;
    let artifact = read_view(&read_view_bytes(&path)?)?;
    let derived = &artifact["boundaries"];
    let component = &derived["components"][PROVIDER];
    for field in ["responsibility", "rationale"] {
        let value = &component["boundary"][field];
        ensure!(
            value.as_str().is_some_and(|s| !s.is_empty())
                && value == &model.data["nodes"][PROVIDER]["boundary"][field]
        );
    }
    ensure!(
        component["public_contracts"]
            .as_array()
            .context("missing public contracts")?
            .iter()
            .any(|c| c["action"] == RESERVE && c["provider"] == PROVIDER)
    );
    ensure!(
        derived["calls"]
            .as_array()
            .context("missing boundary calls")?
            .iter()
            .any(|c| c["edge"] == "edge.screen.reserve.calls"
                && c["provider"] == PROVIDER
                && c["action"] == RESERVE
                && c["status"] == "public")
    );

    let executable = cli(model.directory)?;
    let view = run(&executable, "boundaries", &model.path, &[])?;
    validate_boundaries(model.directory, &view)?;
    ensure!(view == *derived, "CLI and artifact boundary data differ");

    let review_file = model.temp.path().join("review.json");
    write_json(&review_file, &review::complete(model)?)?;
    let result = run(
        &executable,
        "design-review",
        &model.path,
        &[
            "--review".as_ref(),
            review_file.as_os_str(),
            "--implementation".as_ref(),
            review::IMPLEMENTATION.as_ref(),
            "--code-revision".as_ref(),
            review::CODE_REVISION.as_ref(),
            "--scope".as_ref(),
            RESERVE.as_ref(),
        ],
    )?;
    ensure!(
        result["status"] == "COMPLETE",
        "unexpected design-review verdict: {result}"
    );
    Ok(())
}
