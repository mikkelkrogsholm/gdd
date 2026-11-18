#![cfg(any(target_os = "linux", target_os = "macos", windows))]

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Barrier};

use gdd::{DEFAULT_PROJECT_ID, DEFAULT_TITLE, init_project};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = dir
        .read_dir()
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn default_cli_draft_and_receipt_match_the_python_contract() {
    let root = tempfile::tempdir().unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_gdd"))
        .current_dir(root.path())
        .args(["init", "draft"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let receipt: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(receipt["format"], "gdd-init/0.1");
    assert_eq!(receipt["project_id"], "project.new");
    assert_eq!(
        receipt["files"],
        serde_json::json!(["SOURCES.md", "gdd.json", "records.json"])
    );
    assert_eq!(receipt["model"], "PASS");
    assert_eq!(receipt["readiness"], "FAIL");
    assert_eq!(
        receipt["spec_revision"],
        "sha256:054f290a76fbd050a10fa4223e5539254218cecc5876d4d27632be825ce3efea"
    );
    let output = root.path().join("draft").canonicalize().unwrap();
    assert_eq!(
        Path::new(receipt["path"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        output
    );
    assert_eq!(
        Path::new(receipt["manifest"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        output.join("gdd.json")
    );

    assert_eq!(files_in(root.path()), ["draft"]);
    assert_eq!(
        files_in(&output),
        ["SOURCES.md", "gdd.json", "records.json"]
    );
    for (name, expected) in [
        (
            "SOURCES.md",
            include_bytes!("../templates/project/SOURCES.md").as_slice(),
        ),
        (
            "records.json",
            include_bytes!("../templates/project/records.json").as_slice(),
        ),
    ] {
        assert_eq!(fs::read(output.join(name)).unwrap(), expected);
    }
    // The Python initializer re-encodes the model with multiline JSON arrays.
    let model = fs::read(output.join("gdd.json")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(model)),
        "d97bea80393b8e2050e038ab572f42a8bff08c094c1a450b6c128b38e7c85f62"
    );
}

#[test]
fn custom_cli_draft_preserves_unicode_and_escapes() {
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("draft");
    let title = "Dansk ø 🦀 \"quoted\"\\\nnext\tline\u{2028}\u{b}";
    let project_id = "alpha_9.2-beta";
    let result = Command::new(env!("CARGO_BIN_EXE_gdd"))
        .args(["init", "--title", title, "--project-id", project_id])
        .arg(&output)
        .output()
        .unwrap();
    assert!(result.status.success());
    let receipt: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(receipt["project_id"], project_id);
    assert_eq!(
        receipt["spec_revision"],
        "sha256:4455e967f4a568cf5a52dba71de30af214e0c29a88901139d90f36c099b4813f"
    );

    // File hashes computed from the Python starter with these same arguments.
    for (name, expected) in [
        (
            "SOURCES.md",
            "b6a5f24cc82513ed9481872fab06c2ba8b427aa91ae492f571d60bb2941d3b3b",
        ),
        (
            "gdd.json",
            "7735c8c78ae9955787ecd4ca3f916f55b25a965ebf2963cfc61a80e2aabcc1b5",
        ),
        (
            "records.json",
            "dd81a4ea9da5d8609b56f749f36443ae545e581134a1ec30d66d7dc4725d3563",
        ),
    ] {
        let bytes = fs::read(output.join(name)).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected);
    }
    let model: Value = serde_json::from_slice(&fs::read(output.join("gdd.json")).unwrap()).unwrap();
    assert_eq!(model["project"]["title"], title);
}

#[test]
fn invalid_arguments_and_missing_parent_leave_no_files() {
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("draft");
    for id in [
        "",
        "Project.invalid",
        "project.invalid\n",
        "invalid space",
        "project.",
        "p..q",
        "é",
    ] {
        let error = init_project(&output, id, DEFAULT_TITLE).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidInput, "{id:?}");
    }
    for title in ["", " \n\t", "\u{1c}\u{a0}\u{2003}"] {
        let error = init_project(&output, DEFAULT_PROJECT_ID, title).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidInput, "{title:?}");
    }
    let nul = root.path().join("nul\0path");
    let error = init_project(&nul, DEFAULT_PROJECT_ID, DEFAULT_TITLE).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    let error = init_project(
        &root.path().join("missing").join("draft"),
        DEFAULT_PROJECT_ID,
        DEFAULT_TITLE,
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert!(files_in(root.path()).is_empty());
}

#[test]
fn existing_file_and_directory_are_preserved() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("file");
    fs::write(&file, "keep file").unwrap();
    let dir = root.path().join("dir");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("keep"), "keep directory").unwrap();

    for path in [&file, &dir] {
        let error = init_project(path, DEFAULT_PROJECT_ID, DEFAULT_TITLE).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
    }
    assert_eq!(fs::read(&file).unwrap(), b"keep file");
    assert_eq!(fs::read(dir.join("keep")).unwrap(), b"keep directory");
    assert_eq!(files_in(root.path()), ["dir", "file"]);
    assert_eq!(files_in(&dir), ["keep"]);
}

#[cfg(unix)]
#[test]
fn parent_symlinks_are_resolved_and_destination_symlinks_are_preserved() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let actual = root.path().join("actual");
    fs::create_dir(&actual).unwrap();
    let alias = root.path().join("alias");
    symlink(&actual, &alias).unwrap();
    let missing = root.path().join("missing");
    let dangling = root.path().join("dangling");
    symlink(&missing, &dangling).unwrap();

    for path in [&alias, &dangling] {
        let error = init_project(path, DEFAULT_PROJECT_ID, DEFAULT_TITLE).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
    }
    assert_eq!(fs::read_link(&alias).unwrap(), actual);
    assert_eq!(fs::read_link(&dangling).unwrap(), missing);
    assert!(!missing.exists());

    let receipt = init_project(&alias.join("draft"), DEFAULT_PROJECT_ID, DEFAULT_TITLE).unwrap();
    assert_eq!(
        Path::new(&receipt.path),
        actual.canonicalize().unwrap().join("draft")
    );
    assert_eq!(files_in(root.path()), ["actual", "alias", "dangling"]);
}

#[test]
fn competing_initializations_publish_one_complete_bundle() {
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("draft");
    let barrier = Arc::new(Barrier::new(8));
    let attempts: Vec<_> = (0..8)
        .map(|index| {
            let barrier = barrier.clone();
            let output = output.clone();
            std::thread::spawn(move || {
                barrier.wait();
                init_project(&output, &format!("project.{index}"), DEFAULT_TITLE)
            })
        })
        .collect();

    let mut winners = Vec::new();
    for attempt in attempts {
        match attempt.join().unwrap() {
            Ok(receipt) => winners.push(receipt),
            Err(error) => assert_eq!(error.kind(), ErrorKind::AlreadyExists),
        }
    }
    assert_eq!(winners.len(), 1);
    assert_eq!(files_in(root.path()), ["draft"]);
    assert_eq!(
        files_in(&output),
        ["SOURCES.md", "gdd.json", "records.json"]
    );
    let model: Value = serde_json::from_slice(&fs::read(output.join("gdd.json")).unwrap()).unwrap();
    let records: Value =
        serde_json::from_slice(&fs::read(output.join("records.json")).unwrap()).unwrap();
    assert_eq!(model["project"]["id"], winners[0].project_id);
    assert_eq!(records["project_id"], winners[0].project_id);
}
