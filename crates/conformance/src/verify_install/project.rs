//! Check the installed scaffold, read-only inspections and refused initializations.

use super::{Program, Runner};
use crate::fixture::{snapshot, write_json};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

const STARTER: &str = "fresh-project/gdd.json";
const READY: &str = "docs/tutorial/refined.gdd.json";
const CHANGED: &str = "changed-model.json";

pub(super) fn check(run: &mut Runner) -> Result<()> {
    let root = run.recipient;
    let project = root.join("fresh-project");
    let init = [
        "init",
        "fresh-project",
        "--project-id",
        "project.installed",
        "--title",
        "Installed CLI starter",
    ];
    let starter = run.case("fresh-project-init", 0, |case| {
        let report = case.json(Program::Installed, &init)?;
        ensure!(
            report["format"] == "gdd-init/0.1"
                && report["project_id"] == "project.installed"
                && report["model"] == "PASS"
                && report["readiness"] == "FAIL",
            "unexpected starter init receipt"
        );
        let files = snapshot(&project)?;
        ensure!(
            files.keys().eq(["SOURCES.md", "gdd.json", "records.json"]
                .map(PathBuf::from)
                .iter()),
            "expected SOURCES.md, gdd.json and records.json in the starter, found {:?}",
            files.keys()
        );
        Ok(files)
    })?;
    run.compare("fresh-project-model", 0, &["validate", STARTER])?;
    run.case("fresh-project-status", 1, |case| {
        let report = case.compare(&["status", STARTER, "--scope", "action.main"])?;
        ensure!(
            report["assessments"]["readiness"]["result"] == "FAIL",
            "starter status must report readiness FAIL"
        );
        Ok(())
    })?;
    let ready = fs::read(root.join(READY))?;
    run.case("ready-status", 0, |case| {
        let report = case.compare(&["status", READY, "--scope", "action.add"])?;
        ensure!(
            report["assessments"]["authorization"]
                == json!({"assessed": false, "result": null, "check_refs": []}),
            "status inferred authorization without host input"
        );
        ensure!(
            fs::read(root.join(READY))? == ready,
            "status changed {READY}"
        );
        Ok(())
    })?;
    run.case("ready-impact", 0, |case| {
        let report = case.compare(&["impact", READY, "--scope", "action.add"])?;
        ensure!(
            report["coverage"]["basis"] == "declared-model",
            "impact omitted its declared-model basis"
        );
        ensure!(
            fs::read(root.join(READY))? == ready && snapshot(&project)? == starter,
            "read-only inspection changed the ready model or starter files"
        );
        Ok(())
    })?;
    run.case("equal-model-diff", 0, |case| {
        let diff = case.compare(&["diff", READY, READY])?;
        ensure!(
            diff["comparison"]["semantic_changes"] == false,
            "equal models should have no semantic changes"
        );
        ensure!(fs::read(root.join(READY))? == ready, "diff changed {READY}");
        Ok(())
    })?;
    let mut weakened: Value = serde_json::from_slice(&ready)?;
    let conditions = weakened
        .pointer_mut("/nodes/action.add/contract/postconditions")
        .context("refined model has no action.add postconditions")?;
    ensure!(
        conditions.as_array().is_some_and(|items| !items.is_empty()),
        "expected at least one postcondition to weaken"
    );
    *conditions = json!([]);
    write_json(&root.join(CHANGED), &weakened)?;
    let before = fs::read(root.join(CHANGED))?;
    run.case("weakened-contract-diff", 0, |case| {
        let diff = case.compare(&["diff", READY, CHANGED])?;
        ensure!(
            diff["comparison"]["semantic_changes"] == true
                && diff["advisories"].as_array().is_some_and(|a| !a.is_empty()),
            "diff omitted the weakened contract or its advisory"
        );
        ensure!(
            fs::read(root.join(READY))? == ready && fs::read(root.join(CHANGED))? == before,
            "diff changed its inputs"
        );
        Ok(())
    })?;
    run.case("fresh-project-unready", 1, |case| {
        let report = case.compare(&[
            "validate",
            STARTER,
            "--profile",
            "readiness",
            "--scope",
            "action.main",
        ])?;
        ensure!(
            report["checks"]
                .as_array()
                .context("missing readiness checks")?
                .iter()
                .any(|check| check["dimension"] == "readiness" && check["result"] == "FAIL"),
            "expected starter readiness FAIL"
        );
        ensure!(
            snapshot(&project)? == starter,
            "validation changed the starter"
        );
        Ok(())
    })?;
    run.case("repeated-init-preserves-files", 1, |case| {
        case.execute(Program::Installed, &init)?;
        ensure!(
            snapshot(&project)? == starter,
            "repeated init changed the starter"
        );
        Ok(())
    })?;
    let occupied = root.join("occupied");
    fs::create_dir(&occupied)?;
    fs::write(
        occupied.join("existing.txt"),
        b"Existing work must survive.\n",
    )?;
    let before = snapshot(&occupied)?;
    run.case("occupied-init-preserves-files", 1, |case| {
        case.execute(
            Program::Installed,
            &[
                "init",
                "occupied",
                "--project-id",
                "project.occupied",
                "--title",
                "Occupied directory",
            ],
        )?;
        ensure!(
            snapshot(&occupied)? == before,
            "init changed an occupied directory"
        );
        Ok(())
    })
}
