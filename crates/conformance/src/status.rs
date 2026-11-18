//! Run the named project status cases against native assessors and private fixtures.

mod workflow;

use crate::fixture::write_json;
use crate::named::{self, Behavior};
use anyhow::{Context, Result, ensure};
use gdd::input::Value as Input;
use gdd::report::CheckResult;
use gdd::status::{ProjectStatus, StatusOptions, project_status, status_text};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

macro_rules! status_case {
    ($name:ident, $execute:path) => {
        Behavior {
            id: concat!("status.", stringify!($name)),
            reference: concat!("tests/test_project_status.py::test_", stringify!($name)),
            parameters: &["default"],
            execute: $execute,
        }
    };
}

const BEHAVIORS: &[Behavior] = &[
    Behavior {
        id: "status.integration_exit_policy",
        reference: "tests/test_project_status.py::test_assessed_integration_checks_affect_exit_without_erasing_readiness",
        parameters: &["https://example.invalid/spec", "../outside.txt"],
        execute: integration_exit_policy,
    },
    status_case!(
        starter_questions_and_blockers_are_usable_without_host,
        starter_questions
    ),
    status_case!(
        ready_scope_does_not_imply_authority_or_evidence,
        ready_scope
    ),
    status_case!(explicit_empty_host_is_assessed_unresolved, empty_host),
    status_case!(known_failure_outranks_unknown, known_failure),
    status_case!(
        inputs_remain_explicit_and_cannot_silently_override_trust,
        explicit_inputs
    ),
    status_case!(actions_are_deterministic_bounded_and_traceable, actions),
    status_case!(
        optional_run_exactly_projects_existing_status_without_journal_writes,
        workflow::optional_run
    ),
    Behavior {
        id: "status.recorded_acceptance_is_only_current_when_existing_assessor_says_so",
        reference: "tests/test_project_status.py::test_recorded_acceptance_is_only_current_when_existing_assessor_says_so",
        parameters: &["current", "stale"],
        execute: workflow::acceptance,
    },
    status_case!(
        missing_run_is_unknown_and_malformed_journal_is_fail,
        workflow::journal_errors
    ),
    status_case!(
        unrelated_historical_record_does_not_erase_readiness,
        historical_record
    ),
    status_case!(
        run_model_revision_is_not_silently_rebound,
        workflow::changed_revision
    ),
    status_case!(
        explicit_code_identity_never_manufactures_implementation_evidence,
        explicit_identity
    ),
];

pub(super) fn command(directory: &Path, output: Option<PathBuf>) -> Result<ExitCode> {
    named::command(
        directory,
        output,
        "status",
        "gdd-status-conformance/0.1",
        BEHAVIORS,
        (),
    )
}

struct Project {
    temp: tempfile::TempDir,
    model: PathBuf,
    scope: [String; 1],
}

impl Project {
    fn ready(directory: &Path) -> Result<Self> {
        let temp = tempfile::tempdir()?;
        let model = temp.path().join("model.json");
        fs::copy(directory.join("../docs/tutorial/refined.gdd.json"), &model)
            .context("copy ready tutorial model")?;
        Ok(Self {
            temp,
            model,
            scope: ["action.add".into()],
        })
    }

    fn starter() -> Result<Self> {
        let temp = tempfile::tempdir()?;
        let project = temp.path().join("project");
        gdd::init_project(&project, gdd::DEFAULT_PROJECT_ID, gdd::DEFAULT_TITLE)?;
        Ok(Self {
            temp,
            model: project.join("gdd.json"),
            scope: ["action.main".into()],
        })
    }

    fn options(&self) -> StatusOptions<'_> {
        StatusOptions {
            scope: &self.scope,
            ..Default::default()
        }
    }

    fn status(&self) -> Result<ProjectStatus> {
        Ok(project_status(&self.model, self.options())?)
    }
}

fn read_json(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

fn validate_status(directory: &Path, status: &ProjectStatus) -> Result<Value> {
    let schemas = directory.join("../schemas");
    let report = read_json(&schemas.join("report.schema.json"))?;
    let id = report["$id"]
        .as_str()
        .context("missing report schema ID")?
        .to_owned();
    let registry = jsonschema::Registry::new().add(&id, report)?.prepare()?;
    let schema = read_json(&schemas.join("status.schema.json"))?;
    let validator = jsonschema::draft202012::options()
        .offline()
        .with_registry(&registry)
        .build(&schema)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let value = serde_json::to_value(status)?;
    validator
        .validate(&value)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(value)
}

fn expect_dimension(status: &ProjectStatus, dimension: &str, expected: CheckResult) -> Result<()> {
    let actual = status
        .assessments
        .get(dimension)
        .context("missing assessment")?;
    ensure!(
        actual.assessed && actual.result == Some(expected),
        "{dimension}: expected {expected}, got {actual:?}"
    );
    Ok(())
}

fn integration_exit_policy(directory: &Path, uri: &str) -> Result<()> {
    let project = Project::ready(directory)?;
    let mut model = read_json(&project.model)?;
    let sources = model
        .as_object_mut()
        .context("expected tutorial model object")?
        .entry("sources")
        .or_insert_with(|| json!({}));
    let sources = sources
        .as_object_mut()
        .context("expected model sources object")?;
    sources.insert(
        "source.normative".into(),
        json!({
            "role":"knowledge", "title":"Normative input", "uri":uri, "format":"text/plain",
            "normative":true, "revision":{"sha256":"0".repeat(64)}
        }),
    );
    write_json(&project.model, &model)?;
    let status = project.status()?;
    validate_status(directory, &status)?;
    expect_dimension(&status, "readiness", CheckResult::Pass)?;
    let (result, exit) = match uri {
        "https://example.invalid/spec" => (CheckResult::Unknown, 2),
        "../outside.txt" => (CheckResult::Fail, 1),
        _ => anyhow::bail!("unknown integration input {uri}"),
    };
    ensure!(status.spec_revision.is_none() && status.exit_code == exit);
    ensure!(
        status
            .reports
            .get("model")
            .context("missing model report")?
            .checks
            .iter()
            .any(|c| c.dimension == "integration" && c.result == result)
    );
    Ok(())
}

fn starter_questions(directory: &Path, _: &str) -> Result<()> {
    let project = Project::starter()?;
    let before = fs::read(&project.model)?;
    let status = project.status()?;
    validate_status(directory, &status)?;
    expect_dimension(&status, "model", CheckResult::Pass)?;
    expect_dimension(&status, "readiness", CheckResult::Fail)?;
    ensure!(
        status
            .open_questions
            .first()
            .is_some_and(|q| !q.id.is_empty())
    );
    ensure!(status.workflow.is_none() && status.exit_code == 1);
    ensure!(
        before == fs::read(&project.model)?,
        "status changed the starter model"
    );
    Ok(())
}

fn ready_scope(directory: &Path, _: &str) -> Result<()> {
    let project = Project::ready(directory)?;
    let status = project.status()?;
    let value = validate_status(directory, &status)?;
    for dimension in ["model", "readiness"] {
        expect_dimension(&status, dimension, CheckResult::Pass)?;
    }
    for dimension in ["authorization", "implementation"] {
        ensure!(
            value["assessments"][dimension]
                == json!({"assessed":false,"result":null,"check_refs":[]}),
            "unexpected assessment for {dimension}"
        );
    }
    ensure!(
        status.exit_code == 0 && value.get("complete").is_none() && value.get("result").is_none()
    );
    Ok(())
}

fn empty_host(directory: &Path, _: &str) -> Result<()> {
    let project = Project::ready(directory)?;
    let status = project_status(
        &project.model,
        StatusOptions {
            host_context: Some(&Input::from(&json!({}))),
            ..project.options()
        },
    )?;
    validate_status(directory, &status)?;
    expect_dimension(&status, "authorization", CheckResult::Unresolved)?;
    expect_dimension(&status, "implementation", CheckResult::Unknown)?;
    ensure!(status.exit_code == 2);
    Ok(())
}

fn known_failure(_: &Path, _: &str) -> Result<()> {
    let project = Project::starter()?;
    let status = project_status(
        &project.model,
        StatusOptions {
            host_context: Some(&Input::from(&json!({}))),
            ..project.options()
        },
    )?;
    expect_dimension(&status, "readiness", CheckResult::Fail)?;
    expect_dimension(&status, "authorization", CheckResult::Unresolved)?;
    ensure!(status.exit_code == 1);
    Ok(())
}

fn explicit_inputs(directory: &Path, _: &str) -> Result<()> {
    let project = Project::ready(directory)?;
    let implementation = "implementation.example";
    let host = Input::from(&json!({"implementation":implementation, "code_revision":"revision-a"}));
    let original = host.clone();
    let status = project_status(
        &project.model,
        StatusOptions {
            host_context: Some(&host),
            implementation: Some(implementation),
            ..project.options()
        },
    )?;
    ensure!(status.inputs.code_revision.as_deref() == Some("revision-a") && host == original);
    let conflict = project_status(
        &project.model,
        StatusOptions {
            host_context: Some(&host),
            code_revision: Some("revision-b"),
            ..Default::default()
        },
    )
    .err()
    .context("conflicting revisions were accepted")?;
    ensure!(conflict.to_string().contains("conflicts"));
    // Scope is typed as strings in Rust. Exercise rejection of the invalid ID "1".
    ensure!(
        project_status(
            &project.model,
            StatusOptions {
                scope: &["1".into(), "action.add".into()],
                ..Default::default()
            }
        )
        .is_err()
    );
    let bad_host = project_status(
        &project.model,
        StatusOptions {
            host_context: Some(&Input::Array(vec![])),
            implementation: Some(implementation),
            ..Default::default()
        },
    )
    .err()
    .context("array host was accepted")?;
    ensure!(bad_host.to_string().contains("host input"));
    Ok(())
}

fn actions(_: &Path, _: &str) -> Result<()> {
    let project = Project::starter()?;
    let status = project.status()?;
    let value = serde_json::to_value(&status)?;
    let repeated = project_status(
        &project.model,
        StatusOptions {
            scope: &["action.main".into(), "action.main".into()],
            ..Default::default()
        },
    )?;
    ensure!(
        value == serde_json::to_value(repeated)?,
        "duplicate scope changed status"
    );
    ensure!((1..=12).contains(&status.next_actions.len()));
    let text = status_text(&status);
    for action in &status.next_actions {
        let source = value
            .pointer(&action.source)
            .context("action source pointer did not resolve")?;
        if let Some(code) = action.diagnostic_code {
            ensure!(source["code"] == code && source["message"] == action.reason);
        } else {
            ensure!(
                source
                    .as_str()
                    .is_some_and(|s| status.inputs.missing.contains(&s))
            );
        }
        ensure!(action.argv.first().is_some_and(|s| s == "gdd"));
        ensure!(text.contains(&action.source) && text.contains(&action.reason));
    }
    for (name, assessment) in &status.assessments {
        let expected = if assessment.assessed {
            assessment
                .result
                .context("missing assessed result")?
                .to_string()
        } else {
            "UNASSESSED".into()
        };
        ensure!(text.contains(&format!("{name}: {expected}")));
    }
    ensure!(text.ends_with('\n') && text.contains("Exit code: 1"));
    Ok(())
}

fn historical_record(directory: &Path, _: &str) -> Result<()> {
    let project = Project::ready(directory)?;
    let mut model = read_json(&project.model)?;
    model
        .as_object_mut()
        .context("expected tutorial model object")?
        .insert("record_files".into(), json!(["records.json"]));
    write_json(&project.model, &model)?;
    let records = json!({"format":"gdd-records/0.2","project_id":model["project"]["id"],"records":{
        "binding.old":{"kind":"binding","implementation":"implementation.old",
            "spec_revision":format!("sha256:{}", "0".repeat(64)), "targets":["action.retired"],
            "role":"code","resource":"old.py","selector":{"kind":"whole-document"},"status":"planned"}
    }});
    write_json(&project.temp.path().join("records.json"), &records)?;
    let status = project.status()?;
    expect_dimension(&status, "model", CheckResult::Unknown)?;
    expect_dimension(&status, "readiness", CheckResult::Pass)?;
    ensure!(status.exit_code == 2);
    ensure!(
        status
            .reports
            .get("model")
            .context("missing model report")?
            .checks
            .iter()
            .flat_map(|c| &c.diagnostics)
            .any(|d| d.related_ids == ["binding.old"])
    );
    Ok(())
}

fn explicit_identity(directory: &Path, _: &str) -> Result<()> {
    let project = Project::ready(directory)?;
    let status = project_status(
        &project.model,
        StatusOptions {
            implementation: Some("implementation.example"),
            code_revision: Some("claimed"),
            ..project.options()
        },
    )?;
    expect_dimension(&status, "implementation", CheckResult::Unknown)?;
    expect_dimension(&status, "authorization", CheckResult::Unresolved)?;
    ensure!(status.inputs.implementation.as_deref() == Some("implementation.example"));
    ensure!(status.inputs.code_revision.as_deref() == Some("claimed"));
    ensure!(status.inputs.missing.contains(&"host_context"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_manifest_entries_and_unavailable_inputs_fail_the_suite() -> Result<()> {
        let root = tempfile::tempdir()?;
        let directory = root.path().join("conformance");
        fs::create_dir(&directory)?;
        let output = root.path().join("receipt.json");
        let run = || command(&directory, Some(output.clone()));
        let manifest = directory.join("status.cases.json");
        ensure!(run().is_err());
        write_json(&manifest, &json!([]))?;
        ensure!(run().is_err() && !output.exists());

        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/status.cases.json"),
            &manifest,
        )?;
        ensure!(run()? == ExitCode::FAILURE);
        let failed = read_json(&output)?;
        ensure!(failed["passed"] == false && failed["test_instances"] == 15);
        let integration = &failed["cases"][0];
        ensure!(integration["id"] == "status.integration_exit_policy");
        ensure!(integration["result"] == "FAIL" && integration["test_instances"] == 2);
        ensure!(
            integration["errors"]
                .as_array()
                .is_some_and(|errors| errors.len() == 2)
        );
        Ok(())
    }
}
