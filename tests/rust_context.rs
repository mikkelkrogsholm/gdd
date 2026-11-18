use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use gdd::bundle::{self, Bundle};
use gdd::context::ContextGraph;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn edge(id: &str, from: &str, relation: &str, to: &str) -> Value {
    json!({"id": id, "from": from, "relation": relation, "to": to})
}

fn model(nodes: Value, edges: Vec<Value>) -> Value {
    json!({"format": "gdd/0.3", "nodes": nodes, "edges": edges,
        "project": {"id": "example.context", "title": "Context", "goal": "Derive scoped read context."}})
}

fn load(model: &Value) -> Bundle {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gdd.json");
    fs::write(&path, model.to_string()).unwrap();
    bundle::load(&path)
}

#[test]
fn initial_refinements_precede_global_rules_and_read_dependencies() {
    let nodes = json!({
        "parent":{"kind":"component"}, "child.a":{"kind":"action"}, "child.z":{"kind":"action"},
        "joined":{"kind":"action"}, "rule.child":{"kind":"rule"}, "community":{"kind":"community"},
        "scenario":{"kind":"scenario"}, "extension.work":{"kind":"example.work"},
        "rule.global":{"kind":"rule", "rule":{"scope":"global", "constraint":{"to":"global.target"}}}
    });
    let edges = vec![
        edge("edge.second", "child.a", "refines", "parent"),
        edge("edge.first", "child.z", "refines", "parent"),
        edge("edge.a", "child.a", "refines", "child.z"),
        edge("edge.b", "child.z", "refines", "child.a"),
        edge("edge.join.a", "child.a", "calls", "joined"),
        edge("edge.join.z", "child.z", "calls", "joined"),
        edge("edge.back", "joined", "calls", "parent"),
        edge("edge.support", "rule.child", "refines", "parent"),
        edge("edge.community", "community", "refines", "rule.child"),
        edge("edge.scenario", "scenario", "refines", "community"),
        edge("edge.custom", "extension.work", "refines", "scenario"),
        edge("edge.unrelated", "sibling", "refines", "joined"),
    ];
    let bundle = load(&model(nodes, edges));
    let graph = ContextGraph::new(&bundle);
    let scope = graph.scope(["parent", "parent"]);
    assert_eq!(
        scope.implementation_targets,
        BTreeSet::from(["parent", "child.a", "child.z", "extension.work"])
    );
    assert_eq!(
        scope.nodes().collect::<Vec<_>>(),
        [
            "child.a",
            "child.z",
            "community",
            "extension.work",
            "global.target",
            "joined",
            "parent",
            "rule.child",
            "rule.global",
            "scenario"
        ]
    );
    assert_eq!(scope.path_to("parent").unwrap(), []);
    assert!(scope.path_to("sibling").is_none());
    let path = scope.path_to("joined").unwrap();
    assert_eq!(
        serde_json::to_value(path).unwrap(),
        json!([
            {"from":"parent", "to":"child.z", "edge":"edge.first", "relation":"refines", "orientation":"reverse", "derivation":"initial-refinement"},
            {"from":"child.z", "to":"joined", "edge":"edge.join.z", "relation":"calls", "orientation":"forward", "derivation":"declared-context"}
        ])
    );
    let with_support = graph.scope(["parent", "community", "unknown"]);
    assert!(with_support.implementation_targets.contains("community"));
    assert!(with_support.implementation_targets.contains("unknown"));
    assert_eq!(
        with_support.path_to("global.target").unwrap()[0].from,
        Some("community")
    );
    let empty = graph.scope([]);
    assert_eq!(
        empty.nodes().collect::<Vec<_>>(),
        ["global.target", "rule.global"]
    );
    assert!(empty.implementation_targets.is_empty());
    assert_eq!(
        serde_json::to_value(empty.path_to("rule.global")).unwrap(),
        json!([
            {"from":null, "to":"rule.global", "edge":null, "relation":null, "orientation":"semantic", "derivation":"global-rule-context"}
        ])
    );
}

#[test]
fn only_declared_semantic_fields_contribute_links() {
    let reference = json!({"type":"reference","target":"entity.item"});
    let object = json!({"type":"object","properties":{"a":reference,"z":{"type":"array","items":reference}}});
    let nodes = json!({
        "root":{
            "interfaces":["public"],
            "fields":{"obj":object, "primitive":{"type":"string","target":"ignored"}},
            "contract":{
                "input":{"x":reference}, "context":{"x":reference}, "output":reference,
                "risks":{"authorization":{"scenarios":["scenario"]}}, "error_cases":{"bad":["scenario"]}
            },
            "machine":{"transitions":[{"trigger":"trigger"}]},
            "rule":{"constraint":{"from":"left","to":"right"}},
            "data":{"type":"reference","target":"ignored"}
        }
    });
    let bundle = load(&model(
        nodes,
        vec![edge("edge.scenario", "root", "calls", "scenario")],
    ));
    let graph = ContextGraph::new(&bundle);
    assert_eq!(
        graph
            .links()
            .map(|l| (l.to, l.derivation))
            .collect::<Vec<_>>(),
        [
            ("entity.item", "type-reference"),
            ("entity.item", "type-reference"),
            ("entity.item", "type-reference"),
            ("entity.item", "type-reference"),
            ("left", "rule-constraint-endpoint"),
            ("public", "public-interface"),
            ("right", "rule-constraint-endpoint"),
            ("scenario", "contract-error-scenario"),
            ("scenario", "contract-risk-scenario"),
            ("scenario", "declared-context"),
            ("trigger", "machine-trigger"),
        ]
    );
    assert_eq!(
        graph.scope(["root"]).path_to("scenario").unwrap()[0].derivation,
        "contract-error-scenario"
    );
}

#[test]
fn core_relation_directions_and_versioned_community_membership() {
    let both = ["constrains", "exercises", "verifies", "state_of"];
    let forward = [
        "owned_by",
        "stored_in",
        "imports",
        "calls",
        "reads",
        "creates",
        "changes",
        "navigates_to",
        "depends_on",
        "refines",
        "member_of",
    ];
    let edges = both
        .iter()
        .chain(&forward)
        .map(|r| edge(r, "source", r, r))
        .collect();
    let mut manifest = model(json!({}), edges);
    for version in ["gdd/0.2", "gdd/0.3"] {
        manifest["format"] = json!(version);
        let bundle = load(&manifest);
        let graph = ContextGraph::new(&bundle);
        let links: Vec<_> = graph.links().collect();
        for relation in both.iter().chain(&forward) {
            assert_eq!(
                links
                    .iter()
                    .any(|l| l.relation == Some(relation) && l.orientation == "forward"),
                *relation != "member_of" || version == "gdd/0.3"
            );
            assert_eq!(
                links
                    .iter()
                    .any(|l| l.relation == Some(relation) && l.orientation == "reverse"),
                both.contains(relation)
            );
        }
    }
}

#[test]
fn extension_context_directions_dependency_and_effect_are_independent() {
    let dir = tempfile::tempdir().unwrap();
    let policies = [
        ("none", false, "none", false, false),
        ("incoming", false, "none", false, true),
        ("outgoing", false, "none", true, false),
        ("both", false, "none", true, true),
        ("none", true, "none", true, false),
        ("incoming", false, "read", true, true),
    ];
    let mut ext = json!({"format":"gdd-extension/0.2","namespace":"custom","version":"1.0","relations":{}, "node_kinds":{}});
    let mut edges = Vec::new();
    for (index, (direction, dependency, effect, _, _)) in policies.iter().enumerate() {
        let relation = format!("custom.r{index}");
        ext["relations"][&relation] = json!({"context":direction, "dependency":dependency, "effect":effect,
            "description":"Read context.", "from":["action"], "to":["action"], "data_schema":{"type":"object"}});
        edges.push(edge(
            &format!("edge.e{index}"),
            "root",
            &relation,
            &relation,
        ));
    }
    let bytes = serde_json::to_vec(&ext).unwrap();
    let mut manifest = model(json!({}), edges);
    manifest["extensions"] = json!({"extension.custom":{"namespace":"custom", "version":"1.0","path":"ext.json",
        "sha256": format!("{:x}", Sha256::digest(&bytes))}});
    fs::write(dir.path().join("ext.json"), bytes).unwrap();
    let path = dir.path().join("gdd.json");
    fs::write(&path, manifest.to_string()).unwrap();
    let bundle = bundle::load(&path);
    let graph = ContextGraph::new(&bundle);
    let root = graph.scope(["root"]);
    for (i, (_, _, _, forward, reverse)) in policies.iter().enumerate() {
        let target = format!("custom.r{i}");
        assert_eq!(root.contains(&target), *forward, "{target}");
        assert_eq!(
            graph.scope([target.as_str()]).contains("root"),
            *reverse,
            "{target}"
        );
    }
}

fn cli(path: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gdd"))
        .arg("context")
        .arg(path)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn cli_projects_fragments_and_preserves_existing_outputs_and_validation_results() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/fragmented.gdd.yaml");
    let out = cli(&path, &["--scope", "entity.note", "--scope", "entity.note"]);
    assert!(out.status.success());
    let manifest: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        manifest,
        json!({
            "format":"gdd-context/0.2","spec_revision":bundle::load(&path).snapshot().unwrap().revision,
            "scope":["entity.note"],"nodes":["component.notes","entity.note"],"edges":["edge.note.owner"],
            "sources":[], "questions":[], "extensions":[]
        })
    );
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("result.json");
    let args = [
        "--scope",
        "component.notes",
        "--output",
        output.to_str().unwrap(),
    ];
    assert!(cli(&path, &args).status.success());
    let first = fs::read(&output).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&first).unwrap()["nodes"],
        json!(["component.notes"])
    );
    assert!(!cli(&path, &args).status.success());
    assert_eq!(fs::read(output).unwrap(), first);
    assert_eq!(cli(&path, &[]).status.code(), Some(2));
    let unknown = cli(&path, &["--scope", "unknown"]);
    assert_eq!(unknown.status.code(), Some(1));
    assert!(unknown.stdout.is_empty());
    assert_eq!(unknown.stderr, b"gdd: Unknown context scope.\n");
    let invalid = dir.path().join("invalid.json");
    for (input, code) in [(r#"{"format":"gdd/0.4"}"#, 2), ("{", 1)] {
        fs::write(&invalid, input).unwrap();
        let failed = cli(&invalid, &["--scope", "unknown"]);
        assert_eq!(failed.status.code(), Some(code));
        assert_eq!(
            serde_json::from_slice::<Value>(&failed.stdout).unwrap()["format"],
            json!("gdd-report/0.2")
        );
    }
}

#[test]
fn cli_collects_induced_edges_sources_ancestors_and_intersecting_questions() {
    let nodes = json!({
        "screen":{"kind":"screen", "description":"Checkout.",
            "source_refs":[{"source":"source.node", "purpose":"provenance"}],
            "design_refs":[{"source":"source.design", "purpose":"contract"}]},
        "owner":{"kind":"component", "description":"Checkout owner."},
        "community":{"kind":"community","description":"Selected independently."},
        "outside":{"kind":"component", "description":"Outside the scope.",
            "source_refs":[{"source":"source.outside","purpose":"provenance"}]}
    });
    // A 0.2 member_of edge is induced when both endpoints are selected.
    let mut membership = edge("edge.member", "owner", "member_of", "community");
    membership["source_refs"] = json!([{"source":"source.edge","purpose":"provenance"}]);
    let mut manifest = model(
        nodes,
        vec![
            membership,
            edge("edge.owner", "screen", "owned_by", "owner"),
        ],
    );
    manifest["format"] = json!("gdd/0.2");
    let revision = json!({"producer_revision":"edition.1"});
    for name in ["node", "design", "edge", "parent", "ancestor", "outside"] {
        manifest["sources"][format!("source.{name}")] = json!({"role":"knowledge","title":name,
            "uri":format!("{name}.md"),"format":"text/markdown","normative":false,"revision":revision});
    }
    manifest["sources"]["source.edge"]["derived_from"] =
        json!([{"source":"source.parent","revision":revision}]);
    manifest["sources"]["source.parent"]["derived_from"] =
        json!([{"source":"source.ancestor","revision":revision}]);
    for (id, target) in [("question.kept", "owner"), ("question.ignored", "outside")] {
        manifest["questions"][id] = json!({"statement":"Confirm the scope.", "targets":[target], "status":"open", "blocks":["modeling"]});
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gdd.json");
    fs::write(&path, manifest.to_string()).unwrap();
    let out = cli(&path, &["--scope", "screen", "--scope", "community"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["scope"], json!(["community", "screen"]));
    assert_eq!(result["edges"], json!(["edge.member", "edge.owner"]));
    assert_eq!(
        result["sources"],
        json!([
            "source.ancestor",
            "source.design",
            "source.edge",
            "source.node",
            "source.parent"
        ])
    );
    assert_eq!(result["questions"], json!(["question.kept"]));

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/extension.gdd.yaml");
    let out = cli(&path, &["--scope", "entity.room"]);
    assert!(out.status.success());
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["nodes"], json!(["entity.room"]));
    assert_eq!(result["extensions"], json!(["extension.sensors"]));
}
