use super::{Project, analyze, write_json};
use anyhow::{Context, Result, ensure};
use gdd::{
    bundle,
    impact::{ImpactReport, analyze_impact},
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};

const IMPLEMENTATION: &str = "implementation.control";

struct Ledger {
    project: Project,
    revision: String,
    document: Value,
}

impl Ledger {
    fn new(directory: &Path) -> Result<Self> {
        let mut project = Project::new(directory)?;
        project.model["record_files"] = json!(["records.json"]);
        let path = project.save()?;
        let document = json!({"format":"gdd-records/0.2", "project_id":project.model["project"]["id"], "records":{}});
        write_json(&project.temp.path().join("records.json"), &document)?;
        let revision = bundle::load_model_only(path)
            .snapshot()
            .context("missing model revision")?
            .revision
            .clone();
        Ok(Self {
            project,
            revision,
            document,
        })
    }

    fn save(&mut self, records: Value) -> Result<&Path> {
        self.document["records"] = records;
        write_json(
            &self.project.temp.path().join("records.json"),
            &self.document,
        )?;
        Ok(&self.project.path)
    }
}

fn binding(revision: &str, resource: &str) -> Value {
    json!({"kind":"binding", "spec_revision":revision, "targets":["action.add"],
        "implementation":IMPLEMENTATION,"role":"code","resource":resource,
        "selector":{"kind":"whole-document"},"status":"planned"})
}

fn check(revision: &str, scope: &[&str]) -> Value {
    json!({"kind":"check","scope":scope,"spec_revision":revision,"method":"unit_test",
        "required_environment":[],"negative_controls":[]})
}

fn selected(report: &ImpactReport) -> BTreeSet<&str> {
    report
        .records
        .iter()
        .map(|record| record.id.as_str())
        .collect()
}

pub(super) fn history(directory: &Path, _: &str) -> Result<()> {
    let mut ledger = Ledger::new(directory)?;
    let records = json!({
        "binding.current":binding(&ledger.revision, "module.py"),
        "binding.history":binding(&format!("sha256:{}", "0".repeat(64)), "old.py"),
        "check.current":check(&ledger.revision, &["scenario.add"]),
    });
    let report = analyze_impact(
        ledger.save(records)?,
        &["action.add".into()],
        Some(IMPLEMENTATION),
    )?;
    ensure!(
        report.exit_code == 0
            && selected(&report) == BTreeSet::from(["binding.current", "check.current"])
    );
    ensure!(report.records_omitted.len() == 1 && report.records_omitted[0].id == "binding.history");
    ensure!(
        report
            .records
            .iter()
            .all(|r| r.spec_revision == ledger.revision)
    );
    Ok(())
}

pub(super) fn check_supersession(directory: &Path, replacement: &str) -> Result<()> {
    let mut ledger = Ledger::new(directory)?;
    let previous = check(&ledger.revision, &["scenario.add", "action.add"]);
    let mut current = previous.clone();
    current["supersedes"] = json!("check.old");
    let expected = match replacement {
        "compatible" => BTreeSet::from(["check.new"]),
        "different_method" => {
            current["method"] = json!("another_method");
            BTreeSet::from(["check.new", "check.old"])
        }
        "narrower_scope" => {
            current["scope"] = json!(["scenario.add"]);
            BTreeSet::from(["check.new", "check.old"])
        }
        "invalid" => {
            current["scope"] = json!(["scenario.add", "action.add", "unknown.id"]);
            BTreeSet::from(["check.old"])
        }
        _ => anyhow::bail!("unknown replacement {replacement}"),
    };
    let path = ledger.save(json!({"check.old":previous, "check.new":current}))?;
    let report = analyze(path, &["action.add"])?;
    ensure!(selected(&report) == expected);
    if replacement == "compatible" {
        ensure!(
            report
                .records_omitted
                .iter()
                .any(|r| r.id == "check.old" && r.reason.contains("supersession"))
        );
    }
    Ok(())
}

pub(super) fn binding_supersession(directory: &Path, _: &str) -> Result<()> {
    let mut ledger = Ledger::new(directory)?;
    let old = binding(&ledger.revision, "old.py");
    let mut new = binding(&ledger.revision, "new.py");
    new["supersedes"] = json!("binding.old");
    let path = ledger.save(json!({"binding.old":old, "binding.new":new}))?;
    let report = analyze(path, &["action.add"])?;
    ensure!(selected(&report) == BTreeSet::from(["binding.old", "binding.new"]));
    Ok(())
}
