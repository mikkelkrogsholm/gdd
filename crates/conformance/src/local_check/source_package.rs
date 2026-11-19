//! Retain and test the extracted native source packages.

use super::{Run, os_value};
use anyhow::{Context, Result, ensure};
use flate2::read::GzDecoder;
use serde_json::{Value, json};
use std::{
    ffi::OsStr,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process::Command,
};

fn package_file(directory: &Path) -> Result<PathBuf> {
    let mut archives = Vec::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension() == Some(OsStr::new("crate")) {
            archives.push(path);
        }
    }
    ensure!(
        archives.len() == 1,
        "expected one .crate in {} after packaging, found {}",
        directory.display(),
        archives.len()
    );
    Ok(archives.pop().unwrap())
}

const TUTORIAL_TEST: &str = "tests::attest_actual_checks_and_observe_drift";

fn package(
    run: &mut Run<'_>,
    recipient: &Path,
    step: &str,
    manifest: &str,
    name: &str,
) -> Result<(PathBuf, Value)> {
    let staging = tempfile::tempdir_in(recipient)?;
    let mut cargo = Command::new("cargo");
    cargo
        .args([
            "package",
            "--locked",
            "--no-verify",
            "--allow-dirty",
            "--manifest-path",
            manifest,
            "-p",
            name,
            "--target-dir",
        ])
        .arg(staging.path());
    run.step(step, cargo, run.root)?;

    let archive = package_file(&staging.path().join("package"))?;
    let retained = run
        .output
        .join(archive.file_name().context("missing Cargo archive name")?);
    io::copy(&mut File::open(archive)?, &mut File::create_new(&retained)?)?;
    let bytes = fs::read(&retained)?;
    let identity = json!({
        "path": os_value(retained.as_os_str()), "sha256": gdd::sha256(&bytes)
    });
    tar::Archive::new(GzDecoder::new(bytes.as_slice())).unpack(recipient)?;
    let root = recipient.join(
        retained
            .file_stem()
            .context("missing Cargo package directory name")?,
    );
    Ok((root, identity))
}

fn package_tests(target: &Path) -> Command {
    let mut test = Command::new("cargo");
    test.args([
        "test",
        "--locked",
        "--manifest-path",
        "Cargo.toml",
        "--target-dir",
    ])
    .arg(target)
    // Nested Cargo commands in the tutorial must also use fresh artifacts.
    .env("CARGO_TARGET_DIR", target)
    .env("CARGO_BUILD_BUILD_DIR", target);
    test
}

fn confirm_tutorial(stdout: &str) -> Result<()> {
    // The exact filter selects only TUTORIAL_TEST. Child output can interrupt
    // libtest's per-test status line when the harness runs single-threaded.
    const PASSED: &str = "test result: ok. 1 passed; 0 failed; 0 ignored;";
    ensure!(
        stdout.lines().any(|line| line.starts_with(PASSED)),
        "the packaged tutorial test {TUTORIAL_TEST} did not report a pass; see tutorial-source-package-tests.stdout.txt"
    );
    Ok(())
}

pub(super) fn check(run: &mut Run<'_>, recipient: &Path) -> Result<()> {
    let extraction = tempfile::tempdir_in(recipient)?;
    let (root, identity) = package(
        run,
        extraction.path(),
        "source-package",
        "Cargo.toml",
        "gdd",
    )?;
    run.source_package = Some(identity);
    // Cargo archives have normalized timestamps. A shared target can reuse tests
    // from another extraction of the same package name and version.
    let target = tempfile::tempdir_in(recipient)?;
    run.step("source-package-tests", package_tests(target.path()), &root)?;

    let (checker, identity) = package(
        run,
        extraction.path(),
        "tutorial-source-package",
        "docs/tutorial/rust/Cargo.toml",
        "gdd-tutorial-add",
    )?;
    run.tutorial_source_package = Some(identity);
    let mut tutorial = package_tests(target.path());
    tutorial
        .args([
            "--example",
            "tutorial_evidence",
            "--",
            "--ignored",
            "--exact",
            TUTORIAL_TEST,
            "--format",
            "pretty",
            "--color",
            "never",
        ])
        .env("GDD_TUTORIAL_SOURCE", checker);
    run.step_checked("tutorial-source-package-tests", tutorial, &root, 0, |log| {
        confirm_tutorial(&fs::read_to_string(log)?)
    })?;
    let (concert, identity) = package(
        run,
        extraction.path(),
        "concert-source-package",
        "examples/concert-app/Cargo.toml",
        "gdd-concert",
    )?;
    run.concert_source_package = Some(identity);
    for (step, phase, exit_code) in [
        ("concert-requirements-a", "A", 101),
        ("concert-requirements-b", "B", 0),
    ] {
        let mut test = package_tests(target.path());
        test.args([
            "--test",
            "requirement_change",
            "--",
            "--format",
            "pretty",
            "--color",
            "never",
        ])
        .env("GDD_CONCERT_REQUIREMENT_PHASE", phase)
        .env_remove("RUST_TEST_NOCAPTURE")
        // Avoid generating debug symbols for the embedded database on this fresh build.
        .env("CARGO_PROFILE_TEST_DEBUG", "0");
        run.step_checked(step, test, &concert, exit_code, |log| {
            super::concert::confirm(&fs::read_to_string(log)?, phase == "A")
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_tests_or_an_ignored_tutorial_cannot_pass_release() {
        assert!(confirm_tutorial("running 0 tests\ntest result: ok. 0 passed").is_err());
        assert!(confirm_tutorial("test result: ok. 0 passed; 0 failed; 1 ignored;").is_err());
        assert!(
            confirm_tutorial("test result: ok. 1 passed; 0 failed; 0 ignored; 1 filtered out;")
                .is_ok()
        );
    }

    #[test]
    fn omitted_fixture_fails_tests_of_the_retained_package() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("selected checkout");
        let output = temp.path().join("release evidence");
        let target = temp.path().join("build cache");
        fs::create_dir_all(root.join("src"))?;
        fs::create_dir(&output)?;
        fs::create_dir_all(target.join("package"))?;
        fs::write(target.join("package/gdd-0.4.2.crate"), b"stale package")?;
        fs::write(root.join("fixture.txt"), "checkout fixture")?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='gdd'\nversion='0.4.2'\nedition='2024'\ninclude=['src/lib.rs']\n",
        )?;
        fs::write(
            root.join("Cargo.lock"),
            "version = 4\n[[package]]\nname='gdd'\nversion='0.4.2'\n",
        )?;
        fs::write(
            root.join("src/lib.rs"),
            "#[test] fn check_fixture() { assert!(std::path::Path::new(\"fixture.txt\").is_file()); }",
        )?;
        let old = temp.path().join("old recipient/gdd-0.4.2");
        fs::create_dir_all(old.join("src"))?;
        fs::copy(root.join("Cargo.toml"), old.join("Cargo.toml"))?;
        fs::copy(root.join("Cargo.lock"), old.join("Cargo.lock"))?;
        fs::write(old.join("src/lib.rs"), "#[test] fn check_fixture() {}")?;
        let warm = Command::new("cargo")
            .args(["test", "--locked", "--target-dir"])
            .arg(&target)
            .env("CARGO_BUILD_BUILD_DIR", &target)
            .current_dir(&old)
            .output()?;
        assert!(
            warm.status.success(),
            "{}",
            String::from_utf8_lossy(&warm.stderr)
        );
        assert!(String::from_utf8_lossy(&warm.stdout).contains("check_fixture ... ok"));
        let mut run = Run {
            root: &root,
            output: &output,
            target,
            cli: None,
            installed: Default::default(),
            source_package: None,
            tutorial_source_package: None,
            concert_source_package: None,
            steps: Vec::new(),
            toolchain: Default::default(),
        };
        let error = check(&mut run, temp.path()).unwrap_err();
        assert!(
            error.to_string().contains("source-package-tests failed"),
            "{error:#}"
        );
        let retained = output.join("gdd-0.4.2.crate");
        let identity = run.source_package.unwrap();
        assert_eq!(identity["path"], os_value(retained.as_os_str()));
        assert_eq!(identity["sha256"], gdd::sha256(&fs::read(retained)?));
        assert_eq!(run.steps.len(), 2);
        assert_eq!(run.steps[0]["passed"], true);
        assert_eq!(run.steps[1]["passed"], false);
        let test_args = run.steps[1]["command"].as_array().unwrap();
        assert_eq!(test_args[test_args.len() - 2], "--target-dir");
        let private_target = Path::new(test_args.last().unwrap().as_str().unwrap());
        assert_eq!(private_target.parent(), Some(temp.path()));
        assert_ne!(private_target, run.target);
        assert!(!Path::new(run.steps[1]["cwd"].as_str().unwrap()).starts_with(root));
        assert!(
            fs::read_to_string(output.join("source-package-tests.stdout.txt"))?
                .contains("check_fixture ... FAILED")
        );
        Ok(())
    }
}
