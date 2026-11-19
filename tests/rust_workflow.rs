use gdd::bundle;
use gdd::input::{self, Value};
use gdd::report::{CheckResult, Report};
use gdd::workflow::{
    prepare_workflow, prepare_workflow_from_bundle, verify_workflow, verify_workflow_from_bundle,
};
use serde_json::{Value as Json, json};
use std::{fs, path::Path, process::Command};

fn parsed(json: &Json) -> Value {
    Value::from(json)
}

fn check(report: &Report, id: &str) -> CheckResult {
    report
        .checks
        .iter()
        .find(|c| c.check_id == id)
        .unwrap()
        .result
}

fn request(target: &str) -> Json {
    json!({
        "format": "gdd-work-request/0.1", "id": "work.test", "objective": "Implement a contract.",
        "implementation": "test", "base_code_revision": "base",
        "packages": [{"id": "package.one", "scope": [target], "workspace": "work",
            "write_resources": ["src/item"], "read_resources": [], "deliverables": ["Item"],
            "integration_plan": "Verify the contract."}],
        "serialized": [], "integration": {"owner": "owner", "checks": []}
    })
}

#[test]
fn archived_plan_verifies_on_captured_ledger_and_detects_replacement() {
    let original = Path::new("examples/agent-native");
    let dir = tempfile::tempdir().unwrap();
    for asset in ["model.gdd.json", "model.records.json", "REQUIREMENTS.md"] {
        fs::copy(original.join(asset), dir.path().join(asset)).unwrap();
    }
    let path = dir.path().join("model.gdd.json");
    let model = bundle::load(&path);
    let request = input::parse(&fs::read(original.join("workflow.request.json")).unwrap()).unwrap();
    let archived: Json =
        serde_json::from_slice(&fs::read(original.join("workflow.plan.json")).unwrap()).unwrap();
    let prepared = prepare_workflow_from_bundle(&model, &request, None).unwrap();
    assert_eq!(prepared.artifact, archived);
    assert_eq!(
        check(&prepared.report, "workflow.check_plans"),
        CheckResult::Pass
    );

    let ledger = dir.path().join("model.records.json");
    let mut records: Json = serde_json::from_slice(&fs::read(&ledger).unwrap()).unwrap();
    let mut replacement = records["records"]["check.demo.all"].clone();
    replacement["supersedes"] = json!("check.demo.all");
    records["records"]["check.replacement"] = replacement;
    fs::write(&ledger, serde_json::to_vec(&records).unwrap()).unwrap();
    let archived = parsed(&archived);
    let current = verify_workflow(&path, &archived, None).unwrap();
    assert_eq!(
        check(&current, "workflow.check_plans"),
        CheckResult::Unknown
    );
    assert_eq!(
        check(&current, "workflow.packet_integrity"),
        CheckResult::Fail
    );
    assert_eq!(current.spec_revision, prepared.report.spec_revision);

    fs::remove_file(&path).unwrap();
    fs::remove_file(dir.path().join("REQUIREMENTS.md")).unwrap();
    let captured = verify_workflow_from_bundle(&model, &archived, None).unwrap();
    assert_eq!(
        check(&captured, "workflow.packet_integrity"),
        CheckResult::Pass
    );
    assert_eq!(check(&captured, "workflow.check_plans"), CheckResult::Pass);
}

const MANIFEST: &str = r#"{"format":"gdd/0.2","project":{"id":"test.work","title":"Amounts","goal":"Store exact values."},"nodes":{"component.owner":{"kind":"component","description":"Owns the amount."}},"edges":[],"includes":["./parts//items.json"]}"#;
const FRAGMENT: &str = r#"{"format":"gdd-fragment/0.2","nodes":{"entity.item":{"kind":"entity","description":"Amount in €.","fields":{"amount":{"type":"number","minimum":-0.00,"maximum":1.000e16},"rate":{"type":"number","minimum":1E-007}}}},"edges":[{"id":"edge.owner","relation":"owned_by","from":"entity.item","to":"component.owner"}]}"#;

#[test]
fn packet_hash_binds_declared_fragment_spelling_and_canonical_numeric_types() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    fs::write(&path, MANIFEST).unwrap();
    fs::create_dir(dir.path().join("parts")).unwrap();
    let fragment = dir.path().join("parts/items.json");
    fs::write(&fragment, FRAGMENT).unwrap();
    let request = parsed(&request("entity.item"));
    let result = prepare_workflow(&path, &request, None).unwrap();
    let plan = result.artifact;
    let location = &plan["packages"][0]["locations"];
    assert_eq!(
        location["entity.item"],
        json!(["./parts//items.json", "/nodes/entity.item"])
    );
    assert_eq!(
        location["edge.owner"],
        json!(["./parts//items.json", "/edges/0"])
    );
    assert_eq!(plan["snapshot"]["assets"][1]["path"], "parts/items.json");
    // Independent canonical hash from the Python reference.
    assert_eq!(
        plan["revision"],
        "sha256:cc39e825776bfb82767813264ae928d89df6f96009877e3096c46a590200745b"
    );
    let mut same = plan.clone();
    let fields = &mut same["packages"][0]["model_slice"]["nodes"]["entity.item"]["fields"];
    fields["amount"]["minimum"] = serde_json::from_str("-0.0").unwrap();
    fields["amount"]["maximum"] = serde_json::from_str("1e+16").unwrap();
    fields["rate"]["minimum"] = serde_json::from_str("1e-07").unwrap();
    assert_eq!(
        check(
            &verify_workflow(&path, &parsed(&same), None).unwrap(),
            "workflow.packet_integrity"
        ),
        CheckResult::Pass
    );
    same["packages"][0]["model_slice"]["nodes"]["entity.item"]["fields"]["amount"]["minimum"] =
        json!(0);
    assert_eq!(
        check(
            &verify_workflow(&path, &parsed(&same), None).unwrap(),
            "workflow.packet_integrity"
        ),
        CheckResult::Fail
    );

    fs::write(&fragment, FRAGMENT.replace("1E-007", "1e-4000")).unwrap();
    let exact = prepare_workflow(&path, &request, None).unwrap().artifact;
    let mut altered = exact.clone();
    altered["packages"][0]["model_slice"]["nodes"]["entity.item"]["fields"]["rate"]["minimum"] =
        json!(0.0);
    assert_eq!(
        check(
            &verify_workflow(&path, &parsed(&exact), None).unwrap(),
            "workflow.packet_integrity"
        ),
        CheckResult::Pass
    );
    assert_eq!(
        check(
            &verify_workflow(&path, &parsed(&altered), None).unwrap(),
            "workflow.packet_integrity"
        ),
        CheckResult::Fail
    );
}

#[test]
fn core_model_blocks_derivation_and_operational_errors_are_reported_with_drafts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    let mut manifest: Json = serde_json::from_str(MANIFEST).unwrap();
    manifest.as_object_mut().unwrap().remove("includes");
    manifest["record_files"] = json!(["invalid.json"]);
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    fs::write(
        dir.path().join("invalid.json"),
        br#"{"format":"gdd-records/0.2","project_id":"test.work","records":{"check.bad":{}}}"#,
    )
    .unwrap();
    let mut req = request("component.owner");
    req["integration"]["checks"] = json!(["check.bad"]);
    let result = prepare_workflow(&path, &parsed(&req), None).unwrap();
    assert_eq!(result.report.result("model"), CheckResult::Fail);
    assert_eq!(result.artifact["packages"][0]["check_plans"], json!({}));
    assert_eq!(
        check(&result.report, "workflow.check_plans"),
        CheckResult::Unknown
    );

    manifest["edges"] = json!([{"id":"edge.unknown","from":"component.owner","relation":"depends_on","to":"component.missing"}]);
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(
        prepare_workflow(&path, &parsed(&req), None)
            .unwrap_err()
            .to_string()
            .contains("unresolved model semantics")
    );
}

#[test]
fn a_pinned_extension_identity_error_fails_the_report_and_keeps_its_packet_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let model = dir.path().join("model.json");
    let src = Path::new("examples/product");
    let mut manifest: Json =
        serde_json::from_slice(&fs::read(src.join("greenfield.gdd.json")).unwrap()).unwrap();
    fs::create_dir(dir.path().join("extensions")).unwrap();
    let ext = "extensions/product-0.1.json";
    fs::copy(src.join(ext), dir.path().join(ext)).unwrap();
    manifest["extensions"]["extension.product"]["version"] = json!("8.0.0");
    fs::write(&model, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let req = parsed(&request("component.booking"));
    let draft = prepare_workflow(&model, &req, None).unwrap();
    assert_eq!(
        check(&draft.report, "check.extension_identity_mismatch"),
        CheckResult::Fail
    );
    assert_eq!(
        draft.artifact["packages"][0]["extension_semantics"]["gdd.product"]["version"],
        "0.1.0"
    );
    assert_eq!(
        check(&draft.report, "package.one.readiness.unavailable"),
        CheckResult::Unknown
    );
    // Two separately hashed declarations can resolve a snapshot while their
    // duplicate namespace prevents deriving a graph with unambiguous semantics.
    manifest["extensions"]["extension.product"]["version"] = json!("0.1.0");
    let mut duplicate = manifest["extensions"]["extension.product"].clone();
    duplicate["path"] = json!("other.json");
    manifest["extensions"]["extension.other"] = duplicate;
    fs::copy(src.join(ext), dir.path().join("other.json")).unwrap();
    fs::write(&model, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let captured = bundle::load(&model);
    assert!(captured.snapshot().is_some());
    assert!(
        prepare_workflow_from_bundle(&captured, &req, None)
            .unwrap_err()
            .to_string()
            .contains("unresolved model semantics")
    );
}

#[test]
fn cli_writes_consumable_plan_exclusively_and_verification_preserves_input() {
    let root = Path::new("examples/agent-native");
    let manifest = root.join("model.gdd.json");
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("plan.json");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_gdd"))
            .args(["workflow", "plan"])
            .arg(&manifest)
            .arg("--request")
            .arg(root.join("workflow.request.json"))
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap()
    };
    let first = run();
    assert!(first.status.success());
    let stored = fs::read(&output).unwrap();
    let wrapper: Json = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(
        wrapper["artifact"],
        serde_json::from_slice::<Json>(&stored).unwrap()
    );
    assert!(!run().status.success());
    assert_eq!(fs::read(&output).unwrap(), stored);
    let report = dir.path().join("report.json");
    let verification = Command::new(env!("CARGO_BIN_EXE_gdd"))
        .args(["workflow", "verify"])
        .arg(&manifest)
        .arg("--plan")
        .arg(&output)
        .arg("--output")
        .arg(&report)
        .output()
        .unwrap();
    assert!(verification.status.success());
    assert!(verification.stdout.is_empty());
    let report: Json = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(
        report["checks"].as_array().unwrap().last().unwrap()["result"],
        "PASS"
    );
    assert_eq!(fs::read(&output).unwrap(), stored);
}

#[test]
fn historical_workplan_hashes_preserve_out_of_range_exponent_spelling() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    let manifest = include_str!("../docs/tutorial/refined.gdd.json");
    let request = parsed(&request("action.add"));
    // These identities were generated by the native workflow implementation
    // before its canonical serializer was shared with model inspection.
    for (token, expected) in [
        (
            "1E+09223372036854775809",
            "sha256:35230adac95d2c55bd4b3bc31e516866d2b0118535563e24b2a5919cfbc4362a",
        ),
        (
            "10.0e9223372036854775808",
            "sha256:a3f1a9f8e23eb967094f6982d17a7d26e4eddfc183d780e95f54bc0cf6302667",
        ),
    ] {
        let model = manifest.replace(
            r#""kind": "action","#,
            &format!(r#""kind": "action", "annotations": {{"test.big": {token}}},"#),
        );
        fs::write(&path, model).unwrap();
        let plan = prepare_workflow(&path, &request, None).unwrap().artifact;
        assert_eq!(plan["revision"], expected);
        let verified = verify_workflow(&path, &parsed(&plan), None).unwrap();
        assert_eq!(
            check(&verified, "workflow.packet_integrity"),
            CheckResult::Pass
        );
    }
}

#[test]
fn wiki_packets_dispatch_frozen_scoped_guidance_and_reconcile_source_drift() {
    use gdd::workflow_run::{advance_run, create_run};

    let fixture = Path::new("examples/agent-native");
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    let path = dir.join("model.gdd.json");
    let ledger = dir.join("model.records.json");
    let read =
        |file| -> Json { serde_json::from_slice(&fs::read(fixture.join(file)).unwrap()).unwrap() };
    let write = |path: &Path, data: &Json| {
        fs::write(path, serde_json::to_vec(data).unwrap()).unwrap();
    };
    let billing = "# Billing\n\n## Rounding\n\nUse integer floors for [discounts](gdd:node/action.discount).\n\n## Experiments\nPrivate draft idea.\n";
    let wiki_source = |file: &str| json!({"role":"knowledge", "title":file, "uri":file, "format":"text/markdown", "normative":false});
    let mut manifest = read("model.gdd.json");
    manifest["sources"]["source.billing"] = wiki_source("Billing.md");
    manifest["sources"]["source.billing"]["revision"] =
        json!({"sha256":gdd::sha256(billing.as_bytes())});
    manifest["sources"]["source.unrelated"] = wiki_source("Other.md");
    manifest["nodes"]["action.discount"]["source_refs"].as_array_mut().unwrap().push(
        json!({"source":"source.billing","purpose":"provenance","selector":{"kind":"markdown-heading","value":"Billing / Rounding"}})
    );
    fs::copy(fixture.join("REQUIREMENTS.md"), dir.join("REQUIREMENTS.md")).unwrap();
    fs::write(dir.join("Billing.md"), billing).unwrap();
    fs::write(dir.join("Other.md"), "# Elsewhere\n").unwrap();
    write(&path, &manifest);

    // Only the check declaration belongs to this synthetic snapshot. Historical
    // executed evidence and host acceptance stay attached to the original demo.
    let revision = bundle::load_model_only(&path)
        .snapshot()
        .unwrap()
        .revision
        .clone();
    let mut records = read("model.records.json");
    let mut selected = records["records"]["check.demo.all"].clone();
    selected["spec_revision"] = json!(revision);
    records["records"] = json!({"check.demo.all":selected});
    write(&ledger, &records);
    let request = parsed(&read("workflow.request.json"));
    let snapshot = bundle::load_for_knowledge(&path, &[]);
    let plan = prepare_workflow_from_bundle(&snapshot, &request, None)
        .unwrap()
        .artifact;
    assert_eq!(
        plan,
        prepare_workflow(&path, &request, None).unwrap().artifact
    );
    assert_eq!(plan["format"], "gdd-workplan/0.2");
    let packet = plan["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "package.discount")
        .unwrap();
    let knowledge = &packet["knowledge"];
    assert_eq!(knowledge["links"][0]["status"], "pinned");
    let excerpts = knowledge["excerpts"].as_object().unwrap();
    assert_eq!(excerpts.len(), 1);
    let (id, prose) = excerpts.iter().next().unwrap();
    assert!(prose.as_str().unwrap().contains("Use integer floors"));
    assert_eq!(
        id,
        &format!("sha256:{}", gdd::sha256(prose.as_str().unwrap().as_bytes()))
    );
    let links = knowledge["links"].as_array().unwrap();
    assert_eq!(links.len(), 2);
    for link in links {
        assert_eq!(link["excerpt_ref"].as_str(), Some(id.as_str()));
        assert!(link.get("excerpt").is_none());
        assert_eq!(link["source"], "source.billing");
        assert_eq!(link["truncated"], false);
    }
    assert!(!knowledge.to_string().contains("Private draft idea."));
    assert!(!knowledge.to_string().contains("source.unrelated"));

    // A previously published 0.1 plan on the same model keeps its historical profile.
    let mut old_plan = plan.clone();
    old_plan["format"] = json!("gdd-workplan/0.1");
    for packet in old_plan["packages"].as_array_mut().unwrap() {
        packet.as_object_mut().unwrap().remove("knowledge");
    }
    old_plan.as_object_mut().unwrap().remove("revision");
    old_plan["revision"] = json!(gdd::canonical_digest(&old_plan));
    assert_eq!(
        check(
            &verify_workflow(&path, &parsed(&old_plan), None).unwrap(),
            "workflow.packet_integrity"
        ),
        CheckResult::Pass
    );

    let run = dir.join("run");
    let plan_input = parsed(&plan);
    let initial = create_run(&path, &plan_input, &run, None).unwrap();
    let mut host = read("host.implementation.json");
    host["policy"]["spec_revision"] = json!(revision);
    let host = parsed(&host);
    let dispatch = |revision: &str, package: &str| {
        parsed(
            &json!({"type":"dispatch", "package":package, "attempt":format!("{package}.one"), "idempotency_key":package,"expected_revision":revision}),
        )
    };
    fs::write(dir.join("Other.md"), "# Unrelated edit\n").unwrap();
    let event = dispatch(&initial.revision, "package.discount");
    let dispatched = advance_run(&path, &run, &event, Some(&host)).unwrap();
    assert!(dispatched.applied, "{:?}", dispatched.report);
    assert_eq!(
        dispatched.artifact.unwrap()["package"]["knowledge"],
        *knowledge
    );

    fs::write(
        dir.join("Billing.md"),
        "# Billing\n\n## Rounding\nChanged rule.\n",
    )
    .unwrap();
    let current = verify_workflow(&path, &plan_input, None).unwrap();
    assert_eq!(
        check(&current, "workflow.packet_integrity"),
        CheckResult::Fail
    );
    let saved = verify_workflow_from_bundle(&snapshot, &plan_input, None).unwrap();
    assert_eq!(
        check(&saved, "workflow.packet_integrity"),
        CheckResult::Pass
    );
    let revision = dispatched.receipt.unwrap()["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let attempt = advance_run(
        &path,
        &run,
        &dispatch(&revision, "package.tax"),
        Some(&host),
    )
    .unwrap();
    assert!(!attempt.applied);
    assert!(attempt.artifact.is_none());
}

#[test]
fn first_wiki_backlink_after_a_plan_requires_refresh() {
    let dir = tempfile::tempdir().unwrap();
    let model = dir.path().join("gdd.json");
    let page = dir.path().join("team.md");
    let manifest = json!({
        "format":"gdd/0.2",
        "project":{"id":"project.work","title":"Work","goal":"Prepare the first slice."},
        "nodes":{"component.core":{"kind":"component","description":"Owns the first slice."}},
        "edges":[],
        "sources":{"source.team":{"role":"knowledge","title":"Team conventions",
            "uri":"team.md","format":"text/markdown","normative":false}}
    });
    fs::write(&model, serde_json::to_vec(&manifest).unwrap()).unwrap();
    fs::write(&page, "# Conventions\n").unwrap();
    let request = parsed(&request("component.core"));
    let plan = prepare_workflow(&model, &request, None).unwrap().artifact;
    assert_eq!(plan["format"], "gdd-workplan/0.2");
    assert!(plan["packages"][0].get("knowledge").is_none());
    fs::write(
        &page,
        "# Conventions\nApplies to [core](gdd:node/component.core).\n",
    )
    .unwrap();
    let stale = verify_workflow(&model, &parsed(&plan), None).unwrap();
    assert_eq!(
        check(&stale, "workflow.packet_integrity"),
        CheckResult::Fail
    );
}
