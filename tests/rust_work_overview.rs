use gdd::{
    input::Value,
    report::CheckResult,
    work_overview::{WorkOverview, work_overview, work_overview_text},
};
use std::{fs, path::Path};

const FIXTURE: &str = "examples/agent-native";

#[test]
fn package_progress_tracks_only_targets_and_their_owners() {
    let fixture = Path::new(FIXTURE);
    let model = fixture.join("model.gdd.json");
    let run = tempfile::tempdir().unwrap();
    let events = run.path().join("events");
    fs::create_dir(&events).unwrap();
    let recorded = fixture.join("native-run");
    fs::copy(recorded.join("run.json"), run.path().join("run.json")).unwrap();
    let initial = work_overview(&model, run.path(), None).unwrap();
    let ready = initial
        .packages
        .iter()
        .filter(|p| p.available)
        .map(|p| p.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ready, ["package.discount", "package.tax"]);
    let quote = initial.packages_for(&["component.quote".into()]);
    assert_eq!(quote.len(), 1);
    assert_eq!(quote[0].id, "package.quote");
    assert!(
        quote[0]
            .blockers
            .iter()
            .any(|s| s.contains("package.discount"))
    );
    assert!(quote[0].blockers.iter().any(|s| s.contains("package.tax")));
    assert_eq!(quote[0].verification, None);
    for n in 1..=6 {
        let name = format!("{n:06}.json");
        fs::copy(recorded.join("events").join(&name), events.join(name)).unwrap();
    }
    let mut later = work_overview(&model, run.path(), None).unwrap();
    assert!(later.retain_scope(&["action.unknown".into()]).is_err());
    later.retain_scope(&["action.quote".into()]).unwrap();
    assert_eq!(later.packages.len(), 1);
    assert!(later.packages[0].available);
    assert!(later.packages[0].blockers.is_empty());
    assert!(work_overview_text(&later, &[]).contains("available"));
    later.retain_scope(&["scenario.quote.cli".into()]).unwrap();
    assert!(later.packages.is_empty());
}

#[test]
fn captured_evidence_matches_the_run_and_keeps_original_references() {
    let fixture = Path::new(FIXTURE);
    let run = fixture.join("native-run");
    let model = fixture.join("model.gdd.json");
    let mut host: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.join("host.acceptance.json")).unwrap()).unwrap();
    let report = work_overview(&model, &run, Some(&Value::from(&host))).unwrap();
    assert!(report.plan_current && report.acceptance_current);
    for package in &report.packages {
        assert_eq!(package.phase, "integrated");
        assert_eq!(package.verification, Some(CheckResult::Pass));
        assert!(package.evidence.iter().any(|check| {
            check.id == "check.demo.all"
                && check
                    .evidence_refs
                    .iter()
                    .any(|r| r == "evidence.demo.final")
                && check.code_revision.as_deref() == Some(report.code_revision.as_str())
        }));
    }
    let package = &report.packages_for(&["component.discount".to_owned()])[0];
    let rules: std::collections::BTreeSet<_> = package
        .evidence
        .iter()
        .filter_map(|check| check.rule.as_deref())
        .collect();
    assert_eq!(
        rules,
        ["rule.discount_floor", "rule.input"].into_iter().collect()
    );
    let tax = &report.packages_for(&["action.tax".to_owned()])[0];
    assert!(
        tax.evidence
            .iter()
            .any(|check| check.rule.as_deref() == Some("rule.tax_floor"))
    );
    assert!(
        !tax.evidence
            .iter()
            .any(|check| check.rule.as_deref() == Some("rule.discount_floor"))
    );
    let saved = serde_json::to_vec(&report).unwrap();
    let offline: WorkOverview = serde_json::from_slice(&saved).unwrap();
    assert_eq!(offline.run_revision, report.run_revision);

    host["code_revision"] = "a-different-checkout".into();
    let stale = work_overview(&model, &run, Some(&Value::from(&host))).unwrap();
    assert!(!stale.acceptance_current);
    assert!(stale.evidence_revision.is_none());
    assert!(
        stale
            .packages
            .iter()
            .all(|p| p.verification == Some(CheckResult::Unknown)
                && p.evidence.iter().all(|e| e.evidence_refs.is_empty()))
    );
}

#[test]
fn model_changes_keep_history_and_require_a_new_plan() {
    let fixture = Path::new(FIXTURE);
    let tmp = tempfile::tempdir().unwrap();
    for name in ["model.gdd.json", "model.records.json", "REQUIREMENTS.md"] {
        fs::copy(fixture.join(name), tmp.path().join(name)).unwrap();
    }
    let model = tmp.path().join("model.gdd.json");
    let mut bytes = fs::read(&model).unwrap();
    bytes.push(b'\n');
    fs::write(&model, bytes).unwrap();
    let report = work_overview(&model, &fixture.join("native-run"), None).unwrap();
    assert_eq!(report.run_phase, "accepted");
    assert!(!report.acceptance_current && !report.plan_current);
    assert_ne!(
        report.spec_revision.as_deref(),
        Some(report.plan_spec_revision.as_str())
    );
}
