use gdd::bundle;
use gdd::report::{CheckResult, Report};
use gdd::validation::{Profile, ValidationOptions, validate, validate_with_options};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

fn cli(path: &Path, scope: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gdd"))
        .arg("validate")
        .arg(path)
        .args(["--profile", "readiness", "--scope", scope])
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn tutorial_refinement_has_a_ready_revision_bound_action_scope() {
    let work = tempfile::tempdir().unwrap();
    let run = |args: &[&str], expected: i32| -> Value {
        let output = Command::new(env!("CARGO_BIN_EXE_gdd"))
            .current_dir(work.path())
            .args(args)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(expected),
            "{args:?}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    };
    let draft = run(
        &[
            "init",
            "draft",
            "--project-id",
            "project.integer-addition",
            "--title",
            "Local integer addition",
        ],
        0,
    );
    let sketch = work.path().join("draft/gdd.json");
    let original = fs::read(&sketch).unwrap();
    run(&["validate", "draft/gdd.json", "--profile", "model"], 0);
    run(
        &[
            "validate",
            "draft/gdd.json",
            "--profile",
            "readiness",
            "--scope",
            "action.main",
        ],
        1,
    );

    let model = "refined.gdd.json";
    let refined = include_bytes!("../docs/tutorial/refined.gdd.json");
    fs::write(work.path().join(model), refined).unwrap();
    let ready = run(
        &[
            "validate",
            model,
            "--profile",
            "readiness",
            "--scope",
            "action.add",
        ],
        0,
    );
    let diff = run(&["diff", "draft/gdd.json", model], 0);
    let packet = run(&["context", model, "--scope", "action.add"], 0);
    assert_eq!(diff["base"]["spec_revision"], draft["spec_revision"]);
    assert_eq!(diff["comparison"]["semantic_changes"], true);
    assert_ne!(diff["base"]["spec_revision"], diff["head"]["spec_revision"]);
    assert_eq!(diff["head"]["spec_revision"], ready["spec_revision"]);
    assert_eq!(packet["spec_revision"], ready["spec_revision"]);
    assert_eq!(packet["scope"], json!(["action.add"]));
    assert_eq!(
        packet["nodes"],
        json!(["action.add", "component.arithmetic", "scenario.add"])
    );
    assert_eq!(
        packet["edges"],
        json!(["edge.add.owner", "edge.add.scenario"])
    );
    assert_eq!(fs::read(sketch).unwrap(), original);
    assert_eq!(fs::read(work.path().join(model)).unwrap(), refined);
}

#[test]
fn scoped_component_reads_its_public_action_from_the_owning_fragment() {
    let directory = tempfile::tempdir().unwrap();
    let manifest = directory.path().join("gdd.json");
    let fragment = directory.path().join("part.json");
    let mut model: Value =
        serde_json::from_str(include_str!("../docs/tutorial/refined.gdd.json")).unwrap();
    let mut part = json!({"format":"gdd-fragment/0.3", "nodes":model["nodes"].take(), "edges":model["edges"].take()});
    model["nodes"] = json!({});
    model["edges"] = json!([]);
    model["includes"] = json!(["part.json"]);
    let action = part["nodes"]["action.add"].as_object_mut().unwrap();
    let behavior = action.remove("behavior").unwrap();
    write(&manifest, &model);
    write(&fragment, &part);
    let scope = ["component.arithmetic".to_owned()];
    let validate_scope = |bundle: &bundle::Bundle| {
        validate_with_options(
            bundle,
            ValidationOptions {
                profiles: &[Profile::Readiness],
                scope: &scope,
                ..Default::default()
            },
        )
        .unwrap()
    };
    let original = bundle::load(&manifest);
    let report = validate_scope(&original);
    assert_eq!(report.result("model"), CheckResult::Pass);
    assert_eq!(report.exit_code(), 1);
    let checks: Vec<_> = report
        .checks
        .iter()
        .filter(|c| c.dimension == "readiness")
        .collect();
    assert_eq!(checks.len(), 1);
    let diagnostic = &checks[0].diagnostics[0];
    assert_eq!(diagnostic.code, "READINESS_CONTRACT_MISSING");
    assert_eq!(diagnostic.location.file.as_deref(), Some("part.json"));
    assert_eq!(diagnostic.location.pointer, "/nodes/action.add");
    let plain = cli(&manifest, "component.arithmetic", &[]);
    let explained = cli(&manifest, "component.arithmetic", &["--explain"]);
    assert!(plain.stderr.is_empty());
    assert_eq!(plain.stdout, explained.stdout);
    assert_eq!(explained.status.code(), Some(1));
    let explanation = String::from_utf8(explained.stderr).unwrap();
    assert!(explanation.contains("READINESS_CONTRACT_MISSING part.json#/nodes/action.add"));
    assert!(explanation.contains("component.arithmetic -> action.add (public-interface)"));
    part["nodes"]["action.add"]["behavior"] = behavior;
    write(&fragment, &part);
    let repaired = validate_scope(&bundle::load(&manifest));
    let explained = cli(&manifest, "component.arithmetic", &["--explain"]);
    assert_eq!(explained.status.code(), Some(0));
    assert!(explained.stderr.is_empty());
    assert_eq!(
        explained.stdout,
        cli(&manifest, "component.arithmetic", &[]).stdout
    );
    assert_eq!(repaired.exit_code(), 0);
    assert_ne!(report.spec_revision, repaired.spec_revision);
    assert_eq!(validate_scope(&original).exit_code(), 1);
}

fn source_model(
    directory: &Path,
    bytes: &[u8],
    format: &str,
    selector: Value,
    normative: bool,
) -> bundle::Bundle {
    let path = directory.join("source");
    fs::write(path, bytes).unwrap();
    let model = json!({
        "format":"gdd/0.2", "project":{"id":"project.test","title":"Test","goal":"Read a contract."},
        "nodes":{"entity.x":{"kind":"entity","description":"Entity","fields":{},
            "source_refs":[{"source":"source.x","purpose":"contract","selector":selector}]}}, "edges":[],
        "sources":{"source.x":{"title":"Contract","role":"input","uri":"source","format":format,"normative":normative,
            "revision":{"sha256":format!("{:x}",Sha256::digest(bytes))}}}
    });
    let manifest = directory.join("gdd.json");
    write(&manifest, &model);
    bundle::load(&manifest)
}

fn diagnostics(report: &Report) -> Vec<&str> {
    report
        .checks
        .iter()
        .filter(|c| c.dimension == "readiness")
        .flat_map(|c| &c.diagnostics)
        .map(|d| d.code)
        .collect()
}

#[test]
fn verified_source_selectors_use_captured_bytes_and_declared_format() {
    let directory = tempfile::tempdir().unwrap();
    let yaml = b"'a/b': ['present']";
    let selector = json!({"kind":"json-pointer","value":"/a~1b/0"});
    let valid = source_model(directory.path(), yaml, "yaml", selector.clone(), true);
    fs::remove_file(directory.path().join("source")).unwrap();
    let report = validate(&valid, Profile::Readiness);
    assert_eq!(report.exit_code(), 0);
    assert_eq!(
        report.spec_revision.as_ref(),
        valid.snapshot().map(|s| &s.revision)
    );
    let invalid_json = source_model(directory.path(), yaml, "json", selector.clone(), true);
    assert_eq!(
        diagnostics(&validate(&invalid_json, Profile::Readiness)),
        ["SOURCE_UNAVAILABLE"]
    );
    let too_deep = format!("{}0{}", "[".repeat(257), "]".repeat(257));
    let limited = source_model(
        directory.path(),
        too_deep.as_bytes(),
        "json",
        selector,
        true,
    );
    assert_eq!(
        diagnostics(&validate(&limited, Profile::Readiness)),
        ["FORMAT_UNSUPPORTED"]
    );
}

#[test]
fn a_readable_advisory_contract_is_captured_outside_the_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let bundle = source_model(
        directory.path(),
        b"one\r\ntwo\n",
        "text",
        json!({"kind":"line-range","value":"L1-L2"}),
        false,
    );
    assert_eq!(bundle.snapshot().unwrap().assets.len(), 1);
    fs::remove_file(directory.path().join("source")).unwrap();
    assert_eq!(
        diagnostics(&validate(&bundle, Profile::Readiness)),
        ["CONTRACT_SOURCE_UNPINNED", "CONTRACT_SOURCE_UNPINNED"]
    );
    let missing = bundle::load(&directory.path().join("gdd.json"));
    assert_eq!(
        diagnostics(&validate(&missing, Profile::Readiness)),
        ["CONTRACT_SOURCE_UNPINNED", "SOURCE_UNAVAILABLE"]
    );
}

#[test]
fn trusted_question_decision_is_retired_only_by_an_applicable_trusted_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("gdd.json");
    let ledger = dir.path().join("records.json");
    write(
        &manifest,
        &json!({
            "format":"gdd/0.2", "project":{"id":"project.test","title":"Test","goal":"Resolve a question."},
            "nodes":{"entity.x":{"kind":"entity","description":"Entity","fields":{}}}, "edges":[],
            "record_files":["records.json"], "questions":{"question.x":{"statement":"Choose a value.",
            "targets":["entity.x"],"blocks":["implementation"],"status":"resolved","resolution":{"decision":"decision.review"}}}
        }),
    );
    let mut records = json!({"format":"gdd-records/0.2","project_id":"project.test","records":{}});
    write(&ledger, &records);
    let revision = bundle::load(&manifest).snapshot().unwrap().revision.clone();
    let decision = json!({"kind":"decision","topic":"model_change","scope":["entity.x"],"spec_revision":revision,
        "policy_revision":"policy.1","actor":{"id":"actor.reviewer","kind":"human"},"outcome":"accepted","rationale":"Reviewed."});
    records["records"]["decision.review"] = decision.clone();
    let mut replacement = decision;
    replacement["supersedes"] = json!("decision.review");
    replacement["outcome"] = json!("rejected");
    records["records"]["decision.replacement"] = replacement;
    write(&ledger, &records);
    let assess = |trusted: &[&str]| {
        let host =
            gdd::input::parse(&serde_json::to_vec(&json!({"trusted_decisions":trusted})).unwrap())
                .unwrap();
        validate_with_options(
            &bundle::load(&manifest),
            ValidationOptions {
                profiles: &[Profile::Readiness],
                host_context: Some(&host),
                ..Default::default()
            },
        )
        .unwrap()
        .result("readiness")
    };
    assert_eq!(assess(&[]), CheckResult::Unknown);
    assert_eq!(assess(&["decision.review"]), CheckResult::Pass);
    assert_eq!(
        assess(&["decision.review", "decision.replacement"]),
        CheckResult::Unknown
    );
    records["records"]["decision.replacement"]["policy_revision"] = json!("policy.other");
    write(&ledger, &records);
    assert_eq!(
        assess(&["decision.review", "decision.replacement"]),
        CheckResult::Pass
    );
}

#[test]
fn scoped_and_model_only_loaders_skip_unrelated_advisory_contracts() {
    let dir = tempfile::tempdir().unwrap();
    source_model(
        dir.path(),
        b"contract",
        "text",
        json!({"kind":"whole-document"}),
        false,
    );
    let path = dir.path().join("gdd.json");
    let mut model: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    model["nodes"]["entity.other"] = json!({"kind":"entity","description":"Other","fields":{}});
    write(&path, &model);
    let other = ["entity.other".to_owned()];
    let captured = bundle::load_for_readiness(&path, &other);
    let limited = bundle::load_model_only(&path);
    let report = validate_with_options(
        &captured,
        ValidationOptions {
            profiles: &[Profile::Readiness],
            scope: &other,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(report.exit_code(), 0);
    for bundle in [&captured, &limited] {
        assert_eq!(
            diagnostics(&validate(bundle, Profile::Readiness)),
            ["CONTRACT_SOURCE_UNPINNED", "SOURCE_UNAVAILABLE"]
        );
    }
}

#[test]
fn optional_host_input_preserves_absence_and_distinguishes_capacity_from_invalid_data() {
    let directory = tempfile::tempdir().unwrap();
    let host = directory.path().join("host.json");
    let model = Path::new("docs/tutorial/refined.gdd.json");
    let no_host = cli(model, "action.add", &[]);
    assert!(no_host.status.success());
    for absent in [
        "null",
        "",
        "  \n  ",
        "# empty host\n",
        "---\n# empty document\n...\n",
    ] {
        fs::write(&host, absent).unwrap();
        let output = cli(
            model,
            "action.add",
            &["--host-context", host.to_str().unwrap()],
        );
        assert!(output.status.success());
        assert_eq!(output.stdout, no_host.stdout);
        assert!(output.stderr.is_empty());
    }
    for (input, code) in [
        (format!("{}0{}", "[".repeat(1800), "]".repeat(1800)), 2),
        ("{\"trusted_decisions\":false}".to_owned(), 1),
        ("false".to_owned(), 1),
        ("[]".to_owned(), 1),
        ("[unterminated".to_owned(), 1),
    ] {
        fs::write(&host, input).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_gdd"))
            .args([
                "validate",
                "docs/tutorial/refined.gdd.json",
                "--profile",
                "readiness",
                "--host-context",
            ])
            .arg(&host)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn explains_reached_question_targets_and_edge_endpoints_without_treating_them_as_nodes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("gdd.json");
    let mut model: Value =
        serde_json::from_str(include_str!("../docs/tutorial/refined.gdd.json")).unwrap();
    model["nodes"]["component.parent"] = json!({"kind":"component","description":"Arithmetic domain",
        "interfaces":[],"boundary":{"responsibility":"Provide arithmetic.","rationale":"One domain.","excludes":[]}});
    model["edges"].as_array_mut().unwrap().push(json!({
        "id":"edge.arithmetic.parent","from":"component.arithmetic","to":"component.parent","relation":"refines"
    }));
    model["questions"] = json!({"question.x":{"statement":"Ready to expose the action?",
        "status":"open","targets":["action.add"],"blocks":["implementation"]}});
    model["sources"] = json!({"source.x":{"role":"input","title":"Design","uri":"missing.md","format":"text","normative":false}});
    model["edges"][0]["source_refs"] = json!([{"purpose":"contract","source":"source.x"}]);
    write(&path, &model);
    let explained = cli(&path, "component.parent", &["--explain"]);
    assert_eq!(explained.status.code(), Some(1));
    let explanation = String::from_utf8(explained.stderr).unwrap();
    assert!(explanation.contains("QUESTION_BLOCKS_SCOPE gdd.json#/questions/question.x"));
    assert!(explanation.contains("Question question.x targets action.add."));
    assert!(explanation.contains("SOURCE_UNAVAILABLE gdd.json#/edges/0"));
    assert!(
        explanation.contains(
            "Edge edge.add.owner connects action.add -> component.arithmetic (owned_by)."
        )
    );
    let path_to_action = "Scope path from component.parent\n    component.parent -> component.arithmetic (initial-refinement) [reverse refines, edge.arithmetic.parent] gdd.json#/nodes/component.arithmetic\n    component.arithmetic -> action.add (public-interface) gdd.json#/nodes/action.add";
    assert!(explanation.contains(path_to_action));
    assert!(!explanation.contains("-> question.x"));
    assert!(!explanation.contains("-> edge.add.owner"));

    let unknown = cli(&path, "edge.add.owner", &["--explain"]);
    let explanation = String::from_utf8(unknown.stderr).unwrap();
    assert!(explanation.contains("Unknown readiness target."));
    assert!(!explanation.contains("Scope path"));
    assert!(!explanation.contains("connects"));

    model["nodes"]["action.add"]["description"] = json!(23);
    write(&path, &model);
    let unavailable = cli(&path, "component.arithmetic", &["--explain"]);
    let explanation = String::from_utf8(unavailable.stderr).unwrap();
    assert!(explanation.contains("SCHEMA_INVALID"));
    assert!(explanation.contains("REVISION_UNRESOLVED"));
    assert!(!explanation.contains("Scope path"));
}
