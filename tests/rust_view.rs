//! Portable native artifact publication, admission and offline fidelity.

use gdd::{perspectives::load_perspective_bundle, view};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn generate(model: &Path, output: &Path, scope: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gdd"));
    command.arg("view").arg(model).arg("--output").arg(output);
    for id in scope {
        command.arg("--scope").arg(id);
    }
    command.output().unwrap()
}

const SKETCH: &str = include_str!("../examples/sketch.gdd.yaml");

#[test]
fn relocated_file_retains_one_capture_with_declared_fragments_and_hashes() {
    let inputs = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    let model = inputs.path().join("model.yaml");
    let fragment = include_str!("../examples/note.fragment.yaml");
    let private = "External normative text stays in its own file.";
    let hash = format!("{:x}", Sha256::digest(private));
    fs::write(inputs.path().join("contract.txt"), private).unwrap();
    fs::write(inputs.path().join("note.fragment.yaml"), fragment).unwrap();
    let manifest = include_str!("../examples/fragmented.gdd.yaml");
    fs::write(&model, format!("{manifest}\nsources:\n  source.contract:\n    role: knowledge\n    title: Contract\n    uri: contract.txt\n    format: text\n    normative: true\n    revision:\n      sha256: {hash}\n")).unwrap();
    let capture = load_perspective_bundle(&model).unwrap();
    let output = inputs.path().join("shared.gddview");
    let generated = generate(&model, &output, &["component.notes", "component.notes"]);
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let report: Value = serde_json::from_slice(&generated.stdout).unwrap();
    assert_eq!(report["checks"][0]["check_id"], "integration.visualization");
    assert_eq!(report["checks"][0]["result"], "PASS");
    assert_eq!(report["scope"], json!(["component.notes"]));
    assert_eq!(report["checks"][0]["scope"], report["scope"]);
    let moved = destination.path().join("received.gddview");
    fs::rename(output, &moved).unwrap();
    inputs.close().unwrap();
    let bytes = fs::read(&moved).unwrap();
    assert!(!std::str::from_utf8(&bytes).unwrap().contains(private));
    let data = view::read_view(&bytes).unwrap();
    assert_eq!(
        data,
        view::view_data(&capture, &["component.notes".into()]).unwrap()
    );
    assert_eq!(report["spec_revision"], data["snapshot"]["revision"]);
    assert!(
        data["snapshot"]["assets"]
            .as_array()
            .unwrap()
            .contains(&json!({"path":"contract.txt", "sha256":hash}))
    );
    assert_eq!(
        data["manifest"],
        json!({"shown_nodes":["component.notes"],"omitted_nodes":["entity.note"],"shown_edges":[],"omitted_edges":["edge.note.owner"]})
    );
    assert_eq!(
        data["locations"]["entity.note"],
        json!(["note.fragment.yaml", "/nodes/entity.note"])
    );
    assert_eq!(data["model"]["edges"][0]["from"], "entity.note");
    assert_eq!(data["perspectives"]["format"], "gdd-perspectives/0.1");
    assert!(data.get("boundaries").is_none());
}

#[test]
fn reader_rejects_invalid_graphs_and_partitions() {
    let dir = tempfile::tempdir().unwrap();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/fragmented.gdd.yaml");
    let file = dir.path().join("view");
    view::render_view(&path, &file, &["component.notes".into()]).unwrap();
    let original: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
    for (pointer, value) in [
        ("/model/edges/0/to", json!("unknown")),
        ("/model/edges/0/id", json!("entity.note")),
        ("/model/nodes/component.notes", json!("component")),
        (
            "/manifest/shown_nodes",
            json!(["component.notes", "entity.note"]),
        ),
        ("/manifest/omitted_nodes", json!([])),
        ("/manifest/shown_edges", json!([null])),
        ("/scope", json!(["entity.note"])),
        ("/locations/edge.note.owner", json!([42, "/edges/0"])),
        ("/spec_revision", json!("stale")),
        ("/perspectives", json!(false)),
    ] {
        let mut altered = original.clone();
        *altered.pointer_mut(pointer).unwrap() = value;
        assert!(
            view::read_view(&serde_json::to_vec(&altered).unwrap()).is_err(),
            "{pointer}"
        );
    }
    for bytes in [
        b"---\nformat: gdd-native-view/0.1".as_slice(),
        &[0xff],
        b"{}{}",
    ] {
        assert!(view::read_view(bytes).is_err());
    }
}

#[test]
fn admission_and_publication_fail_without_partial_or_replaced_files() {
    let dir = tempfile::tempdir().unwrap();
    let model = dir.path().join("model.yaml");
    let output = dir.path().join("view.gddview");
    for (source, scope, expected) in [
        (SKETCH.to_owned(), vec!["unknown"], 1),
        (SKETCH.replace("gdd/0.2", "gdd/99"), vec![], 2),
        (format!("{SKETCH}\nincludes: [missing.yaml]"), vec![], 2),
        (
            SKETCH.replace(
                "edges: []",
                "edges: [{id: edge.bad, from: action.hello, relation: depends_on, to: unknown}]",
            ),
            vec![],
            1,
        ),
        (
            SKETCH.replace(
                "edges: []",
                &format!(
                    "    annotations:\n      test.deep: {}0{}\nedges: []",
                    "[".repeat(251),
                    "]".repeat(251)
                ),
            ),
            vec![],
            2,
        ),
    ] {
        fs::write(&model, source).unwrap();
        let result = generate(&model, &output, &scope);
        assert_eq!(
            result.status.code(),
            Some(expected),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!output.exists());
    }
    fs::write(&model, SKETCH).unwrap();
    fs::write(&output, "preserved").unwrap();
    assert_eq!(generate(&model, &output, &[]).status.code(), Some(1));
    assert_eq!(fs::read_to_string(&output).unwrap(), "preserved");
}
