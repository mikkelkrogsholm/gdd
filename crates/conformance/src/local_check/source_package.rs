//! Retain the root Cargo package and test its extracted sources.

use super::{Run, os_value};
use anyhow::{Context, Result, ensure};
use flate2::read::GzDecoder;
use serde_json::json;
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
        "expected one .crate in {} after packaging gdd, found {}",
        directory.display(),
        archives.len()
    );
    Ok(archives.pop().unwrap())
}

pub(super) fn check(run: &mut Run<'_>, recipient: &Path) -> Result<()> {
    let staging = tempfile::tempdir_in(recipient)?;
    let mut package = Command::new("cargo");
    package
        .args([
            "package",
            "--locked",
            "--no-verify",
            "--allow-dirty",
            "-p",
            "gdd",
            "--target-dir",
        ])
        .arg(staging.path());
    run.step("source-package", package, run.root)?;

    let archive = package_file(&staging.path().join("package"))?;
    let retained = run
        .output
        .join(archive.file_name().context("missing Cargo archive name")?);
    io::copy(&mut File::open(archive)?, &mut File::create_new(&retained)?)?;
    let bytes = fs::read(&retained)?;
    run.source_package = Some(json!({
        "path": os_value(retained.as_os_str()), "sha256": gdd::sha256(&bytes)
    }));

    let extraction = tempfile::tempdir_in(recipient)?;
    tar::Archive::new(GzDecoder::new(bytes.as_slice())).unpack(extraction.path())?;
    let root = extraction.path().join(
        retained
            .file_stem()
            .context("missing Cargo package directory name")?,
    );
    // Cargo archives have normalized timestamps. A shared target can reuse tests
    // from another extraction of the same package name and version.
    let target = tempfile::tempdir_in(recipient)?;
    let mut test = Command::new("cargo");
    test.args([
        "test",
        "--locked",
        "--manifest-path",
        "Cargo.toml",
        "--target-dir",
    ])
    .arg(target.path())
    .env("CARGO_BUILD_BUILD_DIR", target.path());
    run.step("source-package-tests", test, &root)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
