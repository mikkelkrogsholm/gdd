use gdd::graphify::{
    AST_PROFILE, GraphifyOptions, ImportReport, NODE_LINK_PROFILE, compact_observations,
    observe_graphify,
};
use gdd::report::CheckResult;
use serde_json::{Value, json};

fn observe(raw: &[u8], profile: &str) -> (Value, ImportReport) {
    observe_graphify(
        raw,
        GraphifyOptions {
            profile: Some(profile),
            ..Default::default()
        },
    )
    .unwrap()
}

fn check(report: &ImportReport, name: &str) -> CheckResult {
    report
        .checks
        .iter()
        .find(|c| c.check_id == format!("graphify.{name}"))
        .unwrap()
        .result
}

fn codes(report: &ImportReport) -> Vec<&str> {
    report
        .checks
        .iter()
        .flat_map(|check| check.diagnostics.iter().map(|d| d.code))
        .collect()
}

#[test]
fn actual_ast_projection_matches_archived_observations_and_checks() {
    let raw = include_bytes!("../examples/graphify/raw.ast.json");
    let mut archived: Value = serde_json::from_slice(include_bytes!(
        "../examples/graphify/observations/observations.json"
    ))
    .unwrap();
    let (index, report) = observe_graphify(
        raw,
        GraphifyOptions {
            profile: Some(AST_PROFILE),
            producer_version: Some("0.9.75"),
            locator: archived["input_claims"]["locator"].as_str().unwrap(),
            ..Default::default()
        },
    )
    .unwrap();
    // The archive predates the adapter's depth handling in 0.1.2.
    archived["importer"]["version"] = "0.1.2".into();
    assert_eq!(index, archived);
    let mut expected: Value = serde_json::from_slice(include_bytes!(
        "../examples/graphify/observations/import.report.json"
    ))
    .unwrap();
    expected["tool"]["version"] = "0.1.2".into();
    expected["checks"].as_array_mut().unwrap().retain(|c| {
        !matches!(
            c["check_id"].as_str(),
            Some("graphify.integrity" | "graphify.projection")
        )
    });
    assert_eq!(serde_json::to_value(&report).unwrap(), expected);
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn exported_fixtures_preserve_stored_direction_and_edge_count() {
    for (raw, count, exit) in [
        (
            &include_bytes!("../examples/graphify/export.directed.json")[..],
            5,
            0,
        ),
        (
            &include_bytes!("../examples/graphify/export.undirected.json")[..],
            5,
            2,
        ),
        (
            &include_bytes!("../examples/graphify/export.collapsed.json")[..],
            3,
            2,
        ),
    ] {
        let (index, report) = observe(raw, NODE_LINK_PROFILE);
        assert_eq!(index["edges"].as_array().unwrap().len(), count);
        assert_eq!(report.exit_code(), exit);
        let original: Value = serde_json::from_slice(raw).unwrap();
        for (n, edge) in index["edges"].as_array().unwrap().iter().enumerate() {
            assert_eq!(edge["raw"], original["links"][n]);
            assert_eq!(edge["identity"]["pointer"], format!("/links/{n}"));
        }
    }
}

#[test]
fn marker_pairs_and_opaque_claims_survive_projection() {
    let mut data = json!({
        "directed":false, "multigraph":true, "graph":{"code_revision":"old","hyperedges":[false]},
        "nodes":[{"id":"1","file_type":"document","source_file":"note.py","community_name":"a"},
                 {"id":1,"file_type":"code","source_file":"lib.rs","source_location":{"<<":1}}],
        "links":[{"id":"dup","relation":"calls","source":"1","target":1,"_src":1,"_tgt":"1"},
                 {"id":"dup","relation":"calls","source":"1","target":1,"_src":1,"_tgt":"1"}],
        "communities":null, "code_revision":"new"
    });
    let (index, report) = observe(&serde_json::to_vec(&data).unwrap(), NODE_LINK_PROFILE);
    assert_eq!(report.exit_code(), 2);
    assert_eq!(codes(&report), ["REVISION_UNRESOLVED"]);
    assert_eq!(index["nodes"][0]["classification"], "noncode");
    assert_eq!(
        index["edges"][0]["source_observation"],
        index["nodes"][1]["id"]
    );
    assert_ne!(index["edges"][0]["id"], index["edges"][1]["id"]);
    assert_eq!(index["edges"][1]["direction"]["basis"], "/links/1/_src");
    assert_eq!(
        index["binding_candidates"][1]["selector_claim"],
        json!({"<<":1})
    );
    assert_eq!(
        index["opaque"][0]["identity"]["pointer"],
        "/nodes/0/community_name"
    );
    assert_eq!(
        index["opaque"][1]["identity"]["pointer"],
        "/graph/hyperedges"
    );
    assert_eq!(index["opaque"][2]["identity"]["pointer"], "/communities");
    data["directed"] = true.into();
    let (index, report) = observe(&serde_json::to_vec(&data).unwrap(), NODE_LINK_PROFILE);
    assert_eq!(check(&report, "edges"), CheckResult::Fail);
    assert_eq!(index["edges"][0]["source_observation"], Value::Null);
    assert_eq!(index["edges"][0]["direction"]["value"], "unknown");
}

fn graph_with_ids(ids: &[&str], source: &str, target: &str) -> String {
    let nodes = ids
        .iter()
        .map(|id| format!(r#"{{"id":{id},"file_type":"code","source_file":"x.py"}}"#))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"directed":true,"multigraph":true,"graph":{{}},"nodes":[{nodes}],"links":[{{"source":{source},"target":{target},"relation":"calls"}}]}}"#
    )
}

#[test]
fn exact_numeric_references_remain_separate_from_text_and_rounding() {
    let big_exp = "100000000000000000000001";
    let significant = format!("2e{big_exp}");
    let scaled = "20e100000000000000000000000";
    for (ids, source, target) in [
        (vec!["0", "1e-4000"], "-0.0", "10e-4001"),
        (
            vec!["9007199254740992", "9007199254740993.0"],
            "9007199254740992e0",
            "9007199254740993",
        ),
        (vec![&significant, r#""2""#], scaled, r#""2""#),
    ] {
        let (index, report) = observe(
            graph_with_ids(&ids, source, target).as_bytes(),
            NODE_LINK_PROFILE,
        );
        assert_eq!(report.exit_code(), 0);
        assert_eq!(
            index["edges"][0]["source_observation"],
            index["nodes"][0]["id"]
        );
        assert_eq!(
            index["edges"][0]["target_observation"],
            index["nodes"][1]["id"]
        );
    }
    for ids in [&["0", "-0.0"], &["1.00", "1e0"]] {
        let (index, report) = observe(
            graph_with_ids(ids, ids[0], ids[1]).as_bytes(),
            NODE_LINK_PROFILE,
        );
        assert_eq!(report.exit_code(), 1);
        assert_eq!(
            codes(&report),
            [
                "ID_COLLISION",
                "REFERENCE_UNRESOLVED",
                "REFERENCE_UNRESOLVED"
            ]
        );
        assert!(index["edges"][0]["source_observation"].is_null());
    }
}

fn deep(depth: usize, leaf: &str) -> String {
    let arrays = depth - 3;
    format!(
        r#"{{"nodes":[{{"id":1,"file_type":"code","source_file":"x.py","label":"a","extra":{}{leaf}{}}}],"edges":[]}}"#,
        "[".repeat(arrays),
        "]".repeat(arrays)
    )
}

#[test]
fn depth_cap_applies_to_the_raw_object_and_all_syntax_is_checked() {
    for depth in [255, 256, 257, 1200, 12000] {
        let raw = deep(depth, "0");
        let (index, report) = observe(raw.as_bytes(), AST_PROFILE);
        if depth <= 256 {
            assert_eq!(report.exit_code(), 0);
            assert_eq!(index["nodes"].as_array().unwrap().len(), 1);
            assert!(serde_json::to_vec(&index).is_ok());
        } else {
            assert_eq!(report.exit_code(), 2);
            assert_eq!(check(&report, "depth"), CheckResult::Unknown);
            assert_eq!(index, compact_observations(&index));
            for invalid in [
                raw[..raw.len() - 1].to_owned(),
                deep(depth, r#"{"a":0,"\u0061":1}"#),
                deep(depth, r#""\uD800""#),
                deep(depth, "NaN"),
            ] {
                let (_, report) = observe(invalid.as_bytes(), AST_PROFILE);
                assert_eq!(check(&report, "json"), CheckResult::Fail);
            }
        }
    }
    let (_, report) = observe(
        format!("{}0{}", "[".repeat(300), "]".repeat(300)).as_bytes(),
        AST_PROFILE,
    );
    assert_eq!(check(&report, "layout"), CheckResult::Fail);
}

#[test]
fn admission_and_schema_errors_remain_distinct() {
    for malformed in [
        &b"nodes: []"[..],
        b"[]0",
        b"{\"a\": NaN}",
        b"{\"x\":01}",
        b"\"\xff\"",
        b"",
        b"null\nnull",
        br#"{"x":"\udc00"}"#,
    ] {
        let (index, report) = observe(malformed, AST_PROFILE);
        assert_eq!(check(&report, "json"), CheckResult::Fail);
        assert_eq!(index["artifact"]["byte_count"], malformed.len());
    }
    let (_, report) = observe(b"{}", NODE_LINK_PROFILE);
    assert_eq!(codes(&report), ["SCHEMA_INVALID", "SCHEMA_INVALID"]);
    let (index, report) = observe(
        br#"{"nodes":[],"edges":[],"links":[],"graph":{"built_at_commit":4}}"#,
        AST_PROFILE,
    );
    assert_eq!(check(&report, "layout"), CheckResult::Unknown);
    assert_eq!(index["producer_claims"]["/graph/built_at_commit"], 4);
    let (index, report) = observe(br#"{"format":false,"nodes":[],"edges":[]}"#, AST_PROFILE);
    assert_eq!(report.exit_code(), 2);
    assert_eq!(index["producer_claims"]["/format"], false);
    let (index, report) = observe_graphify(b"uninterpreted", Default::default()).unwrap();
    assert_eq!(check(&report, "layout"), CheckResult::Unknown);
    assert_eq!(index, compact_observations(&index));
    assert!(
        observe_graphify(
            b"{}",
            GraphifyOptions {
                source_id: "bad/id",
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn producer_objects_keep_json_keys_and_selector_at_maximum_depth() {
    let field = r#"{"$serde_json::private::Number":{"<<":"01"}}"#;
    let selector = format!("{}{field}{}", r#"{"next":"#.repeat(251), "}".repeat(251));
    let raw = format!(
        r#"{{"nodes":[{{"id":"a","label":"A","source_file":"src.rs","file_type":"code","source_location":{selector}}}],"edges":[],"hyperedges":{field}}}"#
    );
    let (index, report) = observe(raw.as_bytes(), AST_PROFILE);
    assert_eq!(report.exit_code(), 0);
    assert_eq!(
        index["binding_candidates"][0]["selector_claim"],
        index["nodes"][0]["raw"]["source_location"]
    );
    assert_eq!(
        index["opaque"][0]["raw"]["$serde_json::private::Number"]["<<"],
        "01"
    );
}

#[test]
fn invalid_nodes_and_references_are_retained_without_binding_candidates() {
    let (index, report) = observe(
        br#"{"nodes":[null,{"id":false,"file_type":"concept","source_file":"app.py"},{"id":"ok","file_type":"code","source_file":".rs"}],"edges":[{"source":false,"target":"ok","relation":"calls","source_file":"lib.rs","confidence":"EXTRACTED"}]}"#,
        AST_PROFILE,
    );
    assert_eq!(report.exit_code(), 1);
    assert_eq!(check(&report, "nodes"), CheckResult::Fail);
    assert_eq!(check(&report, "edges"), CheckResult::Fail);
    assert_eq!(check(&report, "code_candidates"), CheckResult::Unknown);
    assert_eq!(index["nodes"].as_array().unwrap().len(), 3);
    assert_eq!(index["nodes"][0]["raw"], Value::Null);
    assert_eq!(index["nodes"][1]["producer_id"], false);
    assert_eq!(index["nodes"][1]["classification"], "noncode");
    assert_eq!(index["nodes"][2]["classification"], "unknown");
    assert_eq!(index["binding_candidates"].as_array().unwrap().len(), 1);
    assert_eq!(index["edges"][0]["source_observation"], Value::Null);
    assert_eq!(
        index["edges"][0]["target_observation"],
        index["nodes"][2]["id"]
    );
}
