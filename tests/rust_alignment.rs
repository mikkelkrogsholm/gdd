use gdd::graphify::{
    AST_PROFILE, GraphifyOptions, NODE_LINK_PROFILE,
    alignment::{align, align_with_previous},
    import_graphify,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn model() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/refinement.gdd.yaml")
}

fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn capture(dir: &Path, name: &str, input: &Value, profile: &str) -> PathBuf {
    let path = dir.join(format!("{name}.json"));
    let output = dir.join(name);
    write(&path, input);
    let opts = GraphifyOptions {
        profile: Some(profile),
        ..Default::default()
    };
    let report = import_graphify(&path, &output, opts).unwrap();
    assert_ne!(report.exit_code(), 1, "{report:?}");
    output
}

fn source() -> Value {
    serde_json::from_str(include_str!("../examples/graphify/raw.ast.json")).unwrap()
}

fn mapping() -> Value {
    json!({
        "format":"gdd-code-alignment/0.1",
        "project":"example.refinement", "source_id":"source.graphify",
        "bindings":[
            {"id":"site.module","targets":["component.checkout","action.checkout"],"selectors":[{"producer_id":"reservation_py"}]},
            {"id":"site.entry","targets":["action.charge"],"selectors":[{"producer_id":"reservation_reserve"}]},
            {"id":"site.storage","targets":["action.record"],"selectors":[
                {"symbol":{"file":"reservation.py","label":"Reservation"}},
                {"symbol":{"file":"store.py","label":"StoredReservation"}}
            ]},
            {"id":"site.next","targets":["action.record"],"selectors":[{"producer_id":"target_after_migration"}]}
        ],
        "relations":{"calls":"calls","references":"depends_on"}
    })
}

#[test]
fn matches_shared_and_distributed_code_and_reports_a_move_and_a_removed_dependency() {
    let dir = tempfile::tempdir().unwrap();
    let map = dir.path().join("align.json");
    write(&map, &mapping());
    let mut raw = source();
    let before = capture(dir.path(), "before", &raw, AST_PROFILE);
    raw["nodes"][1]["label"] = json!("StoredReservation");
    raw["nodes"][1]["source_file"] = json!("store.py");
    raw["nodes"].as_array_mut().unwrap().reverse();
    raw["edges"]
        .as_array_mut()
        .unwrap()
        .retain(|e| e["relation"] != "calls");
    let after = capture(dir.path(), "after", &raw, AST_PROFILE);
    let report = align_with_previous(&model(), &after, &map, Some(&before)).unwrap();
    let current = &report["current"];
    assert_ne!(
        current["observation_revision"],
        report["changes"]["previous_observation_revision"]
    );
    assert_eq!(
        current["ideal_nodes"]["action.checkout"]["state"],
        "observed"
    );
    assert_eq!(
        current["ideal_nodes"]["component.checkout"]["state"],
        "observed"
    );
    assert_eq!(
        current["ideal_nodes"]["action.record"]["state"],
        "partially_observed"
    );
    assert_eq!(
        current["ideal_nodes"]["entity.receipt"]["state"],
        "unmapped"
    );
    assert_eq!(current["bindings"]["site.storage"]["state"], "matched");
    assert_eq!(current["bindings"]["site.next"]["state"], "unobserved");
    assert_eq!(
        current["comparable_ideal_edges"]["edge.charge.calls_record"],
        json!([])
    );
    assert_eq!(
        current["comparable_ideal_edges"]["edge.charge.depends_record"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(current["observed_edges"].as_array().unwrap().len(), 4);
    let changes = &report["changes"]["bindings"];
    assert_eq!(changes.as_object().unwrap().len(), 1);
    assert_eq!(
        changes["site.storage"]["before"][0]["file"],
        "reservation.py"
    );
    assert_eq!(changes["site.storage"]["after"][0]["file"], "store.py");
    assert_eq!(
        report["changes"]["dependencies"],
        json!([{
            "dependency":{"from_bindings":["site.entry"],"relation":"calls","to_bindings":["site.storage"]},
            "before":1,"after":0
        }])
    );
}

#[test]
fn ambiguous_matches_and_unknown_edge_directions_remain_unresolved() {
    let dir = tempfile::tempdir().unwrap();
    let map = dir.path().join("map.json");
    let mut plan = mapping();
    plan["bindings"][2]["selectors"][1] = json!({"producer_id":"reservation_py"});
    write(&map, &plan);
    let raw: Value =
        serde_json::from_str(include_str!("../examples/graphify/export.undirected.json")).unwrap();
    let bundle = capture(dir.path(), "old", &raw, NODE_LINK_PROFILE);
    let report = align(&model(), &bundle, &map).unwrap();
    assert_eq!(
        report["current"]["bindings"]["site.storage"]["state"],
        "ambiguous"
    );
    assert_eq!(
        report["current"]["ideal_nodes"]["action.record"]["state"],
        "ambiguous"
    );
    assert!(
        report["current"]["observed_edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["state"] == "unknown_direction")
    );
    assert_eq!(
        report["current"]["comparable_ideal_edges"]["edge.charge.calls_record"],
        json!([])
    );
}

#[test]
fn mapping_cannot_silently_switch_project_or_source_or_use_tampered_observations() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = capture(dir.path(), "capture", &source(), AST_PROFILE);
    let path = dir.path().join("mapping.json");
    let mut invalid = mapping();
    invalid["project"] = "another.project".into();
    write(&path, &invalid);
    assert!(align(&model(), &bundle, &path).is_err());
    invalid = mapping();
    invalid["source_id"] = "source.different".into();
    write(&path, &invalid);
    assert!(align(&model(), &bundle, &path).is_err());
    invalid = mapping();
    invalid["bindings"][0]["targets"][0] = "action.typo".into();
    write(&path, &invalid);
    assert!(align(&model(), &bundle, &path).is_err());
    write(&path, &mapping());
    fs::write(bundle.join("graphify.raw.json"), b"{}").unwrap();
    assert!(align(&model(), &bundle, &path).is_err());
}
