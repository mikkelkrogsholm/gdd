//! Commands are independent processes with the selected checkout as their working directory.

use std::{path::Path, process::Command};

const SKILL_MANIFEST: &str = "skills/gdd/scripts/Cargo.toml";
const SUITES: [&str; 9] = [
    "standard",
    "graphify",
    "visualization",
    "workflow",
    "design",
    "code",
    "status",
    "impact",
    "diff",
];

fn cargo(action: &str, manifest: &str, args: &[&str]) -> Command {
    let mut command = Command::new("cargo");
    command.arg(action);
    if action != "fmt" {
        command.arg("--locked");
    }
    command.args(["--manifest-path", manifest]).args(args);
    command
}

fn workspace(action: &str, args: &[&str]) -> Command {
    cargo(action, "Cargo.toml", args)
}

fn skill(action: &str, args: &[&str]) -> Command {
    cargo(action, SKILL_MANIFEST, args)
}

pub(super) fn commands(quick: bool, root: &Path, output: &Path) -> Vec<(&'static str, Command)> {
    let clippy = ["--all-targets", "--all-features", "--", "-D", "warnings"];
    let mut commands = if quick {
        vec![
            (
                "readiness-and-view-cli",
                workspace(
                    "test",
                    &[
                        "-p",
                        "gdd",
                        "--test",
                        "rust_readiness",
                        "--test",
                        "rust_view",
                    ],
                ),
            ),
            (
                "viewer-frames",
                workspace("test", &["-p", "gdd-viewer", "viewer::frame_tests::"]),
            ),
        ]
    } else {
        let mut workspace_tests = workspace("test", &["--workspace", "--all-features"]);
        workspace_tests.env("GDD_CONCERT_REQUIREMENT_PHASE", "B");
        let mut workspace_clippy = workspace("clippy", &["--workspace"]);
        workspace_clippy.args(clippy);
        vec![
            ("workspace-fmt", workspace("fmt", &["--all", "--check"])),
            ("skill-fmt", skill("fmt", &["--all", "--check"])),
            (
                "workspace-build",
                workspace(
                    "build",
                    &[
                        "--workspace",
                        "--all-features",
                        "--bins",
                        "--message-format=json-render-diagnostics",
                    ],
                ),
            ),
            ("workspace-tests", workspace_tests),
            ("skill-tests", skill("test", &["--all-features"])),
            ("workspace-clippy", workspace_clippy),
            ("skill-clippy", skill("clippy", &clippy)),
        ]
    };
    commands.extend([
        (
            "skill-sync",
            skill("run", &["--", "sync", "--check", "--root", "."]),
        ),
        (
            "skill-verify",
            skill("run", &["--", "verify", "skills/gdd"]),
        ),
    ]);
    if !quick {
        for suite in SUITES {
            let mut command = workspace("run", &["-p", "gdd-conformance", "--", "--checkout"]);
            command
                .arg(root)
                .arg(suite)
                .arg("--output")
                .arg(output.join(format!("{suite}.json")));
            commands.push((suite, command));
        }
    }
    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, ffi::OsStr};

    #[test]
    fn full_runs_workspace_and_skill_gates_and_publishes_nine_distinct_receipts() {
        let root = Path::new("checkout path");
        let output = Path::new("output path");
        let commands: BTreeMap<_, _> = commands(false, root, output).into_iter().collect();
        let argv = |name: &str| commands[name].get_args().collect::<Vec<_>>();
        for (name, action) in [
            ("workspace-build", "build"),
            ("workspace-tests", "test"),
            ("workspace-clippy", "clippy"),
        ] {
            let args = argv(name);
            assert_eq!(args[0], action);
            assert!(args.contains(&OsStr::new("--workspace")));
            assert!(args.contains(&OsStr::new("--locked")));
        }
        for name in ["workspace-clippy", "skill-clippy"] {
            assert!(argv(name).ends_with(
                &["--all-targets", "--all-features", "--", "-D", "warnings"].map(OsStr::new)
            ));
        }
        for name in [
            "skill-fmt",
            "skill-tests",
            "skill-clippy",
            "skill-sync",
            "skill-verify",
        ] {
            assert!(argv(name).contains(&OsStr::new(SKILL_MANIFEST)));
        }
        for name in ["workspace-fmt", "skill-fmt"] {
            assert!(argv(name).contains(&OsStr::new("--check")));
        }
        for suite in [
            "standard",
            "graphify",
            "visualization",
            "workflow",
            "design",
            "code",
            "status",
            "impact",
            "diff",
        ] {
            let path = output.join(format!("{suite}.json"));
            assert!(argv(suite).ends_with(&[
                OsStr::new("--checkout"),
                root.as_os_str(),
                OsStr::new(suite),
                OsStr::new("--output"),
                path.as_os_str()
            ]));
        }
    }
}
