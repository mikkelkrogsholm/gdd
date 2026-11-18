use super::*;
use serde_json::Value;
use std::io::Read;
use tempfile::tempdir;

fn init(path: &Path) {
    git(path, &["init", "-q", "--object-format=sha1"]).unwrap();
    git(path, &["config", "core.autocrlf", "false"]).unwrap();
    git(path, &["config", "core.fileMode", "false"]).unwrap();
}

fn commit(path: &Path) {
    git(
        path,
        &[
            "-c",
            "user.name=Archive Test",
            "-c",
            "user.email=archive@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "archive fixture",
        ],
    )
    .unwrap();
}

#[test]
fn builds_from_committed_data_with_normalized_metadata_and_ascii_index() {
    let directory = tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    init(&root);
    let source = root.join("src");
    fs::create_dir(&source).unwrap();
    let separator = if cfg!(unix) { "\n" } else { "-" };
    let name = format!("Aé🎵{separator}{}.bin", "long".repeat(25));
    let data = [0, 0xff, 0x80, 10];
    fs::write(source.join(&name), data).unwrap();
    fs::write(source.join("run.sh"), b"#!/bin/sh\n").unwrap();
    fs::write(root.join("excluded"), b"outside the subtree").unwrap();
    git(&root, &["add", "."]).unwrap();
    git(&root, &["update-index", "--chmod=+x", "src/run.sh"]).unwrap();
    commit(&root);
    fs::write(source.join(&name), b"dirty replacement").unwrap();
    fs::remove_file(source.join("run.sh")).unwrap();
    fs::write(source.join("untracked"), b"untracked").unwrap();

    let archive = root.join("out/archive/café.tar.gz");
    let index = root.join("out/index/files.json");
    let receipt = build(&root, "HEAD", "src", &archive, &index).unwrap();
    assert_eq!(receipt.files, 2);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::metadata(root.join("excluded"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        for output in [&archive, &index] {
            assert_eq!(
                fs::metadata(output).unwrap().permissions().mode() & 0o777,
                permissions
            );
        }
    }
    let json = fs::read_to_string(&index).unwrap();
    assert!(json.is_ascii() && json.ends_with('\n'));
    assert!(json.contains(r"A\u00e9\ud83c\udfb5") && json.contains(r"caf\u00e9.tar.gz"));
    let doc: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(doc["source"]["git_path"], "src");
    assert_eq!(
        doc["source"]["git_tree"],
        revision_id(&root, "HEAD^{tree}").unwrap()
    );
    assert_eq!(doc["files"][0]["original_path"], format!("src/{name}"));
    assert_eq!(doc["files"][0]["git_mode"], "100644");
    assert_eq!(doc["files"][1]["git_mode"], "100755");

    let bytes = fs::read(&archive).unwrap();
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(bytes.as_slice())
        .read_to_end(&mut raw)
        .unwrap();
    assert_eq!(&bytes[..10], &[31, 139, 8, 0, 0, 0, 0, 0, 2, 255]);
    assert_eq!(raw.len() % 10240, 0);
    assert_eq!(&raw[..14], b"././@PaxHeader");
    let record = format!("{} path=src/{name}\n", name.len() + 14);
    assert_eq!(&raw[512..512 + record.len()], record.as_bytes());
    assert_eq!(&raw[1024..1031], b"src/A??");
    assert_eq!(&raw[1178..1180], b"\0 ");

    let restored = root.join("restored");
    super::super::restore(&archive, &index, &restored).unwrap();
    assert_eq!(fs::read(restored.join("src").join(name)).unwrap(), data);
    assert_eq!(
        fs::read(restored.join("src/run.sh")).unwrap(),
        b"#!/bin/sh\n"
    );
    assert!(!restored.join("src/untracked").exists() && !restored.join("excluded").exists());

    let second = directory.path().join("new-parent/../second/café.tar.gz");
    let second_index = root.join("second.json");
    build(&root, "HEAD", "src", &second, &second_index).unwrap();
    assert_eq!(bytes, fs::read(second).unwrap());
    assert_eq!(json.as_bytes(), fs::read(second_index).unwrap());
}

#[test]
fn refuses_incomplete_substituted_empty_and_linked_git_trees() {
    let directory = tempdir().unwrap();
    let root = directory.path();
    init(root);
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/file"), b"commit $Format:%H$\n").unwrap();
    git(root, &["add", "."]).unwrap();
    commit(root);
    let archive = root.join("archive");
    let index = root.join("index");
    let refuse = |path, reason: &str| {
        let message = build(root, "HEAD", path, &archive, &index)
            .err()
            .unwrap()
            .to_string();
        assert!(message.contains(reason), "{message}");
        assert!(!archive.exists() && !index.exists());
    };
    refuse("missing", "empty");
    refuse("../src", "canonical");
    for (attribute, reason) in [
        ("src/file export-ignore", "complete tree"),
        ("src/file export-subst", "blob bytes"),
    ] {
        fs::write(root.join(".gitattributes"), attribute).unwrap();
        git(root, &["add", ".gitattributes"]).unwrap();
        commit(root);
        refuse("src", reason);
    }
    let blob = revision_id(root, "HEAD:src/file").unwrap();
    git(
        root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("120000,{blob},src/link"),
        ],
    )
    .unwrap();
    commit(root);
    refuse("src", "Unsupported Git file");
}

#[test]
fn refuses_occupied_aliasing_and_overlapping_output_paths() {
    let directory = tempdir().unwrap();
    let root = directory.path();
    let file = root.join("occupied");
    let new = root.join("new");
    fs::write(&file, b"keep").unwrap();
    let refuse = |archive: &Path, index: &Path| {
        let error = build(root, "HEAD", "src", archive, index)
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("exists") || error.contains("distinct files"),
            "{error}"
        );
        assert!(!new.exists());
        assert_eq!(fs::read(&file).unwrap(), b"keep");
    };
    refuse(&file, &new);
    refuse(&new, &file);
    refuse(&new, &root.join("./new"));
    refuse(&new, &new.join("index"));
    #[cfg(unix)]
    refuse(&new, &new.join("../index"));
    #[cfg(unix)]
    {
        let link = root.join("broken");
        std::os::unix::fs::symlink(root.join("absent"), &link).unwrap();
        refuse(&link, &new);
        refuse(&new, &link);
        assert!(link.is_symlink());
    }
}

#[test]
fn concurrent_builders_publish_only_complete_pairs() {
    let directory = tempdir().unwrap();
    let root = directory.path();
    init(root);
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/file"), b"data").unwrap();
    git(root, &["add", "."]).unwrap();
    commit(root);
    let index = root.join("index.json");
    let archives = [root.join("one"), root.join("two")];
    let results = std::thread::scope(|scope| {
        archives
            .each_ref()
            .map(|archive| scope.spawn(|| build(root, "HEAD", "src", archive, &index)))
            .map(|thread| thread.join().unwrap().is_ok())
    });
    assert_eq!(results.iter().filter(|&&ok| ok).count(), 1);
    for (archive, ok) in archives.iter().zip(results) {
        assert_eq!(archive.exists(), ok);
        if ok {
            assert_eq!(verified_files(archive, &index).unwrap().receipt.files, 1);
            let doc: Value = serde_json::from_slice(&fs::read(&index).unwrap()).unwrap();
            assert_eq!(
                doc["archive"]["path"],
                archive.file_name().unwrap().to_str().unwrap()
            );
        }
    }
}

#[test]
fn reproduces_preserved_pair_when_source_commit_is_available() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let commit = "7e27757c34b6fa57ecc69660e263c99e1420649b";
    if git(&root, &["cat-file", "-e", &format!("{commit}^{{commit}}")]).is_err() {
        // Source distributions and shallow checkouts can still verify and restore.
        return;
    }
    let directory = tempdir().unwrap();
    let archive = directory.path().join("concert-7e27757.tar.gz");
    let index = directory.path().join("concert-7e27757.index.json");
    let receipt = build(&root, commit, "examples/concert-app", &archive, &index).unwrap();
    assert_eq!(
        receipt.archive_sha256,
        "4888e79870a157e9521e2a5ef226bcbd076fda73c45e2c3c7ee9291d34b50d2d"
    );
    for output in [&archive, &index] {
        let historical = root
            .join("archive/experiments")
            .join(output.file_name().unwrap());
        assert!(
            fs::read(output).unwrap() == fs::read(historical).unwrap(),
            "{} differs",
            output.display()
        );
    }
}
