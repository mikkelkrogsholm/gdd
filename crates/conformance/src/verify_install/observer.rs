//! Exercise static imports using a caller-authored model, bindings and CPython target.

use super::{Case, Runner};
use crate::fixture::write_json;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::fs;

const MODEL: &str = "python-model.json";
const RECORDS: &str = "python-records.json";
const IMPLEMENTATION: &str = "implementation.python";
const SOURCE: &str = "python-source";
const TARGET: &str = "python-runtime.json";
const VERSION: &str = "3.13.5";

fn observe(case: &mut Case, verdict: &str) -> Result<Value> {
    let report = case.compare(&[
        "check-code",
        MODEL,
        "--root",
        SOURCE,
        "--implementation",
        IMPLEMENTATION,
        "--profile",
        "python-imports/0.1",
        "--python-runtime",
        TARGET,
    ])?;
    ensure!(
        report["result"] == verdict,
        "expected {verdict} code observation, got {}",
        report["result"]
    );
    ensure!(
        report["runtime"]["python"] == VERSION && report["runtime"]["implementation"] == "CPython",
        "observer omitted the explicit CPython target"
    );
    Ok(report)
}

pub(super) fn check(run: &mut Runner) -> Result<()> {
    let root = run.recipient;
    let model = json!({
        "format":"gdd/0.3", "project":{"id":"project.python","title":"Python","goal":"Check imports"},
        "record_files":[RECORDS], "edges":[], "nodes":{
            "component.api":{"kind":"component","description":"API"},
            "component.storage":{"kind":"component","description":"Storage"},
            "rule.boundary":{"kind":"rule","description":"Boundary","rule":{
                "category":"architecture","statement":"API must not import storage.",
                "scope":"global","blocking":true,"verification":["python_imports"],
                "constraint":{"relation":"imports","from":"component.api","to":"component.storage","reach":"direct","policy":"forbid"}
            }}
        }
    });
    let mut mapping =
        json!({"format":"gdd-records/0.2","project_id":"project.python","records":{}});
    write_json(&root.join(MODEL), &model)?;
    write_json(&root.join(RECORDS), &mapping)?;
    write_json(
        &root.join(TARGET),
        &json!({"version":VERSION,"impl":"cpython","platform":"linux",
        "builtin_names":[],"frozen_names":[]}),
    )?;
    let source = root.join(SOURCE);
    fs::create_dir(&source)?;
    let api = source.join("api.py");
    let storage = source.join("storage.py");
    fs::write(&api, b"pass\n")?;
    fs::write(&storage, b"pass\n")?;
    let revision = run.case("code-model-snapshot", 0, |case| {
        let snapshot = case.compare(&["snapshot", MODEL])?;
        Ok(snapshot["revision"]
            .as_str()
            .context("missing code model revision")?
            .to_owned())
    })?;
    for name in ["api", "storage"] {
        mapping["records"][format!("binding.{name}")] = json!({
            "kind":"binding", "spec_revision":revision, "targets":[format!("component.{name}")],
            "implementation":IMPLEMENTATION,"role":"code","resource":format!("{name}.py"),
            "selector":{"kind":"whole-document"},"status":"planned"
        });
    }
    write_json(&root.join(RECORDS), &mapping)?;
    let clean = run.case("code-clean", 0, |case| {
        let report = observe(case, "PASS")?;
        Ok(report["code_revision"]
            .as_str()
            .context("missing clean code revision")?
            .to_owned())
    })?;
    fs::write(&api, b"import storage\n")?;
    run.case("code-forbidden-import", 1, |case| {
        let report = observe(case, "FAIL")?;
        ensure!(
            report["code_revision"]
                .as_str()
                .context("missing injected code revision")?
                != clean,
            "injected source retained the clean revision"
        );
        ensure!(
            report["checks"][0]["path"]
                .as_array()
                .is_some_and(|path| !path.is_empty()),
            "forbidden import has no witness path"
        );
        Ok(())
    })?;
    fs::write(&api, b"pass\n")?;
    run.case("code-repaired", 0, |case| {
        let report = observe(case, "PASS")?;
        ensure!(
            report["code_revision"] == clean,
            "repair did not restore the clean revision"
        );
        Ok(())
    })?;
    fs::remove_file(storage)?;
    run.case("code-missing-source", 2, |case| {
        observe(case, "UNKNOWN")?;
        Ok(())
    })
}
