//! Run the public local-check command against a small dependency-free Cargo checkout.
use anyhow::Result;
use serde_json::Value;
use std::{fs, path::Path, process::Command};

const RUNNER: &str = env!("CARGO_BIN_EXE_gdd-conformance");

fn write(root: &Path, name: &str, contents: &str) -> Result<()> {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap())?;
    Ok(fs::write(path, contents)?)
}

fn quick_fixture(root: &Path) -> Result<()> {
    write(root, "conformance/cases.yaml", "cases: []")?;
    write(
        root,
        "Cargo.toml",
        "[workspace]\nresolver = '3'\nmembers = ['gdd', 'viewer']",
    )?;
    for (directory, package) in [
        ("gdd", "gdd"),
        ("viewer", "gdd-viewer"),
        ("skills/gdd/scripts", "gdd-skill-assets"),
    ] {
        let standalone = if package == "gdd-skill-assets" {
            "[workspace]"
        } else {
            ""
        };
        write(
            root,
            &format!("{directory}/Cargo.toml"),
            &format!(
                "[package]\nname = '{package}'\nedition = '2024'\nversion = '0.0.0'\n{standalone}"
            ),
        )?;
    }
    write(root, "gdd/src/lib.rs", "")?;
    let check_env = "#[test] fn check_cli_environment() { assert!(std::env::var_os(\"GDD_NATIVE_CLI\").is_none()); }";
    write(root, "gdd/tests/rust_readiness.rs", check_env)?;
    write(root, "gdd/tests/rust_view.rs", check_env)?;
    write(
        root,
        "viewer/src/lib.rs",
        "mod viewer { mod frame_tests { #[test] fn frame() {} } }",
    )?;
    write(
        root,
        "skills/gdd/scripts/src/main.rs",
        r#"
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(args == ["sync", "--check", "--root", "."] || args == ["verify", "skills/gdd"]);
    assert!(std::path::Path::new("conformance/cases.yaml").is_file());
}
"#,
    )?;
    for manifest in ["Cargo.toml", "skills/gdd/scripts/Cargo.toml"] {
        let result = Command::new("cargo")
            .current_dir(root)
            .args([
                "generate-lockfile",
                "--offline",
                "--manifest-path",
                manifest,
            ])
            .output()?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    Ok(())
}

fn driver(cwd: &Path, checkout: &Path, output: &str) -> Command {
    let mut command = Command::new(RUNNER);
    command
        .current_dir(cwd)
        // This fixture inspects libtest's captured panic in the stdout log.
        .env_remove("RUST_TEST_NOCAPTURE")
        .env("GDD_NATIVE_CLI", "unrelated-cli")
        .env("CARGO_TARGET_DIR", "cache with spaces")
        .arg("--checkout")
        .arg(checkout)
        .args(["local-check", "--quick", "--output", output]);
    command
}

fn summary(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(
        path.join("summary.json"),
    )?)?)
}

#[test]
fn quick_selects_the_checkout_and_retains_both_success_and_test_failure() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let cwd = dir.path().canonicalize()?;
    let checkout = cwd.join("selected checkout");
    quick_fixture(&checkout)?;
    let success = driver(&cwd, &checkout, "run one").output()?;
    assert!(
        success.status.success(),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    let out = cwd.join("run one");
    assert!(String::from_utf8_lossy(&success.stdout).contains(out.to_str().unwrap()));
    let report = summary(&out)?;
    assert_eq!(report["checkout"].as_str(), checkout.to_str());
    assert_eq!(report["mode"], "quick");
    assert_eq!(report["passed"], true);
    assert_eq!(
        report["cargo_target_dir"].as_str(),
        cwd.join("cache with spaces").to_str()
    );
    assert!(!checkout.join("skills/gdd/scripts/target").exists());
    assert!(
        report["toolchain"]["rustc"]
            .as_str()
            .unwrap()
            .contains("host:")
    );
    let steps = report["steps"].as_array().unwrap();
    assert_eq!(
        steps
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "cargo",
            "rustc",
            "readiness-and-view-cli",
            "viewer-frames",
            "skill-sync",
            "skill-verify"
        ]
    );
    for step in steps {
        assert_eq!(step["cwd"].as_str(), checkout.to_str());
        assert_eq!(step["exit_code"], 0);
        assert_eq!(step["passed"], true);
        for stream in ["stdout", "stderr"] {
            let path = Path::new(step[stream].as_str().unwrap());
            assert!(path.is_relative() && out.join(path).is_file());
        }
    }
    assert!(
        fs::read_to_string(out.join("viewer-frames.stdout.txt"))?
            .contains("viewer::frame_tests::frame ... ok")
    );
    write(
        &checkout,
        "gdd/tests/rust_view.rs",
        "#[test] fn cli_failure() { panic!(\"controlled failure\"); }",
    )?;
    let failure = driver(&cwd, &checkout, "run two").output()?;
    assert!(!failure.status.success());
    let out = cwd.join("run two");
    let report = summary(&out)?;
    assert_eq!(report["passed"], false);
    let steps = report["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[2]["name"], "readiness-and-view-cli");
    assert!(steps[2]["exit_code"].as_i64().unwrap() > 0);
    assert!(
        fs::read_to_string(out.join("readiness-and-view-cli.stdout.txt"))?
            .contains("controlled failure")
    );
    Ok(())
}

#[test]
fn missing_program_is_recorded_and_existing_results_are_protected() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    write(root, "conformance/cases.yaml", "")?;
    let failure = driver(root, root, "out").env("PATH", "").output()?;
    assert!(!failure.status.success());
    let out = root.join("out");
    let before = fs::read(out.join("summary.json"))?;
    let report = summary(&out)?;
    assert_eq!(report["passed"], false);
    assert_eq!(report["steps"].as_array().unwrap().len(), 1);
    assert!(report["steps"][0]["exit_code"].is_null());
    assert!(fs::read_to_string(out.join("cargo.stderr.txt"))?.contains("Could not run cargo"));
    let retry = driver(root, root, "out").output()?;
    assert!(!retry.status.success());
    assert_eq!(fs::read(out.join("summary.json"))?, before);
    Ok(())
}

#[cfg(unix)]
#[test]
fn failure_summary_preserves_non_utf8_checkout_and_output_names() -> Result<()> {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    let temp = tempfile::tempdir()?;
    let root = temp.path().join(OsStr::from_bytes(b"checkout-\xfa"));
    write(&root, "conformance/cases.yaml", "")?;
    let output = temp.path().join(OsStr::from_bytes(b"output-\xff"));
    let result = Command::new(RUNNER)
        .current_dir(temp.path())
        .env("PATH", "")
        .env("CARGO_TARGET_DIR", OsStr::from_bytes(b"target-\xfe"))
        .arg("--checkout")
        .arg(&root)
        .args(["local-check", "--quick", "--output"])
        .arg(&output)
        .output()?;
    assert_eq!(result.status.code(), Some(1));
    let report = summary(&output)?;
    assert_eq!(report["passed"], false);
    for (value, path) in [
        (&report["checkout"], root.canonicalize()?),
        (&report["steps"][0]["cwd"], root.canonicalize()?),
        (&report["output"], output.canonicalize()?),
        (
            &report["cargo_target_dir"],
            temp.path()
                .canonicalize()?
                .join(OsStr::from_bytes(b"target-\xfe")),
        ),
    ] {
        assert_eq!(
            value["os_encoded_bytes"],
            serde_json::to_value(path.as_os_str().as_bytes())?
        );
        assert!(value["display"].as_str().unwrap().contains('�'));
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_target_alias_into_the_portable_skill_is_rejected_before_building() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path();
    write(root, "conformance/cases.yaml", "")?;
    let skill = root.join("skills/gdd");
    fs::create_dir_all(&skill)?;
    std::os::unix::fs::symlink(&skill, root.join("alias"))?;
    let result = driver(root, root, "out")
        .env("CARGO_TARGET_DIR", "unused/../alias/scripts/target")
        .output()?;
    assert!(!result.status.success());
    let report = summary(&root.join("out"))?;
    assert!(report["steps"].as_array().unwrap().is_empty());
    assert!(
        report["error"]
            .as_str()
            .unwrap()
            .contains("outside the portable skill")
    );
    assert!(!skill.join("scripts").exists());
    Ok(())
}

#[test]
fn release_conflicts_with_quick_and_records_failure_in_selected_checkout() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let cwd = dir.path().canonicalize()?;
    let checkout = cwd.join("alternate checkout");
    write(&checkout, "conformance/cases.yaml", "")?;
    let conflict = driver(&cwd, &checkout, "unused")
        .arg("--release")
        .output()?;
    assert_eq!(conflict.status.code(), Some(2));
    assert!(!cwd.join("unused").exists());

    let failure = Command::new(RUNNER)
        .current_dir(&cwd)
        .env("PATH", "")
        .arg("--checkout")
        .arg(&checkout)
        .args(["local-check", "--release", "--output", "release output"])
        .output()?;
    assert!(!failure.status.success());
    let report = summary(&cwd.join("release output"))?;
    assert_eq!(report["mode"], "release");
    assert_eq!(report["checkout"].as_str(), checkout.to_str());
    assert_eq!(report["passed"], false);
    assert_eq!(report["steps"].as_array().unwrap().len(), 1);
    assert!(report["installed"].as_object().unwrap().is_empty());
    Ok(())
}
