use gdd::gaps::check;
use serde_json::{Value, json};
use std::{fs, process::Command};

#[test]
fn removing_an_owner_explains_the_hole_and_restoring_it_clears_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    let mut model = json!({
        "format":"gdd/0.3","project":{"id":"example.gaps","title":"Graph gaps","goal":"Review booking gaps."},
        "nodes":{"component.booking":{"kind":"component","description":"Bookings"},
                 "action.book":{"kind":"action","description":"Book"}},
        "edges":[{"id":"edge.owner","from":"action.book","to":"component.booking","relation":"owned_by"}]
    });
    let run = |model: &Value| {
        fs::write(&path, model.to_string()).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_gdd"))
            .arg("gaps")
            .arg(&path)
            .args(["--scope", "action.book"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let base = run(&model);
    let owner = model["edges"].as_array_mut().unwrap().pop().unwrap();
    let broken = run(&model);
    let keys = |v: &Value| {
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["key"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(&base), ["MISSING_NORMAL_SCENARIO"]);
    assert_eq!(
        keys(&broken),
        ["MISSING_ACTION_OWNER", "MISSING_NORMAL_SCENARIO"]
    );
    assert_ne!(base["spec_revision"], broken["spec_revision"]);
    model["nodes"]["action.book"]["annotations"] =
        json!({"gdd.gap_omissions":{"MISSING_ACTION_OWNER":"Ownership is outside this draft."}});
    assert_eq!(
        run(&model)["findings"][0]["omission_reason"],
        "Ownership is outside this draft."
    );
    model["edges"].as_array_mut().unwrap().push(owner);
    assert_eq!(keys(&run(&model)), keys(&base));
    assert!(check(&path, &["absent".into()]).is_err());
}

#[test]
fn scopes_and_saved_views_preserve_advisories() {
    let path = std::path::Path::new("examples/product/partial.gdd.json");
    assert_eq!(check(path, &[]).unwrap().findings.len(), 5);
    let scope = ["step.reserve".to_owned()];
    let scoped = check(path, &scope).unwrap();
    assert_eq!(scoped.findings.len(), 4);
    let dir = tempfile::tempdir().unwrap();
    let saved = dir.path().join("view.gddview");
    gdd::view::render_view(path, &saved, &scope).unwrap();
    let view = gdd::view::read_view(&fs::read(saved).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(gdd::gaps::from_view(&view)).unwrap(),
        serde_json::to_value(scoped).unwrap()
    );
}

#[test]
fn machine_paths_error_scenarios_and_source_backed_reviews() {
    let capture = gdd::perspectives::load_perspective_bundle(std::path::Path::new(
        "examples/concert.gdd.yaml",
    ))
    .unwrap();
    let mut view = gdd::view::view_data(&capture, &[]).unwrap();
    let model = &mut view["model"];
    model["nodes"]["machine.reservation"]["annotations"]["gdd.expected_triggers"] =
        json!({"reserved":["action.cancel"]});
    let machine = &mut model["nodes"]["machine.reservation"]["machine"];
    machine["transitions"].as_array_mut().unwrap().pop();
    model["nodes"]["action.reserve"]["contract"]["error_cases"]["InvalidInput"] =
        json!(["scenario.reserve"]);
    model["sources"]["source.policy"] = json!({"role":"knowledge","normative":true});
    model["nodes"]["action.reserve"]["annotations"]["gdd.gap_candidates"] =
        json!({"retry":{"source":"source.policy","question":"Can callers retry?"}});
    let report = gdd::gaps::from_view(&view);
    for key in [
        "MISSING_TRANSITION/reserved/action.cancel",
        "UNREACHABLE_STATE/cancelled",
        "NONTERMINAL_DEAD_END/reserved",
        "MISSING_ERROR_CASE/InvalidInput",
        "UNREFERENCED_KNOWLEDGE",
    ] {
        assert!(report.findings.iter().any(|f| f.key == key), "{key}");
    }
    let review = report
        .findings
        .iter()
        .find(|f| f.key == "CANDIDATE/retry")
        .unwrap();
    assert_eq!(review.kind, "review");
    assert_eq!(review.source.as_deref(), Some("source.policy"));
    assert_eq!(review.message, "Can callers retry?");
}

#[test]
fn existing_open_questions_remain_visible_in_check_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_gdd"))
        .args([
            "gaps",
            "examples/knowledge/model.gdd.json",
            "--scope",
            "action.total",
            "--check",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let question = &report["findings"][0];
    assert_eq!(question["key"], "OPEN_QUESTION/question.rounding");
    assert_eq!(question["target"], "action.total");
    assert!(
        question["message"]
            .as_str()
            .unwrap()
            .contains("blocks acceptance")
    );
}
