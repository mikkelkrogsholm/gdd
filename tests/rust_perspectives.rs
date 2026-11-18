use gdd::bundle::load_model_only;
use gdd::perspectives::{
    PerspectiveBundle, load_perspective_bundle, perspective_data, perspectives,
};
use gdd::validation::build_schema;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const EXAMPLE: &[u8] = include_bytes!("../examples/product/greenfield.gdd.json");
const PRODUCT: &[u8] = include_bytes!("../examples/product/extensions/product-0.1.json");

fn fixture(dir: &Path) -> (PathBuf, Value) {
    fs::create_dir(dir.join("extensions")).unwrap();
    fs::write(dir.join("extensions/product-0.1.json"), PRODUCT).unwrap();
    let path = dir.join("model.json");
    fs::write(&path, EXAMPLE).unwrap();
    (path, serde_json::from_slice(EXAMPLE).unwrap())
}

fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect()
}

#[test]
fn fragments_keep_edge_identity_and_directed_witnesses_across_views() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut model) = fixture(dir.path());
    let mut fragment = json!({
        "format":"gdd-fragment/0.3",
        "nodes":{"actor.guest": model["nodes"].as_object_mut().unwrap().remove("actor.guest").unwrap()},
        "edges":[
            {"id":"edge.loop","from":"step.reserve","to":"step.reserve","relation":"gdd.product.next","data":{}},
            {"id":"edge.parallel","from":"step.choose","to":"step.reserve","relation":"gdd.product.next","data":{}},
            {"id":"edge.reverse","from":"step.reserve","to":"step.choose","relation":"gdd.product.next","data":{}},
            {"id":"edge.uses_again","from":"component.booking","to":"entity.reservation","relation":"gdd.product.uses","data":{}},
            {"id":"edge.read","from":"action.reserve","to":"entity.reservation","relation":"reads"},
            {"id":"edge.change","from":"action.reserve","to":"entity.reservation","relation":"changes"},
            {"id":"edge.owner","from":"entity.reservation","to":"component.booking","relation":"owned_by"},
            {"id":"edge.storage","from":"entity.reservation","to":"component.booking","relation":"stored_in"}
        ]
    });
    model["includes"] = json!(["slice.json"]);
    write(&path, &model);
    let fragment_path = dir.path().join("slice.json");
    write(&fragment_path, &fragment);
    let mut result = perspectives(&path).unwrap();
    let schema = serde_json::from_str(include_str!("../schemas/perspectives.schema.json")).unwrap();
    build_schema(&schema).unwrap().validate(&result).unwrap();

    let nodes: BTreeMap<_, _> = [&model, &fragment]
        .into_iter()
        .flat_map(|doc| doc["nodes"].as_object().unwrap())
        .collect();
    let edges: BTreeMap<_, _> = [&model, &fragment]
        .into_iter()
        .flat_map(|doc| doc["edges"].as_array().unwrap())
        .map(|e| (e["id"].as_str().unwrap(), e))
        .collect();
    for view in result["views"].as_object().unwrap().values() {
        let primary: BTreeSet<_> = strings(&view["primary_node_ids"]).into_iter().collect();
        let mut shown = primary.clone();
        let mut included = Vec::new();
        let mut omitted = Vec::new();
        for (&id, &edge) in &edges {
            let from = edge["from"].as_str().unwrap();
            let to = edge["to"].as_str().unwrap();
            if primary.contains(from) || primary.contains(to) {
                included.push(id);
                shown.extend([from, to]);
            } else {
                omitted.push(id);
            }
        }
        assert_eq!(strings(&view["shown"]["edge_ids"]), included);
        assert_eq!(strings(&view["omitted"]["edge_ids"]), omitted);
        assert_eq!(
            strings(&view["shown"]["node_ids"]),
            shown.iter().copied().collect::<Vec<_>>()
        );
        assert_eq!(
            strings(&view["omitted"]["node_ids"]),
            nodes
                .keys()
                .map(|k| k.as_str())
                .filter(|k| !shown.contains(k))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            view["edges"].as_array().unwrap().iter().collect::<Vec<_>>(),
            included.iter().map(|id| edges[id]).collect::<Vec<_>>()
        );
        for (id, node) in view["nodes"].as_object().unwrap() {
            assert_eq!(node, nodes[id]);
        }
        for link in view["cross_links"].as_array().unwrap() {
            let id = &link["node_id"];
            let touching: Vec<_> = included
                .iter()
                .copied()
                .filter(|key| {
                    let edge = edges[key];
                    &edge["from"] == id || &edge["to"] == id
                })
                .collect();
            assert_eq!(strings(&link["edge_ids"]), touching);
        }
    }
    let routes: Vec<_> = result["component_data_links"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|link| {
            link["component"] == "component.booking" && link["entity"] == "entity.reservation"
        })
        .map(|link| (link["via"].as_str().unwrap(), strings(&link["edge_ids"])))
        .collect();
    assert_eq!(
        routes,
        [
            ("changes", vec!["edge.change", "edge.reserve.owner"]),
            (
                "creates",
                vec!["edge.reserve.creates", "edge.reserve.owner"]
            ),
            ("owned_by", vec!["edge.owner"]),
            ("reads", vec!["edge.read", "edge.reserve.owner"]),
            ("stored_in", vec!["edge.storage"]),
            ("uses", vec!["edge.booking.reservation"]),
            ("uses", vec!["edge.uses_again"])
        ]
    );
    assert_eq!(result["gaps"], json!([]));

    fragment["edges"].as_array_mut().unwrap().reverse();
    write(&fragment_path, &fragment);
    let mut reordered = perspectives(&path).unwrap();
    assert_ne!(
        result.as_object_mut().unwrap().remove("spec_revision"),
        reordered.as_object_mut().unwrap().remove("spec_revision")
    );
    assert_eq!(result, reordered);
}

#[test]
fn captured_validation_and_detached_projections_need_no_files() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut model) = fixture(dir.path());
    // The parser and serializer must treat every nested object key as user data.
    model["nodes"]["entity.performance"]["annotations"] = json!({
        "test.payload": {"$serde_json::private::Number":"hello", "Å漢字👩🏽‍💻":"e\u{301}", "huge": 9007199254740993_u64}
    });
    model["nodes"]["entity.performance"]["annotations"]["test.payload"]["small"] =
        Value::Number("1e-4000".parse().unwrap());
    model["edges"][0]["annotations"] =
        json!({"test.payload": {"$serde_json::private::Number":"7"}});
    write(&path, &model);
    let path_capture = load_perspective_bundle(&path).unwrap();
    let raw = load_model_only(&path);
    dir.close().unwrap();
    let capture = PerspectiveBundle::try_from(raw).unwrap();
    let expected = perspective_data(&path_capture).unwrap();
    let mut data = perspective_data(&capture).unwrap();
    assert_eq!(data, expected);
    assert_eq!(
        data["spec_revision"],
        capture.bundle().snapshot().unwrap().revision
    );
    let payload =
        &data["views"]["data"]["nodes"]["entity.performance"]["annotations"]["test.payload"];
    assert_eq!(
        payload,
        &model["nodes"]["entity.performance"]["annotations"]["test.payload"]
    );
    let edge = data["views"]["journey"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|edge| edge["id"] == model["edges"][0]["id"])
        .unwrap();
    assert_eq!(edge, &model["edges"][0]);
    data["views"]["journey"]["nodes"]["step.reserve"]["data"]["user_action"] = json!("Changed.");
    assert_eq!(perspective_data(&capture).unwrap(), expected);
}

#[test]
fn partial_sketches_and_other_extensions_keep_their_meaning() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = fixture(dir.path());
    let mut model: Value =
        serde_json::from_slice(include_bytes!("../examples/product/partial.gdd.json")).unwrap();
    for format in ["gdd/0.2", "gdd/0.3"] {
        model["format"] = json!(format);
        write(&path, &model);
        let projection = perspectives(&path).unwrap();
        assert_eq!(
            projection["views"]["journey"]["primary_node_ids"],
            json!(["step.reserve"])
        );
        assert_eq!(
            projection["views"]["data"]["primary_node_ids"],
            json!(["entity.reservation"])
        );
        let codes: Vec<_> = projection["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["code"].as_str().unwrap())
            .collect();
        assert_eq!(
            codes,
            [
                "MISSING_DATA_COMPONENT",
                "MISSING_ACTOR",
                "MISSING_USER_ACTION",
                "MISSING_SYSTEM_RESPONSE",
                "MISSING_RESPONSIBLE_COMPONENT"
            ]
        );
    }
    let other = perspectives(Path::new("examples/extension.gdd.yaml")).unwrap();
    assert_eq!(other["unsupported_extensions"], json!(["example.sensors"]));
    assert!(other["product_extension"].is_null());
    assert_eq!(other["views"]["journey"]["nodes"], json!({}));
}

#[test]
fn repinned_equal_definitions_and_namespaced_lookalikes_are_unsupported() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut model) = fixture(dir.path());
    let def: Value = serde_json::from_slice(PRODUCT).unwrap();
    for changed in [def.clone(), {
        let mut renamed: Value = serde_json::from_str(
            &serde_json::to_string(&def)
                .unwrap()
                .replace("gdd.product.", "gdd.product.other."),
        )
        .unwrap();
        renamed["namespace"] = json!("gdd.product.other");
        renamed
    }] {
        let mut changed = changed;
        // Reorder top-level fields explicitly while preserving their contents.
        let fields = changed.as_object_mut().unwrap();
        let first = fields.shift_remove("format").unwrap();
        fields.insert("format".to_owned(), first);
        let bytes = serde_json::to_vec(&changed).unwrap();
        fs::write(dir.path().join("extensions/product-0.1.json"), &bytes).unwrap();
        if changed["namespace"] == "gdd.product.other" {
            model["nodes"] = json!({});
            model["edges"] = json!([]);
        }
        let declaration = &mut model["extensions"]["extension.product"];
        declaration["namespace"] = changed["namespace"].clone();
        declaration["sha256"] = json!(format!("{:x}", Sha256::digest(&bytes)));
        write(&path, &model);
        let error = load_perspective_bundle(&path).unwrap_err();
        assert_eq!(error.exit_code, 2, "{error}");
        let report = gdd::validation::validate(
            &load_model_only(&path),
            gdd::validation::Profile::Visualization,
        );
        let integration = report.checks.last().unwrap();
        assert_eq!(report.exit_code(), 2);
        assert_eq!(integration.check_id, "integration.visualization");
        assert_eq!(integration.diagnostics[0].code, "FORMAT_UNSUPPORTED");
        assert!(
            error
                .to_string()
                .contains("Unsupported gdd.product semantics"),
            "{error}"
        );
    }
}

#[test]
fn cli_outputs_one_complete_document_and_preserves_existing_files() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut model) = fixture(dir.path());
    let output = dir.path().join("view.json");
    let cli = |args: &[&Path]| {
        Command::new(env!("CARGO_BIN_EXE_gdd"))
            .arg("perspectives")
            .arg(&path)
            .args(args)
            .output()
            .unwrap()
    };
    let stdout = cli(&[]);
    assert!(stdout.status.success() && stdout.stderr.is_empty());
    let run = || cli(&[Path::new("--output"), &output]);
    let (first, second) = std::thread::scope(|s| {
        let competing = s.spawn(run);
        (run(), competing.join().unwrap())
    });
    let mut codes = [first.status.code().unwrap(), second.status.code().unwrap()];
    codes.sort();
    assert_eq!(codes, [0, 1]);
    assert!(first.stdout.is_empty() && second.stdout.is_empty());
    assert_eq!(fs::read(&output).unwrap(), stdout.stdout);
    model["edges"][0]["to"] = json!("step.absent");
    write(&path, &model);
    let error = run();
    assert_eq!(error.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&error.stderr).contains("REFERENCE_UNRESOLVED"));
    assert_eq!(fs::read(&output).unwrap(), stdout.stdout);
    fs::remove_file(&path).unwrap();
    let error = run();
    assert_eq!(error.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&error.stderr).contains("SOURCE_UNAVAILABLE"));
    assert_eq!(fs::read(&output).unwrap(), stdout.stdout);
    assert!(!fs::read_dir(dir.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".gdd-perspectives-")
    }));
}

#[test]
fn malformed_model_path_is_a_caller_error() {
    let error = load_perspective_bundle(Path::new("invalid\0path.json")).unwrap_err();
    assert_eq!(error.exit_code, 1);
    assert!(error.to_string().contains("NUL"));
}

#[test]
fn extension_admission_reports_observed_bytes_before_pin_errors() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut model) = fixture(dir.path());
    let asset = dir.path().join("extensions/product-0.1.json");
    for (bytes, code, diagnostics) in [
        (
            None,
            2,
            &["SOURCE_UNAVAILABLE", "EXTENSION_UNSUPPORTED"][..],
        ),
        (
            Some(&br#"{"format":"gdd-extension/99"}"#[..]),
            2,
            &["FORMAT_UNSUPPORTED", "EXTENSION_UNSUPPORTED"][..],
        ),
        (
            Some(&br#"{"format":"#[..]),
            1,
            &["YAML_FEATURE_FORBIDDEN", "EXTENSION_UNSUPPORTED"][..],
        ),
        (
            Some(&br#"{"format":"gdd-extension/0.2"}"#[..]),
            1,
            &["SCHEMA_INVALID", "EXTENSION_UNSUPPORTED"][..],
        ),
    ] {
        if let Some(bytes) = bytes {
            fs::write(&asset, bytes).unwrap();
        } else {
            fs::remove_file(&asset).unwrap();
        }
        let error = load_perspective_bundle(&path).unwrap_err();
        assert_eq!(error.exit_code, code, "{error}");
        let message = error.to_string();
        for diagnostic in diagnostics {
            assert!(message.contains(diagnostic), "{error}");
        }
        assert!(!message.contains("bytehash mismatch"), "{error}");
    }
    fs::write(&asset, PRODUCT).unwrap();
    model["extensions"]["extension.product"]["version"] = json!("0.9.0");
    write(&path, &model);
    let error = load_perspective_bundle(&path).unwrap_err();
    assert_eq!(error.exit_code, 1);
    assert!(error.to_string().contains("EXTENSION_IDENTITY_MISMATCH"));
    model["extensions"]["extension.product"]["version"] = json!("0.1.0");
    model["extensions"]["extension.product"]["sha256"] = json!("0".repeat(64));
    write(&path, &model);
    let error = load_perspective_bundle(&path).unwrap_err();
    assert_eq!(error.exit_code, 1);
    assert!(error.to_string().contains("EXTENSION_DATA_INVALID"));
}
