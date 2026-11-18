//! Compose project status with freshly prepared runs and the archived acceptance journal.

use super::{Project, expect_dimension, read_json, validate_status};
use crate::fixture::{snapshot, write_json};
use anyhow::{Context, Result, ensure};
use gdd::input::{self, Value as Input};
use gdd::report::CheckResult;
use gdd::status::{StatusOptions, project_status_with_run, status_text};
use gdd::workflow::prepare_workflow;
use gdd::workflow_run::{RunPhase, create_run, run_status};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn pending_run(project: &Project) -> Result<PathBuf> {
    let request = json!({
        "format":"gdd-work-request/0.1", "id":"work.add", "objective":"Implement addition",
        "implementation":"implementation.example", "base_code_revision":"synthetic-0",
        "packages":[{"id":"work.arithmetic","scope":["action.add"],"workspace":"../workspace",
            "write_resources":["src/add.py"],"read_resources":[],
            "deliverables":["Addition implementation"],"integration_plan":"Review and run checks"}],
        "serialized":[],"integration":{"owner":"actor.integrator","checks":[]}
    });
    let plan = prepare_workflow(&project.model, &Input::from(&request), None)?.artifact;
    let run = project.temp.path().join("run");
    create_run(&project.model, &Input::from(&plan), &run, None)?;
    Ok(run)
}

pub(super) fn optional_run(directory: &Path, _: &str) -> Result<()> {
    let project = Project::ready(directory)?;
    let run = pending_run(&project)?;
    let before = snapshot(&run)?;
    let status = project_status_with_run(&project.model, &run, project.options())?;
    validate_status(directory, &status)?;
    let expected = serde_json::to_value(run_status(&project.model, &run, None)?)?;
    let observed = status
        .workflow
        .as_ref()
        .context("missing projected workflow")?;
    for key in [
        "state",
        "acceptance_current",
        "revision",
        "available_packages",
    ] {
        ensure!(
            observed.get(key).context("missing workflow field")? == &expected[key],
            "projected workflow {key} differs from run status"
        );
    }
    ensure!(before == snapshot(&run)?, "journal changed during status");
    ensure!(status_text(&status).contains("work.arithmetic: pending"));
    Ok(())
}

pub(super) fn acceptance(directory: &Path, parameter: &str) -> Result<()> {
    let expected = match parameter {
        "current" => true,
        "stale" => false,
        _ => anyhow::bail!("unknown acceptance control {parameter}"),
    };
    let fixture = directory.join("../examples/agent-native");
    let temp = tempfile::tempdir()?;
    for name in ["model.gdd.json", "model.records.json", "REQUIREMENTS.md"] {
        fs::copy(fixture.join(name), temp.path().join(name))?;
    }
    let run = temp.path().join("run");
    fs::create_dir_all(run.join("events"))?;
    for (name, bytes) in snapshot(&fixture.join("native-run"))? {
        fs::write(run.join(name), bytes)?;
    }
    let host = input::parse(&fs::read(fixture.join("host.acceptance.json"))?)?;
    let path = temp.path().join("model.gdd.json");
    if !expected {
        let ledger = temp.path().join("model.records.json");
        let mut records = read_json(&ledger)?;
        records
            .get_mut("records")
            .and_then(serde_json::Value::as_object_mut)
            .context("missing records")?
            .remove("evidence.demo.final")
            .context("missing final acceptance evidence")?;
        write_json(&ledger, &records)?;
    }
    let assessor = run_status(&path, &run, Some(&host))?;
    ensure!(assessor.state.status == RunPhase::Accepted && assessor.event_count == 10);
    ensure!(
        assessor.acceptance_current == expected,
        "unexpected acceptance from workflow assessor"
    );
    let status = project_status_with_run(
        &path,
        &run,
        StatusOptions {
            host_context: Some(&host),
            ..Default::default()
        },
    )?;
    let workflow = status
        .workflow
        .as_ref()
        .context("missing accepted workflow")?;
    ensure!(workflow["state"]["status"] == "accepted");
    ensure!(workflow["acceptance_current"] == assessor.acceptance_current);
    ensure!(status_text(&status).contains(&format!("acceptance_current={expected}")));
    validate_status(directory, &status)?;
    Ok(())
}

pub(super) fn journal_errors(directory: &Path, _: &str) -> Result<()> {
    let project = Project::ready(directory)?;
    let invalid = project.temp.path().join("invalid");
    fs::create_dir(&invalid)?;
    fs::write(invalid.join("run.json"), "{}")?;
    for (run, expected, exit) in [
        (project.temp.path().join("absent"), "UNKNOWN", 2),
        (invalid, "FAIL", 1),
    ] {
        let status = project_status_with_run(&project.model, &run, project.options())?;
        let error = status
            .workflow_error
            .as_ref()
            .context("missing workflow error")?;
        ensure!(
            error["check"]["result"] == expected && status.exit_code == exit,
            "unexpected workflow verdict for {}",
            run.display()
        );
        validate_status(directory, &status)?;
    }
    Ok(())
}

pub(super) fn changed_revision(directory: &Path, _: &str) -> Result<()> {
    let project = Project::ready(directory)?;
    let run = pending_run(&project)?;
    let mut model = read_json(&project.model)?;
    *model
        .pointer_mut("/project/title")
        .context("missing project title")? = json!("Changed current model");
    let path = project.temp.path().join("changed.json");
    write_json(&path, &model)?;
    let status = project_status_with_run(&path, &run, project.options())?;
    expect_dimension(&status, "readiness", CheckResult::Pass)?;
    let workflow = status.workflow.as_ref().context("missing workflow")?;
    ensure!(workflow["acceptance_current"] == false && status.exit_code == 1);
    ensure!(
        workflow["report"]["checks"]
            .as_array()
            .is_some_and(|checks| checks.iter().any(|check| check["result"] == "FAIL"))
    );
    validate_status(directory, &status)?;
    Ok(())
}
