use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::tempdir;

const BINARY: &str = env!("CARGO_BIN_EXE_gdd-skill-assets");
const INDEX: &str = "references/source-index.json";

fn write(path: &Path, data: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}

fn invoke(binary: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .env_remove("CARGO_MANIFEST_DIR")
        .output()
        .unwrap()
}

fn success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn failure(output: Output, messages: &[&str]) {
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    for message in messages {
        assert!(stderr.contains(message), "{stderr}");
    }
}

fn small_skill(skill: &Path) {
    write(&skill.join("SKILL.md"), "Standalone skill\n");
    write(&skill.join("references/gdd/guide.md"), "");
    write(
        &skill.join(INDEX),
        json!({
            "format": "gdd-skill-assets/1",
            "files": [{"path":"references/gdd/guide.md", "bytes":0,
                       "sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"}]
        })
        .to_string(),
    );
}

#[test]
fn relocated_binary_checks_its_recipient_from_an_unrelated_directory() {
    let temp = tempdir().unwrap();
    let skill = temp.path().join("received");
    small_skill(&skill);
    let scripts = skill.join("scripts");
    fs::create_dir(&scripts).unwrap();
    let binary = scripts.join(Path::new(BINARY).file_name().unwrap());
    if fs::hard_link(BINARY, &binary).is_err() {
        fs::copy(BINARY, &binary).unwrap();
    }

    success(invoke(&binary, temp.path(), &["verify"]));
    write(&skill.join("references/gdd/guide.md"), "corrupt");
    write(&skill.join("references/gdd/unindexed"), "extra");
    failure(
        invoke(&binary, temp.path(), &["verify"]),
        &[
            "Reference hash mismatch: references/gdd/guide.md",
            "Unindexed reference: references/gdd/unindexed",
        ],
    );
    fs::remove_file(skill.join("references/gdd/guide.md")).unwrap();
    failure(
        invoke(&binary, temp.path(), &["verify", skill.to_str().unwrap()]),
        &["Missing reference: references/gdd/guide.md"],
    );
    let unbound = temp.path().join(binary.file_name().unwrap());
    fs::rename(&binary, &unbound).unwrap();
    failure(
        invoke(&unbound, temp.path(), &["verify"]),
        &["verify SKILL_DIR"],
    );
}

#[test]
fn verify_rejects_escaping_ambiguous_and_duplicate_index_entries() {
    let temp = tempdir().unwrap();
    let skill = temp.path().join("skill");
    small_skill(&skill);
    let original: Value = serde_json::from_slice(&fs::read(skill.join(INDEX)).unwrap()).unwrap();
    for (path, message) in [
        ("../../elsewhere", "escapes its directory"),
        ("references/gdd/sub/../guide.md", "escapes its directory"),
        ("references/gdd//guide.md", "escapes its directory"),
        ("references/gdd/guide.md", "Duplicate generated reference"),
    ] {
        let mut index = original.clone();
        let mut copy = original["files"][0].clone();
        copy["path"] = json!(path);
        index["files"].as_array_mut().unwrap().push(copy);
        write(&skill.join(INDEX), index.to_string());
        failure(invoke(Path::new(BINARY), &skill, &["verify"]), &[message]);
    }
}

// Build a source fixture from the references shipped with this skill. The
// temporary checkout contains no source package or pre-existing asset index.
fn source_fixture(root: &Path) -> PathBuf {
    let scripts = std::env::current_dir().unwrap();
    let skill = scripts.parent().unwrap();
    let index: Value = serde_json::from_slice(&fs::read(skill.join(INDEX)).unwrap()).unwrap();
    for file in index["files"].as_array().unwrap() {
        let source = file["source"].as_str().unwrap();
        write(&root.join(source), source);
    }
    let target = root.join("skills/gdd");
    write(&target.join("SKILL.md"), "GDD skill");
    write(&target.join("references/manual.md"), "Hand-authored guide");
    target
}

#[test]
fn sync_checks_and_repairs_drift_and_preflights_all_required_sources() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    let skill = source_fixture(root);
    let cli = Path::new(BINARY);
    write(&root.join("docs/café guide.md"), "Optional reference");
    let guide =
        "Exact bytes\r\n[a](<docs/caf%C3%A9%20guide.md?q=x#proof>) [b](#local) [c](HOST.md)";
    write(&root.join("GDD.md"), guide);
    success(invoke(
        cli,
        root,
        &[
            "sync",
            "--repository",
            "test-url",
            "--git-base-revision",
            "base",
        ],
    ));
    let index_bytes = fs::read(skill.join(INDEX)).unwrap();
    let index: Value = serde_json::from_slice(&index_bytes).unwrap();
    assert_eq!(index["origin"]["git_base_revision"], "base");
    assert_eq!(index["origin"]["repository"], "test-url");
    assert_eq!(
        index["repository_only_links"],
        json!([{
            "from": "GDD.md", "href": "docs/caf%C3%A9%20guide.md?q=x#proof",
            "source_target": "docs/café guide.md", "role": "optional-repository-reference"
        }])
    );
    let copy = skill.join("references/gdd/GDD.md");
    assert_eq!(fs::read(&copy).unwrap(), guide.as_bytes());
    fs::remove_file(&copy).unwrap();
    let obsolete = skill.join("references/gdd/obsolete.md");
    write(&obsolete, "obsolete");
    failure(
        invoke(cli, root, &["sync", "--check"]),
        &[
            "Missing generated asset: references/gdd/GDD.md",
            "Obsolete generated reference: references/gdd/obsolete.md",
        ],
    );
    assert!(!copy.exists());
    assert!(obsolete.is_file());
    success(invoke(cli, root, &["sync"]));
    assert_eq!(fs::read(skill.join(INDEX)).unwrap(), index_bytes);
    assert!(!obsolete.exists());
    assert_eq!(
        fs::read(skill.join("references/manual.md")).unwrap(),
        b"Hand-authored guide"
    );
    success(invoke(cli, root, &["sync", "--check"]));
    success(invoke(cli, &skill, &["verify"]));

    write(&root.join("GDD.md"), "pending");
    fs::remove_file(root.join("HOST.md")).unwrap();
    failure(invoke(cli, root, &["sync"]), &["HOST.md"]);
    assert_eq!(fs::read(skill.join(INDEX)).unwrap(), index_bytes);
    assert_eq!(fs::read(copy).unwrap(), guide.as_bytes());
}

#[cfg(unix)]
#[test]
fn external_symlinks_cannot_supply_or_receive_reference_bytes() {
    use std::os::unix::fs::symlink;
    let temp = tempdir().unwrap();
    let root = temp.path().join("checkout");
    let outside = temp.path().join("external");
    write(&outside, "External file");
    let skill = source_fixture(&root);
    let cli = Path::new(BINARY);
    success(invoke(cli, &root, &["sync"]));
    let guide = skill.join("references/gdd/GDD.md");

    let source_link = root.join("schemas/new.json");
    symlink(&outside, &source_link).unwrap();
    failure(
        invoke(cli, &root, &["sync"]),
        &["escapes the source repository"],
    );
    fs::remove_file(source_link).unwrap();
    // A safe leaf link may point to an obsolete generated copy. Repair must
    // preserve the indexed bytes before removing that obsolete target.
    let extra = skill.join("references/gdd/obsolete");
    fs::rename(&guide, &extra).unwrap();
    symlink(&extra, &guide).unwrap();
    success(invoke(cli, &root, &["sync"]));
    assert!(!extra.exists());
    assert!(guide.is_file() && !guide.is_symlink());
    success(invoke(cli, &skill, &["verify"]));
    fs::remove_file(&guide).unwrap();
    symlink(&outside, &guide).unwrap();
    failure(invoke(cli, &root, &["sync"]), &["GDD.md"]);
    failure(invoke(cli, &skill, &["verify"]), &["GDD.md"]);
    assert_eq!(fs::read(outside).unwrap(), b"External file");
}
