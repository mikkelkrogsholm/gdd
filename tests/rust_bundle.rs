use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use gdd::bundle::{DocumentKind, Location, Outcome, load};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn model() -> Value {
    json!({
        "format": "gdd/0.3",
        "project": {"id": "example.bundle", "title": "Bundle", "goal": "Check assets."},
        "nodes": {},
        "edges": [],
    })
}

fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn snapshot(path: &Path, output: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gdd"));
    command.arg("snapshot").arg(path);
    if let Some(path) = output {
        command.arg("--output").arg(path);
    }
    command.output().unwrap()
}

#[test]
fn existing_examples_match_python_revisions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    for (name, paths, expected) in [
        (
            "fragmented.gdd.yaml",
            &["fragmented.gdd.yaml", "note.fragment.yaml"][..],
            "a0462bbd9495087a06ed617781cbe0ff6e56475601bab6f97c37f73279f9ce23",
        ),
        (
            "concert.gdd.yaml",
            &["concert.gdd.yaml"],
            "6840144caf6ee5566d01d54f663fdf7d83a3b7c8ad309c90952eecfdb6d6bd5a",
        ),
        (
            "concert-0.3.gdd.yaml",
            &["concert-0.3.gdd.yaml"],
            "3a44421b96560abf0761f64e6d1a48db88a65f3f11f563ea134a33ffb1bf8d7b",
        ),
        (
            "product/greenfield.gdd.json",
            &["extensions/product-0.1.json", "greenfield.gdd.json"],
            "9e25335a2908ce72c78ffef55ec1e748f07f3e8a6d8fa668c788c2a2f5e84f3a",
        ),
    ] {
        let path = root.join(name);
        let bundle = load(&path);
        assert_eq!(bundle.exit_code(), 0, "{:?}", bundle.diagnostics());
        let revision = bundle.snapshot().unwrap();
        assert_eq!(revision.revision, format!("sha256:{expected}"));
        assert_eq!(
            revision
                .assets
                .iter()
                .map(|asset| asset.path.as_str())
                .collect::<Vec<_>>(),
            paths,
        );
        for asset in &revision.assets {
            let bytes = fs::read(path.parent().unwrap().join(&asset.path)).unwrap();
            assert_eq!(asset.sha256, digest(&bytes));
        }
    }
}

#[test]
fn merges_fragments_and_preserves_original_documents_and_locations() {
    let bundle = load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/fragmented.gdd.yaml"));
    assert!(bundle.diagnostics().is_empty());
    let fragment = &bundle.documents()[1];
    assert_eq!(fragment.kind, DocumentKind::Fragment02);
    let inherited = bundle
        .nodes()
        .find(|node| node.id == "entity.note")
        .unwrap();
    assert_eq!(
        inherited.location,
        &Location {
            file: "note.fragment.yaml".into(),
            pointer: "/nodes/entity.note".into()
        },
    );
    let gdd::input::Value::Object(value) = &bundle.manifest().unwrap().value else {
        panic!("expected an object");
    };
    let gdd::input::Value::Object(nodes) = &value["nodes"] else {
        panic!("expected nodes");
    };
    assert_eq!(nodes.len(), 1);
    assert_eq!(bundle.edges().next().unwrap().location.pointer, "/edges/0");
}

#[test]
fn pinned_bytes_and_operational_records_are_independent() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("gdd.json");
    let source_bytes = b"Exact source bytes\r\n";
    fs::write(root.path().join("design.md"), source_bytes).unwrap();
    let mut manifest = model();
    manifest["sources"] = json!({
        "source.design": {"normative": true, "uri": "./design.md", "revision": {"sha256": digest(source_bytes)}}
    });
    manifest["record_files"] = json!(["records.json"]);
    write_json(&path, &manifest);
    let records = root.path().join("records.json");
    write_json(
        &records,
        &json!({
            "format": "gdd-records/0.2",
            "project_id": "example.bundle",
            "records": {"record.one": {}},
        }),
    );

    let bundle = load(&path);
    assert_eq!(bundle.exit_code(), 0);
    let asset = &bundle.normative_sources()["source.design"];
    assert_eq!(asset.path, "design.md");
    assert_eq!(asset.bytes, source_bytes);
    let revision = &bundle.snapshot().unwrap().revision;
    assert_eq!(bundle.snapshot().unwrap().assets.len(), 2);
    assert_eq!(
        bundle.records().next().unwrap().location.pointer,
        "/records/record.one"
    );

    write_json(
        &records,
        &json!({
            "format": "gdd-records/0.2",
            "project_id": "another.project",
            "records": {"record.changed": {}},
        }),
    );
    let changed = load(&path);
    assert_eq!(&changed.snapshot().unwrap().revision, revision);
    assert_eq!(changed.diagnostics()[0].code, "RECORD_INVALID");

    fs::write(root.path().join("design.md"), b"changed").unwrap();
    let mismatch = load(&path);
    assert!(mismatch.snapshot().is_none());
    assert!(
        mismatch
            .diagnostics()
            .iter()
            .any(|item| item.code == "CONTRACT_SOURCE_UNPINNED")
    );
}

#[test]
fn rejects_fragment_version_and_duplicate_model_ids() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("gdd.json");
    let fragment = root.path().join("fragment.json");
    let mut manifest = model();
    manifest["includes"] = json!(["fragment.json"]);
    manifest["nodes"] = json!({"node.same": {}});
    manifest["edges"] = json!([{"id": "edge.same", "from": "first"}]);
    write_json(&path, &manifest);
    let mut included = json!({
        "format": "gdd-fragment/0.2",
        "nodes": {"node.same": {"description": "duplicate"}},
        "edges": [{"id": "edge.same", "from": "second"}],
    });
    write_json(&fragment, &included);
    assert_eq!(load(&path).diagnostics()[0].code, "SCHEMA_INVALID");

    included["format"] = json!("gdd-fragment/0.3");
    write_json(&fragment, &included);
    let bundle = load(&path);
    assert_eq!(bundle.exit_code(), 1);
    assert_eq!(bundle.snapshot().unwrap().assets.len(), 2);
    let result = snapshot(&path, None);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(bundle.nodes().len(), 1);
    assert_eq!(bundle.edges().len(), 1);
    assert_eq!(bundle.nodes().next().unwrap().location.file, "gdd.json");
    let diagnostics = bundle.diagnostics();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].code, "ID_COLLISION");
    assert_eq!(diagnostics[0].location.file, "fragment.json");
    assert_eq!(diagnostics[0].location.pointer, "/nodes/node.same");
    assert_eq!(diagnostics[1].code, "ID_COLLISION");
    assert_eq!(diagnostics[1].location.pointer, "/edges/0");
}

#[test]
fn rejects_wrong_extension_digest_or_identity() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("gdd.json");
    let extension = root.path().join("extension.json");
    let data = json!({
        "format": "gdd-extension/0.2",
        "namespace": "example.kind",
        "version": "1.0.0",
        "node_kinds": {},
        "relations": {},
    });
    write_json(&extension, &data);
    let mut manifest = model();
    manifest["extensions"] = json!({
        "extension.one": {"path": "extension.json", "namespace": "example.kind", "version": "1.0.0", "sha256": "0".repeat(64)},
    });
    write_json(&path, &manifest);
    assert_eq!(load(&path).diagnostics()[0].code, "EXTENSION_DATA_INVALID");

    manifest["extensions"]["extension.one"]["sha256"] =
        json!(digest(&fs::read(&extension).unwrap()));
    manifest["extensions"]["extension.one"]["version"] = json!("2.0.0");
    write_json(&path, &manifest);
    let bundle = load(&path);
    assert_eq!(bundle.diagnostics()[0].code, "EXTENSION_IDENTITY_MISMATCH");
    assert_eq!(bundle.snapshot().unwrap().assets.len(), 2);
    assert_eq!(snapshot(&path, None).status.code(), Some(1));

    manifest["extensions"]["extension.one"]["version"] = json!("1.0.0");
    write_json(&path, &manifest);
    let bundle = load(&path);
    assert_eq!(bundle.extensions().next().unwrap().0, "example.kind");
    assert_eq!(bundle.documents()[1].kind, DocumentKind::Extension);
    assert_eq!(bundle.snapshot().unwrap().assets.len(), 2);
}

#[cfg(unix)]
#[test]
fn resolves_local_symlinks_and_rejects_escapes_loops_and_alias_duplicates() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("bundle");
    fs::create_dir(&root).unwrap();
    let path = root.join("gdd.json");
    let fragment = json!({"format": "gdd-fragment/0.3", "nodes": {}, "edges": []});
    write_json(&root.join("é.json"), &fragment);
    write_json(&directory.path().join("outside.json"), &fragment);
    symlink("é.json", root.join("alias.json")).unwrap();
    symlink("../outside.json", root.join("escape.json")).unwrap();
    symlink("../missing.json", root.join("dangling-escape.json")).unwrap();
    symlink("missing.json", root.join("dangling-local.json")).unwrap();
    symlink("loop.json", root.join("loop.json")).unwrap();
    let mut manifest = model();

    for paths in [
        json!(["escape.json"]),
        json!(["dangling-escape.json"]),
        json!(["loop.json"]),
        json!(["alias.json", "./é.json"]),
        json!(["../outside.json"]),
        json!(["never-created/../../missing.json"]),
    ] {
        manifest["includes"] = paths;
        write_json(&path, &manifest);
        let bundle = load(&path);
        assert_eq!(bundle.exit_code(), 1, "{:?}", bundle.diagnostics());
        assert_eq!(bundle.diagnostics()[0].code, "ASSET_PATH_INVALID");
        assert!(bundle.snapshot().is_none());
    }
    manifest["includes"] = json!(["dangling-local.json"]);
    write_json(&path, &manifest);
    let bundle = load(&path);
    assert_eq!(bundle.exit_code(), 2);
    assert_eq!(bundle.diagnostics()[0].outcome, Outcome::Unknown);

    manifest["includes"] = json!(["./alias.json"]);
    write_json(&path, &manifest);
    let bundle = load(&path);
    assert_eq!(bundle.snapshot().unwrap().assets[1].path, "é.json");
}

#[test]
fn snapshot_cli_writes_exactly_once_and_distinguishes_load_failure() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("gdd.json");
    write_json(&path, &model());
    let destination = root.path().join("snapshot.json");
    let stdout = snapshot(&path, None);
    assert!(stdout.status.success());
    assert!(snapshot(&path, Some(&destination)).status.success());
    assert_eq!(fs::read(&destination).unwrap(), stdout.stdout);
    assert_eq!(snapshot(&path, Some(&destination)).status.code(), Some(1));
    assert_eq!(fs::read(&destination).unwrap(), stdout.stdout);

    write_json(&path, &json!({"format": "gdd-report/0.2"}));
    assert_eq!(
        load(&path).root_document().unwrap().kind,
        DocumentKind::Report
    );
    assert_eq!(snapshot(&path, None).status.code(), Some(2));

    fs::write(&path, b"{\"format\":\"gdd/0.3\",\"format\":\"gdd/0.2\"}").unwrap();
    let bad = snapshot(&path, None);
    assert_eq!(bad.status.code(), Some(1));
    assert!(
        String::from_utf8(bad.stderr)
            .unwrap()
            .contains("YAML_DUPLICATE_KEY")
    );
    let unavailable = snapshot(&root.path().join("missing.json"), None);
    assert_eq!(unavailable.status.code(), Some(2));
    assert!(
        String::from_utf8(unavailable.stderr)
            .unwrap()
            .contains("SOURCE_UNAVAILABLE")
    );
}

#[test]
fn external_normative_sources_are_unresolved_and_drive_paths_are_invalid() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("gdd.json");
    let mut manifest = model();
    for (uri, code, outcome) in [
        (
            "https://example.test/reference",
            "SOURCE_UNAVAILABLE",
            Outcome::Unknown,
        ),
        (
            "mailto:author@example.test",
            "SOURCE_UNAVAILABLE",
            Outcome::Unknown,
        ),
        (
            "urn:example:reference",
            "SOURCE_UNAVAILABLE",
            Outcome::Unknown,
        ),
        ("C:other.json", "ASSET_PATH_INVALID", Outcome::Fail),
    ] {
        manifest["sources"] = json!({
            "source.design": {"normative": true, "uri": uri, "revision": {"sha256": "0".repeat(64)}}
        });
        write_json(&path, &manifest);
        let bundle = load(&path);
        assert!(bundle.snapshot().is_none());
        let issue = &bundle.diagnostics()[0];
        assert_eq!(issue.code, code);
        assert_eq!(issue.outcome, outcome);
        assert_eq!(issue.location.pointer, "/sources/source.design");
    }
}
