//! Install the three native packages and exercise the installed programs as a recipient.

use super::{Run, os_value};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn installations(root: &Path, prefix: &Path) -> [(&'static str, &'static str, Command); 3] {
    [
        ("install-gdd", "gdd", root.to_path_buf()),
        ("install-viewer", "gdd-viewer", root.join("crates/viewer")),
        (
            "install-conformance",
            "gdd-conformance",
            root.join("crates/conformance"),
        ),
    ]
    .map(|(step, name, package)| {
        let mut command = Command::new("cargo");
        command
            .args(["install", "--locked", "--path"])
            .arg(package)
            .arg("--root")
            .arg(prefix);
        (step, name, command)
    })
}

fn executable(prefix: &Path, name: &str) -> PathBuf {
    prefix
        .join("bin")
        .join(format!("{name}{}", env::consts::EXE_SUFFIX))
}

fn recipient_commands(
    root: &Path,
    prefix: &Path,
    reference: &Path,
    output: &Path,
) -> [(&'static str, Command); 3] {
    let runner = executable(prefix, "gdd-conformance");
    let mut verifier = Command::new(&runner);
    verifier
        .arg("--checkout")
        .arg(root)
        .args(["verify-install", "--installed"])
        .arg(executable(prefix, "gdd"))
        .arg("--reference")
        .arg(reference)
        .arg("--output")
        .arg(output.join("verify-install.json"));
    let mut help = Command::new(executable(prefix, "gdd-viewer"));
    help.arg("--help")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY");
    let mut demo = Command::new(runner);
    demo.args(["workflow-demo", "--output"])
        .arg(output.join("workflow-demo"));
    [
        ("verify-install", verifier),
        ("installed-viewer-help", help),
        ("installed-workflow-demo", demo),
    ]
}

fn receipt(path: &Path) -> Result<Value> {
    serde_json::from_reader(
        fs::File::open(path).with_context(|| format!("read {}", path.display()))?,
    )
    .with_context(|| format!("parse {}", path.display()))
}

pub(super) fn check(run: &mut Run<'_>) -> Result<()> {
    let prefix = run.output.join("install");
    for (step, name, command) in installations(run.root, &prefix) {
        run.step(step, command, run.root)?;
        let binary = executable(&prefix, name);
        let bytes =
            fs::read(&binary).with_context(|| format!("read installed {}", binary.display()))?;
        run.installed.insert(
            name.into(),
            json!({
                "path": os_value(binary.as_os_str()), "sha256": gdd::sha256(&bytes),
            }),
        );
    }

    let temp = env::temp_dir().canonicalize()?;
    let parent = if temp.starts_with(run.root) {
        run.root
            .parent()
            .context("locate a recipient outside the checkout")?
    } else {
        &temp
    };
    let recipient = tempfile::Builder::new()
        .prefix("gdd-release-recipient-")
        .tempdir_in(parent)?;
    let reference = run
        .cli
        .as_deref()
        .context("missing workspace gdd artifact")?;
    for (step, mut command) in recipient_commands(run.root, &prefix, reference, run.output) {
        // The verifier and demo also create private working directories for their children.
        for key in ["TMPDIR", "TMP", "TEMP"] {
            command.env(key, recipient.path());
        }
        run.step(step, command, recipient.path())?;
        if step == "verify-install" {
            let verified = receipt(&run.output.join("verify-install.json"))?;
            ensure!(
                verified["passed"] == true
                    && verified["controls"].as_array().map(Vec::len)
                        == Some(crate::verify_install::TOTAL)
                    && verified["installed_sha256"] == run.installed["gdd"]["sha256"],
                "installed verifier receipt does not confirm all controls for the installed gdd"
            );
        }
    }

    let summary = receipt(&run.output.join("workflow-demo/summary.json"))?;
    ensure!(
        summary["acceptance_current"] == true
            && summary["baseline_cargo_exit"] == 101
            && summary["final_cargo_exit"] == 0,
        "installed quote workflow did not record an accepted red/green Cargo run"
    );
    super::source_package::check(run, recipient.path())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn failed_viewer_install_retains_cli_identity_and_stops_before_recipient_checks() -> Result<()>
    {
        let dir = tempfile::tempdir()?;
        let root = dir.path().join("selected checkout");
        let output = dir.path().join("fresh output");
        fs::create_dir_all(root.join("src"))?;
        fs::create_dir(&output)?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='gdd'\nversion='0.0.0'\nedition='2024'\n[workspace]\n",
        )?;
        fs::write(
            root.join("Cargo.lock"),
            "version = 4\n[[package]]\nname = 'gdd'\nversion = '0.0.0'\n",
        )?;
        fs::write(root.join("src/main.rs"), "fn main() {}")?;
        let mut run = Run {
            root: &root,
            output: &output,
            target: dir.path().join("custom cache"),
            cli: None,
            installed: Default::default(),
            source_package: None,
            tutorial_source_package: None,
            concert_source_package: None,
            steps: Vec::new(),
            toolchain: Default::default(),
        };
        let error = check(&mut run).unwrap_err();
        assert!(
            error.to_string().contains("install-viewer failed"),
            "{error}"
        );
        assert_eq!(run.steps.len(), 2);
        assert_eq!(run.steps[0]["passed"], true);
        assert_eq!(run.steps[1]["passed"], false);
        assert_eq!(
            run.installed["gdd"]["sha256"],
            gdd::sha256(&fs::read(executable(&output.join("install"), "gdd"))?)
        );
        assert!(run.target.is_dir());
        assert!(!root.join("target").exists());
        assert!(!output.join("verify-install.json").exists());
        Ok(())
    }

    #[test]
    fn release_installs_selected_packages_then_uses_installed_binaries_and_captured_reference() {
        let root = Path::new("selected checkout");
        let output = Path::new("new evidence");
        let prefix = output.join("install");
        let reference = Path::new("custom build/target triple/debug/gdd");
        for ((_, name, command), (expected, package)) in
            installations(root, &prefix).into_iter().zip([
                ("gdd", root.to_owned()),
                ("gdd-viewer", root.join("crates/viewer")),
                ("gdd-conformance", root.join("crates/conformance")),
            ])
        {
            assert_eq!(name, expected);
            assert_eq!(command.get_program(), "cargo");
            assert_eq!(
                command.get_args().collect::<Vec<_>>(),
                [
                    OsStr::new("install"),
                    OsStr::new("--locked"),
                    OsStr::new("--path"),
                    package.as_os_str(),
                    OsStr::new("--root"),
                    prefix.as_os_str()
                ]
            );
        }
        let [(verify_name, verify), (help_name, help), (demo_name, demo)] =
            recipient_commands(root, &prefix, reference, output);
        assert_eq!(
            [verify_name, help_name, demo_name],
            [
                "verify-install",
                "installed-viewer-help",
                "installed-workflow-demo"
            ]
        );
        assert_eq!(verify.get_program(), executable(&prefix, "gdd-conformance"));
        assert_eq!(
            verify.get_args().collect::<Vec<_>>(),
            [
                OsStr::new("--checkout"),
                root.as_os_str(),
                OsStr::new("verify-install"),
                OsStr::new("--installed"),
                executable(&prefix, "gdd").as_os_str(),
                OsStr::new("--reference"),
                reference.as_os_str(),
                OsStr::new("--output"),
                output.join("verify-install.json").as_os_str(),
            ]
        );
        assert_eq!(help.get_program(), executable(&prefix, "gdd-viewer"));
        assert_eq!(help.get_args().collect::<Vec<_>>(), ["--help"]);
        assert_eq!(demo.get_program(), verify.get_program());
        assert_eq!(
            demo.get_args().collect::<Vec<_>>(),
            [
                OsStr::new("workflow-demo"),
                OsStr::new("--output"),
                output.join("workflow-demo").as_os_str(),
            ]
        );
    }
}
