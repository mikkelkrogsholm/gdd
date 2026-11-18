//! Run declared-model impact behaviors against private tutorial models.

mod graph;
mod records;

use crate::fixture::{snapshot, write_json};
use crate::named::{self, Behavior};
use anyhow::{Context, Result, ensure};
use gdd::impact::{ImpactItem, ImpactReport, analyze_impact, impact_text};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

macro_rules! impact_case {
    ($id:ident, $reference:ident, $execute:path, $parameters:expr) => {
        Behavior {
            id: concat!("impact.", stringify!($id)),
            reference: concat!("tests/test_impact.py::test_", stringify!($reference)),
            execute: $execute,
            parameters: $parameters,
        }
    };
    ($id:ident, $reference:ident, $execute:path) => {
        impact_case!($id, $reference, $execute, &["default"])
    };
}

const BEHAVIORS: &[Behavior] = &[
    impact_case!(reverse, reverse_paths_and_supporting_owner, graph::reverse),
    impact_case!(
        cycles,
        cycles_and_multiple_roots_are_deterministic,
        graph::cycles
    ),
    impact_case!(
        scopes,
        invalid_scopes_visible,
        invalid_scope,
        &[
            "[]",
            r#"["unknown.id"]"#,
            r#"["action.add\n"]"#,
            r#""action.add""#,
            "[42]",
            r#"["edge.caller"]"#
        ]
    ),
    impact_case!(mixed, mixed_scope_retains_valid_root, mixed),
    impact_case!(readonly, read_only_text_and_schema, readonly),
    impact_case!(
        closure,
        context_paths_match_existing_closure,
        graph::closure
    ),
    impact_case!(
        extension,
        extension_context_does_not_invent_dependency,
        graph::extension,
        &["false", "true"]
    ),
    impact_case!(
        unknown,
        unknown_extension_is_visible_partial_semantics,
        graph::unknown_extension
    ),
    impact_case!(
        records,
        current_records_selected_history_never_rebound,
        records::history
    ),
    impact_case!(
        types,
        type_reference_consumers_have_semantic_witness,
        graph::types
    ),
    impact_case!(
        entities,
        entity_consumers_reverse_only,
        graph::entities,
        &["reads", "creates", "changes"]
    ),
    impact_case!(
        navigation,
        navigation_consumer_reverse_only,
        graph::navigation
    ),
    impact_case!(
        incomplete,
        explicitly_incomplete_dependencies_partial,
        incomplete
    ),
    impact_case!(malformed, unsupported_and_malformed_inputs, malformed),
    impact_case!(
        unspecified,
        missing_completeness_does_not_require_readiness,
        unspecified
    ),
    impact_case!(
        check_supersession,
        check_plan_supersession_respects_existing_compatibility,
        records::check_supersession,
        &[
            "compatible",
            "different_method",
            "narrower_scope",
            "invalid"
        ]
    ),
    impact_case!(
        binding_supersession,
        binding_supersession_not_retired_without_verified_host_trust,
        records::binding_supersession
    ),
];

pub(super) fn command(directory: &Path, output: Option<PathBuf>) -> Result<ExitCode> {
    named::command(
        directory,
        output,
        "impact",
        "gdd-impact-conformance/0.1",
        BEHAVIORS,
        (),
    )
}

fn edge(id: &str, from: &str, relation: &str, to: &str) -> Value {
    json!({"id":id, "from":from, "to":to, "relation":relation})
}

struct Project {
    temp: tempfile::TempDir,
    path: PathBuf,
    model: Value,
}

impl Project {
    fn new(directory: &Path) -> Result<Self> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("model.json");
        let bytes = fs::read(directory.join("../docs/tutorial/refined.gdd.json"))
            .context("read the refined tutorial model")?;
        let mut model: Value = serde_json::from_slice(&bytes)?;
        let action = model
            .pointer("/nodes/action.add")
            .context("missing action.add")?
            .clone();
        let nodes = model["nodes"]
            .as_object_mut()
            .context("expected model nodes")?;
        for name in ["caller", "outer", "sibling"] {
            nodes.insert(format!("action.{name}"), action.clone());
        }
        let mut project = Self { temp, path, model };
        project.edges()?.extend([
            edge("edge.caller", "action.caller", "calls", "action.add"),
            edge("edge.outer", "action.outer", "calls", "action.caller"),
            edge(
                "edge.sibling",
                "action.sibling",
                "owned_by",
                "component.arithmetic",
            ),
        ]);
        Ok(project)
    }

    fn edges(&mut self) -> Result<&mut Vec<Value>> {
        self.model["edges"]
            .as_array_mut()
            .context("expected model edges")
    }

    fn save(&self) -> Result<&Path> {
        write_json(&self.path, &self.model)?;
        Ok(&self.path)
    }
}

fn analyze(path: &Path, seeds: &[&str]) -> Result<ImpactReport> {
    let scope: Vec<String> = seeds.iter().map(|s| (*s).to_owned()).collect();
    Ok(analyze_impact(path, &scope, None)?)
}

fn item<'a>(report: &'a ImpactReport, root: &str, id: &str) -> Result<&'a ImpactItem> {
    report
        .roots
        .iter()
        .find(|r| r.id == root)
        .and_then(|r| r.items.iter().find(|i| i.id == id))
        .with_context(|| {
            format!(
                "missing {id} under {root}, diagnostics: {:?}",
                report.diagnostics
            )
        })
}

fn invalid_scope(directory: &Path, parameter: &str) -> Result<()> {
    let raw: Value = serde_json::from_str(parameter)?;
    let scope = serde_json::from_value::<Vec<String>>(raw);
    if matches!(parameter, r#""action.add""# | "[42]") {
        // These Python inputs cannot cross the native API's &[String] boundary.
        ensure!(scope.is_err(), "ill-typed scope was accepted");
    } else {
        let project = Project::new(directory)?;
        let report = analyze_impact(project.save()?, &scope?, None)?;
        ensure!(report.exit_code == 1 && !report.diagnostics.is_empty());
        ensure!(report.coverage.result != "complete");
    }
    Ok(())
}

fn mixed(directory: &Path, _: &str) -> Result<()> {
    let project = Project::new(directory)?;
    let report = analyze(project.save()?, &["action.add", "unknown.id"])?;
    ensure!(report.exit_code == 1 && report.coverage.result == "partial");
    ensure!(report.roots.len() == 1 && report.roots[0].id == "action.add");
    Ok(())
}

fn readonly(directory: &Path, _: &str) -> Result<()> {
    let project = Project::new(directory)?;
    let path = project.save()?;
    let before = snapshot(project.temp.path())?;
    let report = analyze(path, &["action.add"])?;
    ensure!(
        snapshot(project.temp.path())? == before && fs::read_dir(project.temp.path())?.count() == 1
    );
    ensure!(report.exit_code == 0 && report.roots.len() == 1);
    let text = impact_text(&report);
    ensure!(text.ends_with('\n') && text.contains("runtime callers unassessed"));
    ensure!(
        report.roots[0]
            .items
            .iter()
            .all(|item| text.contains(&item.id))
    );
    let schema =
        serde_json::from_slice(&fs::read(directory.join("../schemas/impact.schema.json"))?)?;
    let validator = gdd::validation::build_schema(&schema)?;
    let value = serde_json::to_value(report)?;
    validator
        .validate(&value)
        .map_err(|error| anyhow::anyhow!("{error}"))
}

fn incomplete(directory: &Path, _: &str) -> Result<()> {
    let mut project = Project::new(directory)?;
    project.model["nodes"]["action.caller"]["contract"]["dependencies_complete"] = json!(false);
    let report = analyze(project.save()?, &["action.add"])?;
    ensure!(report.exit_code == 2 && report.coverage.result == "partial");
    ensure!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "IMPACT_DEPENDENCIES_INCOMPLETE")
    );
    ensure!(item(&report, "action.add", "action.caller")?.category == "direct");
    Ok(())
}

fn unspecified(directory: &Path, _: &str) -> Result<()> {
    let mut project = Project::new(directory)?;
    project.model["nodes"]["action.add"]
        .as_object_mut()
        .context("expected action")?
        .remove("contract")
        .context("missing tutorial contract")?;
    let report = analyze(project.save()?, &["action.add"])?;
    ensure!(report.exit_code == 0 && report.coverage.unlisted_dependencies == "unassessed");
    ensure!(
        report
            .coverage
            .unspecified_declarations
            .iter()
            .any(|id| id == "action.add")
    );
    Ok(())
}

fn malformed(_: &Path, _: &str) -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("bad.json");
    for bytes in [r#"{"format":"gdd/future"}"#, "{"] {
        fs::write(&path, bytes)?;
        let report = analyze(&path, &["action.add"])?;
        ensure!([1, 2].contains(&report.exit_code) && report.coverage.result == "unavailable");
        ensure!(!report.diagnostics.is_empty() && fs::read(&path)? == bytes.as_bytes());
    }
    Ok(())
}
