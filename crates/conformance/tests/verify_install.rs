//! Exercise failure receipts and publication through the public verifier command.

use anyhow::Result;
use serde_json::Value;
use std::{fs, path::Path, process::Command};

fn driver(installed: &Path, reference: &Path, output: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gdd-conformance"));
    command
        .arg("--checkout")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .arg("verify-install")
        .arg("--installed")
        .arg(installed)
        .arg("--reference")
        .arg(reference)
        .arg("--output")
        .arg(output);
    command
}

#[test]
fn missing_executables_retain_available_identity_and_never_clobber_a_receipt() -> Result<()> {
    let folder = tempfile::tempdir()?;
    let valid = folder.path().join("available binary");
    let missing = folder.path().join("absent binary");
    fs::write(&valid, b"test bytes")?;
    let hash = gdd::sha256(b"test bytes");
    for (role, installed, reference) in [
        ("installed", &missing, &valid),
        ("reference", &valid, &missing),
    ] {
        let output = folder.path().join(format!("{role} receipt.json"));
        let result = driver(installed, reference, &output).output()?;
        assert_eq!(result.status.code(), Some(1));
        let bytes = fs::read(&output)?;
        let report: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(report["passed"], false);
        assert!(report["controls"].as_array().unwrap().is_empty());
        assert!(
            report["error"]
                .as_str()
                .unwrap()
                .contains(&format!("locate {role} executable"))
        );
        let available_role = if role == "installed" {
            "reference"
        } else {
            "installed"
        };
        assert!(report[format!("{role}_sha256")].is_null());
        assert_eq!(report[format!("{available_role}_sha256")], hash);
        let retry = driver(&valid, &valid, &output).output()?;
        assert!(!retry.status.success());
        assert!(String::from_utf8_lossy(&retry.stderr).contains("receipt already exists"));
        assert_eq!(fs::read(&output)?, bytes);
    }
    Ok(())
}

#[cfg(unix)]
fn program(path: &Path, body: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, format!("#!/bin/sh\n{body}\n"))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn exit_parse_and_parity_failures_stop_at_the_first_control() -> Result<()> {
    let folder = tempfile::tempdir()?;
    let candidate = folder.path().join("installed binary");
    let reference = folder.path().join("reference binary");
    let shared = "x".repeat(1600);
    program(&reference, &format!("echo '{{\"shared\":\"{shared}\"}}'"))?;
    for (label, body, exit, diagnostic, reference_ran) in [
        (
            "exit",
            "echo 'controlled failure' >&2; exit 7".to_string(),
            7,
            "controlled failure",
            false,
        ),
        (
            "parse",
            "echo 'invalid-json'".to_string(),
            0,
            "invalid installed JSON",
            false,
        ),
        (
            "parity",
            format!("echo '{{\"shared\":\"{shared}\",\"unexpected_probe_field\":true}}'"),
            0,
            "/unexpected_probe_field",
            true,
        ),
    ] {
        program(&candidate, &body)?;
        let output = folder.path().join(format!("{label}.json"));
        assert_eq!(
            driver(&candidate, &reference, &output)
                .output()?
                .status
                .code(),
            Some(1)
        );
        let report: Value = serde_json::from_slice(&fs::read(output)?)?;
        assert_eq!(report["passed"], false);
        let controls = report["controls"].as_array().unwrap();
        assert_eq!(controls.len(), 1);
        let control = &controls[0];
        assert_eq!(control["name"], "legacy-model");
        assert_eq!(control["passed"], false);
        assert_eq!(control["expected_exit"], 0);
        assert_eq!(control["installed"]["exit_code"], exit);
        assert_eq!(control["reference"].is_object(), reference_ran);
        assert!(control["error"].as_str().unwrap().contains(diagnostic));
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn equal_generation_reports_still_require_matching_generated_and_distributed_views() -> Result<()> {
    let folder = tempfile::tempdir()?;
    let candidate = folder.path().join("installed binary");
    let reference = folder.path().join("reference binary");
    // Both files are valid native views. Only the 0.2 view matches the legacy model.
    let legacy = "examples/concert.gddview";
    let current = "examples/concert-0.3.gddview";
    for (label, reference_artifact, diagnostic) in [
        ("disagreement", legacy, "native view bytes differ"),
        (
            "stale",
            current,
            "stale distributed native view examples/concert.gddview",
        ),
    ] {
        for (path, artifact) in [(&candidate, current), (&reference, reference_artifact)] {
            program(
                path,
                &format!(
                    r#"test "$HOME" = "$PWD" && test -f examples/concert.gdd.yaml || exit 9
if test "$1" = view; then command -p cp '{artifact}' "$4" || exit 9; fi
echo '{{"format":"gdd-report/0.2"}}'"#
                ),
            )?;
        }
        let output = folder.path().join(format!("{label} receipt.json"));
        assert_eq!(
            driver(&candidate, &reference, &output)
                .output()?
                .status
                .code(),
            Some(1)
        );
        let report: Value = serde_json::from_slice(&fs::read(output)?)?;
        let controls = report["controls"].as_array().unwrap();
        assert_eq!(controls.len(), 5);
        assert!(controls[..4].iter().all(|item| item["passed"] == true));
        assert_eq!(controls[4]["name"], "legacy-view");
        assert_eq!(controls[4]["installed"]["exit_code"], 0);
        assert_eq!(controls[4]["reference"]["exit_code"], 0);
        assert!(controls[4]["error"].as_str().unwrap().contains(diagnostic));
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn executable_symlinks_resolve_to_regular_files_and_devices_are_rejected() -> Result<()> {
    use std::os::unix::fs::symlink;
    let folder = tempfile::tempdir()?;
    let target = folder.path().join("executable");
    program(&target, "exit 7")?;
    let alias = folder.path().join("installed alias");
    symlink(&target, &alias)?;
    let output = folder.path().join("receipt.json");
    let run = driver(&alias, Path::new("/dev/null"), &output).output()?;
    assert_eq!(run.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&fs::read(output)?)?;
    assert_eq!(report["installed_sha256"], gdd::sha256(&fs::read(target)?));
    assert!(report["reference_sha256"].is_null());
    assert!(
        report["error"]
            .as_str()
            .unwrap()
            .contains("reference executable must be a regular file")
    );
    assert!(report["controls"].as_array().unwrap().is_empty());
    Ok(())
}
