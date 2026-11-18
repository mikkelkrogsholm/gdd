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

fn assert_verdict(report: &Value, result: &str, code: &str) {
    let check = &report["checks"][0];
    assert_eq!(check["check_id"], "integration.visualization");
    assert_eq!(check["result"], result);
    assert_eq!(check["diagnostics"][0]["code"], code);
}

#[test]
fn exact_verification_distinguishes_selector_revision_bytes_scope_and_capacity() {
    let dir = tempfile::tempdir().unwrap();
    let model = Path::new("examples/fragmented.gdd.yaml");
    let file = dir.path().join("portable");
    assert_verdict(
        &view::verify_view(model, None).unwrap(),
        "UNKNOWN",
        "VIEW_INPUT_MISSING",
    );
    assert_verdict(
        &view::verify_view(model, Some(&file)).unwrap(),
        "UNKNOWN",
        "SOURCE_UNAVAILABLE",
    );
    view::render_view(model, &file, &["component.notes".into()]).unwrap();
    let capture = load_perspective_bundle(model).unwrap();
    let bytes = fs::read(&file).unwrap();
    let verified = view::verify_view(model, Some(&file)).unwrap();
    assert_eq!(verified["scope"], json!(["component.notes"]));
    assert_eq!(verified["checks"][0]["result"], "PASS");
    assert_eq!(verified, view::verify_view_captured(&capture, Some(&bytes)));
    let original = view::read_view(&bytes).unwrap();
    for (pointer, value, result, code) in [
        (
            "/generator/version",
            json!("99"),
            "UNKNOWN",
            "FORMAT_UNSUPPORTED",
        ),
        (
            "/generator/name",
            json!("another"),
            "UNKNOWN",
            "FORMAT_UNSUPPORTED",
        ),
        (
            "/generator/profile",
            json!("future/0.2"),
            "UNKNOWN",
            "FORMAT_UNSUPPORTED",
        ),
        (
            "/spec_revision",
            json!("stale"),
            "FAIL",
            "VIEW_REVISION_MISMATCH",
        ),
        (
            "/scope",
            json!(["unknown"]),
            "FAIL",
            "VIEW_ARTIFACT_MISMATCH",
        ),
        (
            "/scope",
            json!(["component.notes", "component.notes"]),
            "FAIL",
            "VIEW_ARTIFACT_MISMATCH",
        ),
        ("/scope", json!([1]), "FAIL", "VIEW_ARTIFACT_MISMATCH"),
        (
            "/model/edges/0/to",
            json!("entity.note"),
            "FAIL",
            "VIEW_ARTIFACT_MISMATCH",
        ),
    ] {
        let mut altered = original.clone();
        *altered.pointer_mut(pointer).unwrap() = value;
        let encoded = serde_json::to_vec(&altered).unwrap();
        assert_verdict(
            &view::verify_view_captured(&capture, Some(&encoded)),
            result,
            code,
        );
    }
    let depth = format!("{}0{}", "[".repeat(257), "]".repeat(257));
    assert_verdict(
        &view::verify_view_captured(&capture, Some(depth.as_bytes())),
        "UNKNOWN",
        "FORMAT_UNSUPPORTED",
    );
    for invalid in [
        &bytes[..bytes.len() - 1],
        b"{}",
        b"{",
        &[0xff][..],
        b"{\"a\":1,\"a\":2}",
    ] {
        assert_verdict(
            &view::verify_view_captured(&capture, Some(invalid)),
            "FAIL",
            "VIEW_ARTIFACT_MISMATCH",
        );
    }
    assert_eq!(fs::read(file).unwrap(), bytes);
}

#[test]
fn historical_html_selectors_are_passive_unsupported_inputs() {
    let model = Path::new("examples/sketch.gdd.yaml");
    let capture = load_perspective_bundle(model).unwrap();
    let marker = r#"<script id="gdd-data" type="application/json">"#;
    // These envelope selectors occur in the published concert HTML at
    // 73494ed and 1e16fa1. The native verifier only reads the JSON selector.
    for (format, version) in [("gdd-view/0.2", "0.3.0"), ("gdd-view/0.3", "0.4.0")] {
        let payload = json!({"format":format,"generator":{"name":"gdd-viewer","profile":"gdd-viewer-html/0.1","version":version}});
        let shell = format!("<!doctype html><html><body>{marker}{payload}</script></body></html>");
        assert_verdict(
            &view::verify_view_captured(&capture, Some(shell.as_bytes())),
            "UNKNOWN",
            "FORMAT_UNSUPPORTED",
        );
        for invalid in [
            shell.replace(marker, ""),
            shell.replace(marker, &marker.repeat(2)),
            shell.replace("</html>", ""),
            shell.replace("gdd-viewer-html/0.1", "arbitrary"),
        ] {
            assert_verdict(
                &view::verify_view_captured(&capture, Some(invalid.as_bytes())),
                "FAIL",
                "VIEW_ARTIFACT_MISMATCH",
            );
        }
    }
}

#[test]
fn combined_verification_uses_the_captured_inputs_and_independent_scopes() {
    use gdd::validation::{Profile, ValidationOptions, validate_captured_with_view};
    let dir = tempfile::tempdir().unwrap();
    let model = dir.path().join("model.yaml");
    fs::write(&model, include_bytes!("../examples/fragmented.gdd.yaml")).unwrap();
    fs::write(
        dir.path().join("note.fragment.yaml"),
        include_bytes!("../examples/note.fragment.yaml"),
    )
    .unwrap();
    let ledger = dir.path().join("ledger.json");
    fs::write(
        &ledger,
        br#"{"format":"gdd-records/0.2","project_id":"example.fragment","records":{}}"#,
    )
    .unwrap();
    let view_file = dir.path().join("file.gddview");
    view::render_view(&model, &view_file, &["component.notes".into()]).unwrap();
    let audit_file = dir.path().join("audit.json");
    let audit = br#"{"format":"gdd-import-audit/0.2","original":[1,2],"reconstructed":[2,1]}"#;
    fs::write(&audit_file, audit).unwrap();
    let observations = Path::new("examples/graphify/observations");
    let output = dir.path().join("report.json");
    let invocation = Command::new(env!("CARGO_BIN_EXE_gdd"))
        .arg("validate")
        .arg(&ledger)
        .arg("--model")
        .arg(&model)
        .args([
            "--profile",
            "readiness",
            "--profile",
            "import.graphify",
            "--profile",
            "visualization",
            "--profile",
            "records",
            "--scope",
            "entity.note",
            "--graphify",
        ])
        .arg(observations)
        .arg("--import-audit")
        .arg(&audit_file)
        .arg("--view")
        .arg(&view_file)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert_eq!(
        invocation.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&invocation.stderr)
    );
    assert!(invocation.stdout.is_empty());
    let cli: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    let observation_report = gdd::graphify::verify_graphify(observations, None, None).unwrap();
    let model = gdd::bundle::load_with_records(&model, &ledger);
    let bytes = fs::read(view_file).unwrap();
    let audit = gdd::input::parse(audit).unwrap();
    dir.close().unwrap();
    let captured = validate_captured_with_view(
        Some(&model),
        Some(&observation_report),
        ValidationOptions {
            profiles: &[
                Profile::Readiness,
                Profile::ImportGraphify,
                Profile::Visualization,
                Profile::Records,
            ],
            scope: &["entity.note".into()],
            ..Default::default()
        },
        Some(&audit),
        Some(&bytes),
        None,
    )
    .unwrap();
    assert_eq!(cli, serde_json::to_value(captured).unwrap());
    assert_eq!(cli["scope"], json!(["entity.note"]));
    let checks = cli["checks"].as_array().unwrap();
    let loss = checks
        .iter()
        .find(|c| c["check_id"] == "integration.loss")
        .unwrap();
    assert_eq!(
        (loss["result"].as_str(), &loss["scope"]),
        (Some("FAIL"), &cli["scope"])
    );
    let check = checks.last().unwrap();
    assert_eq!(check["check_id"], "integration.visualization");
    assert_eq!(check["result"], "PASS");
    assert_eq!(check["scope"], json!(["component.notes"]));
}

#[test]
fn invalid_models_gate_verification_before_reading_the_view() {
    use gdd::validation::{Profile, ValidationOptions, validate_paths_with_integrations};
    let dir = tempfile::tempdir().unwrap();
    let model = dir.path().join("model.yaml");
    let view = dir.path().join("missing");
    for (source, exit) in [
        (SKETCH.replace("gdd/0.2", "gdd/99"), 2),
        (
            SKETCH.replace(
                "edges: []",
                "edges: [{id: edge.bad, from: action.hello, relation: depends_on, to: unknown}]",
            ),
            1,
        ),
    ] {
        fs::write(&model, source).unwrap();
        let report = validate_paths_with_integrations(
            &model,
            None,
            ValidationOptions {
                profiles: &[Profile::Visualization],
                ..Default::default()
            },
            Default::default(),
            Some(&view),
            None,
        )
        .unwrap();
        assert_eq!(report.exit_code(), exit);
        let check = report.checks.last().unwrap();
        assert_eq!(check.diagnostics[0].code, "REVISION_UNRESOLVED");
        assert_eq!(check.result, gdd::report::CheckResult::Unknown);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn file_reader_rejects_a_fifo_without_waiting_for_a_writer() {
    use rustix::fs::{CWD, Mode, mkfifoat};
    use std::{sync::mpsc, thread, time::Duration};
    let dir = tempfile::tempdir().unwrap();
    assert!(view::read_view_bytes(dir.path()).is_err());
    let path = dir.path().join("pipe.gddview");
    mkfifoat(CWD, &path, Mode::RUSR | Mode::WUSR).unwrap();
    let (send, recv) = mpsc::channel();
    thread::spawn(move || send.send(view::read_view_bytes(&path)).unwrap());
    let result = recv
        .recv_timeout(Duration::from_secs(3))
        .expect("view reader blocked on a FIFO");
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
}
