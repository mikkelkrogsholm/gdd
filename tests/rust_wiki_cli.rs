use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn wiki(model: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gdd"))
        .arg("wiki")
        .arg(model)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn a_stakeholder_can_follow_the_published_example_and_an_agent_can_find_a_standard() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/knowledge");
    let model = dir.join("model.gdd.json");
    let result = wiki(&model, &["--source", "source.standard", "--check"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(
        report["links"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["target"] == "action.total"
                && l["purpose"] == "contract"
                && l["status"] == "pinned")
    );
    let nav = fs::read_to_string(dir.join("gdd-graph.md")).unwrap();
    assert!(nav.contains("gdd-node-action.total") && nav.contains("rounding.md#calculation"));
    let prose = fs::read_to_string(dir.join("wiki/rounding.md")).unwrap();
    assert!(prose.contains("../gdd-graph.md#gdd-node-action.total"));

    // Presentation has no graph link. Discover it in the wiki without widening a package.
    let found = wiki(&model, &["--search", "currency symbol"]);
    assert!(found.status.success());
    let matches: Value = serde_json::from_slice(&found.stdout).unwrap();
    assert!(
        matches["matches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["heading"] == "Invoice rounding / Presentation")
    );
    let scoped = wiki(
        &model,
        &["--search", "currency symbol", "--scope", "action.total"],
    );
    let scoped: Value = serde_json::from_slice(&scoped.stdout).unwrap();
    assert!(
        scoped
            .get("matches")
            .is_none_or(|v| v.as_array().unwrap().is_empty())
    );
}

#[test]
fn checking_a_changed_normative_page_reports_drift_without_showing_its_new_text() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/knowledge");
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join("wiki")).unwrap();
    for file in [
        "model.gdd.json",
        "workshop.md",
        "wiki/rounding.md",
        "wiki/requests.md",
    ] {
        fs::copy(fixture.join(file), tmp.path().join(file)).unwrap();
    }
    fs::write(
        tmp.path().join("wiki/rounding.md"),
        "unreviewed replacement",
    )
    .unwrap();
    let result = wiki(
        &tmp.path().join("model.gdd.json"),
        &["--check", "--source", "source.standard", "--format", "text"],
    );
    assert_eq!(result.status.code(), Some(1));
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(text.contains("stale") && !text.contains("unreviewed replacement"));
}

#[test]
fn sample_plan_contains_the_rule_and_the_unresolved_request() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/knowledge");
    let output = tempfile::tempdir().unwrap();
    let plan_path = output.path().join("plan.json");
    let result = Command::new(env!("CARGO_BIN_EXE_gdd"))
        .args(["workflow", "plan"])
        .arg(source.join("model.gdd.json"))
        .arg("--request")
        .arg(source.join("request.json"))
        .arg("--output")
        .arg(&plan_path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let plan: Value = serde_json::from_slice(&fs::read(plan_path).unwrap()).unwrap();
    assert_eq!(plan["format"], "gdd-workplan/0.2");
    let knowledge = &plan["packages"][0]["knowledge"];
    assert_eq!(knowledge["excerpts"].as_object().unwrap().len(), 2);
    assert_eq!(knowledge["questions"][0]["id"], "question.rounding");
    assert_eq!(knowledge["questions"][0]["status"], "open");
    assert!(
        knowledge["links"]
            .as_array()
            .unwrap()
            .iter()
            .any(|link| link["source"] == "source.standard"
                && link["target"] == "action.total"
                && link["purpose"] == "contract")
    );
}
