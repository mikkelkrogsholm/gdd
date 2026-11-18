use gdd::graphify::{GraphifyOptions, NODE_LINK_PROFILE, import_graphify, propose_graphify};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::tempdir;

const RAW: &str = "graphify.raw.json";
const MAPPING: &str = include_str!("../examples/graphify/mapping.json");
const EXPECTED: &str = include_str!("../examples/graphify/observations/proposal.json");

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/graphify/observations")
}

fn mapping() -> Value {
    serde_json::from_str(MAPPING).unwrap()
}

fn cli(path: &Path, mapping: &Path, output: &Path, format: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gdd"));
    command
        .args(["graphify", "propose"])
        .arg(path)
        .arg("--mapping")
        .arg(mapping)
        .arg("--output")
        .arg(output);
    if let Some(format) = format {
        command.args(["--format", format]);
    }
    command.output().unwrap()
}

#[test]
fn fixture_matches_legacy_bytes_and_current_sketch() {
    let legacy = propose_graphify(&fixture(), &mapping(), RAW, "gdd/0.2").unwrap();
    assert_eq!(
        format!("{}\n", serde_json::to_string_pretty(&legacy).unwrap()),
        EXPECTED
    );
    let mut current = legacy;
    current["model_candidate"]["format"] = "gdd/0.3".into();
    current["model_candidate"]["sources"]["source.graphify"]["uri"] = "../資料/🦀.json".into();
    assert_eq!(
        propose_graphify(
            &fixture().join("observations.json"),
            &mapping(),
            "../資料/🦀.json",
            "gdd/0.3"
        )
        .unwrap(),
        current
    );
}

#[test]
fn invalid_selections_cannot_be_promoted() {
    for (pointer, value, expected) in [
        ("/nodes/1/target", "component.reservations", "one-to-one"),
        (
            "/nodes/1/observation",
            mapping()["nodes"][0]["observation"].as_str().unwrap(),
            "one-to-one",
        ),
        (
            "/nodes/0/observation",
            "observation.missing",
            "code observation",
        ),
        ("/edges/0/observation", "observation.missing", "direction"),
        ("/edges/0/from", "action.construct_reservation", "endpoint"),
        ("/nodes/1/kind", "screen", "model semantics"),
        (
            "/nodes/0/target",
            "question.graphify_contracts",
            "ID_COLLISION",
        ),
        ("/nodes/0/rationale", "", "Invalid explicit mapping"),
    ] {
        let mut mapping = mapping();
        *mapping.pointer_mut(pointer).unwrap() = value.into();
        let err = propose_graphify(&fixture(), &mapping, RAW, "gdd/0.3").unwrap_err();
        assert!(err.to_string().contains(expected), "{pointer}: {err}");
    }
    assert!(
        propose_graphify(Path::new("absent"), &Value::Null, RAW, "gdd/0.4")
            .unwrap_err()
            .to_string()
            .contains("Proposal format")
    );
    assert!(
        propose_graphify(Path::new("absent"), &Value::Null, RAW, "gdd/0.3")
            .unwrap_err()
            .to_string()
            .contains("Invalid explicit mapping")
    );
    assert!(
        propose_graphify(&fixture(), &mapping(), "", "gdd/0.3")
            .unwrap_err()
            .to_string()
            .contains("representation")
    );
}

#[test]
fn unresolved_records_remain_omitted_and_producer_payload_stays_opaque() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("input.json");
    // Build the producer value from bytes so its private-looking JSON key is literal.
    fs::write(&input, r#"{
        "directed": false, "multigraph": true, "graph": {},
        "nodes": [
            {"id":"a","label":"🦀","source_file":"a.rs","file_type":"code","source_location":{"$serde_json::private::Number":{"<<":"01"}}},
            {"id":"b","label":"Guide","source_file":"b.md","file_type":"document"}
        ],
        "links":[{"source":"a","target":"b","relation":"calls","confidence":"EXTRACTED","source_file":"a.rs"}],
        "hyperedges":[{"members":["a","b"]}]
    }"#).unwrap();
    let bundle = dir.path().join("observations");
    let report = import_graphify(
        &input,
        &bundle,
        GraphifyOptions {
            profile: Some(NODE_LINK_PROFILE),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(report.exit_code(), 2);
    let index = gdd::graphify::load_observations(&bundle)
        .unwrap()
        .index
        .unwrap();
    let mut selection = mapping();
    selection["nodes"].as_array_mut().unwrap().truncate(1);
    selection["nodes"][0]["observation"] = index["nodes"][0]["id"].clone();
    selection["edges"] = json!([]);
    let proposal = propose_graphify(&bundle, &selection, RAW, "gdd/0.3").unwrap();
    assert_eq!(
        proposal["binding_candidates"][0]["selector_claim"]["$serde_json::private::Number"]["<<"],
        "01"
    );
    assert_eq!(
        proposal["model_candidate"]["nodes"]["component.reservations"]["label"],
        "🦀"
    );
    assert_eq!(
        proposal["omitted_observations"],
        json!([
            index["nodes"][1]["id"],
            index["edges"][0]["id"],
            index["opaque"][0]["id"]
        ])
    );
    selection["edges"] = mapping()["edges"].clone();
    selection["edges"][0]["observation"] = index["edges"][0]["id"].clone();
    assert!(
        propose_graphify(&bundle, &selection, RAW, "gdd/0.3")
            .unwrap_err()
            .to_string()
            .contains("direction")
    );
    selection["nodes"][0]["observation"] = index["nodes"][1]["id"].clone();
    assert!(
        propose_graphify(&bundle, &selection, RAW, "gdd/0.3")
            .unwrap_err()
            .to_string()
            .contains("noncode")
    );
}

#[test]
fn integrity_projection_and_original_are_required() {
    let folder = tempdir().unwrap();
    let index_path = folder.path().join("observations.json");
    fs::copy(fixture().join("observations.json"), &index_path).unwrap();
    let raw_path = folder.path().join(RAW);
    let rejected = || {
        assert!(
            propose_graphify(folder.path(), &mapping(), RAW, "gdd/0.3")
                .unwrap_err()
                .to_string()
                .contains("intact")
        )
    };
    rejected();
    fs::write(&raw_path, b"{}").unwrap();
    rejected();
    fs::copy(fixture().join(RAW), raw_path).unwrap();
    let mut index: Value = serde_json::from_slice(&fs::read(&index_path).unwrap()).unwrap();
    index["nodes"][0]["raw"]["label"] = "different".into();
    fs::write(index_path, serde_json::to_vec(&index).unwrap()).unwrap();
    rejected();
}

#[test]
fn cli_accepts_yaml_mapping_and_rejects_invalid_inputs_without_creating_output() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("mapping.data");
    let output = dir.path().join("proposal.json");
    let yaml = format!(
        "format: gdd-graphify-mapping/0.2\nproject:\n  id: project.test\n  title: 'Ångström 🦀'\n  goal: Review\nnodes:\n  - observation: {}\n    target: action.test\n    kind: action\n    description: Sketch\n    rationale: Review\nedges: []\n",
        mapping()["nodes"][0]["observation"].as_str().unwrap()
    );
    fs::write(&file, yaml).unwrap();
    let result = cli(&fixture(), &file, &output, None);
    assert!(result.status.success(), "{:?}", result.stderr);
    assert_eq!(result.stdout, fs::read(&output).unwrap());
    let data: Value = serde_json::from_slice(&result.stdout).unwrap();
    let candidate = &data["model_candidate"];
    assert_eq!(candidate["format"], "gdd/0.3");
    assert_eq!(candidate["project"]["title"], "Ångström 🦀");
    let uri = candidate["sources"]["source.graphify"]["uri"]
        .as_str()
        .unwrap();
    assert_eq!(
        dir.path().join(uri).canonicalize().unwrap(),
        fixture().join(RAW)
    );
    let repeated = cli(&fixture(), &file, &output, None);
    assert_eq!(repeated.status.code(), Some(1));
    assert!(repeated.stdout.is_empty());
    assert_eq!(result.stdout, fs::read(&output).unwrap());
    fs::remove_file(&output).unwrap();
    for bad in [
        &b"project: &a {}\nnodes: *a"[..],
        br#"{"format":1,"format":2}"#,
        br#"{"format":"\udfff"}"#,
        &[255],
    ] {
        fs::write(&file, bad).unwrap();
        let result = cli(&fixture(), &file, &output, None);
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty() && !output.exists());
    }
    let result = cli(&fixture(), &file, &output, Some("gdd/0.4"));
    assert_eq!(result.status.code(), Some(2));
    assert!(!output.exists());
}

#[cfg(unix)]
#[test]
fn cli_resolves_manifest_aliases_and_never_replaces_symlinks() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().unwrap();
    let alias = dir.path().join("index-alias");
    symlink(fixture().join("observations.json"), &alias).unwrap();
    let real_output = dir.path().join("out");
    fs::create_dir(&real_output).unwrap();
    let output_alias = dir.path().join("alias");
    symlink(&real_output, &output_alias).unwrap();
    let output = output_alias.join("new.json");
    let mapping = fixture().parent().unwrap().join("mapping.json");
    let result = cli(&alias, &mapping, &output, Some("gdd/0.2"));
    assert!(result.status.success());
    let proposal: Value = serde_json::from_slice(&result.stdout).unwrap();
    let uri = proposal["model_candidate"]["sources"]["source.graphify"]["uri"]
        .as_str()
        .unwrap();
    assert_eq!(
        real_output.join(uri).canonicalize().unwrap(),
        fixture().join(RAW)
    );
    fs::remove_file(&output).unwrap();
    symlink("missing", &output).unwrap();
    assert_eq!(cli(&alias, &mapping, &output, None).status.code(), Some(1));
    assert_eq!(output.read_link().unwrap(), Path::new("missing"));
    assert!(!real_output.join("missing").exists());
}
