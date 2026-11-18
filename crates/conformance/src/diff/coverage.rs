//! Verify what the model diff can report for unavailable, invalid and partial inputs.

use super::{Pair, append_text, edges, remove_node};
use crate::fixture::write_json;
use anyhow::{Result, ensure};
use gdd::model_diff::{ModelDiff, diff_models, diff_text};
use serde_json::json;
use std::{fs, path::Path};

pub(super) fn validate(directory: &Path, report: &ModelDiff) -> Result<()> {
    let schema = serde_json::from_slice(&fs::read(
        directory.join("../schemas/model-diff.schema.json"),
    )?)?;
    let validator = gdd::validation::build_schema(&schema)?;
    validator
        .validate(&serde_json::to_value(report)?)
        .map_err(|e| anyhow::anyhow!("{e}"))
}

pub(super) fn semantic_unknown(directory: &Path, parameter: &str) -> Result<()> {
    let mut pair = Pair::new(directory)?;
    let source = json!({"role":"input", "title":"Source", "uri":"unused.txt", "format":"text/plain", "normative":false});
    let mut derived = source.clone();
    derived["derived_from"] = json!([{"source":"source.parent"}]);
    pair.model["sources"] = json!({"source.parent":source, "source.derived":derived});
    pair.model["nodes"]["component.unused"] = pair.model["nodes"]["component.arithmetic"].clone();
    pair.model["nodes"]["component.unused"]["interfaces"] = json!([]);
    let mut head = pair.model.clone();
    remove_node(&mut head, "component.unused")?;
    append_text(
        &mut head["nodes"]["action.add"]["behavior"],
        " Exact change.",
    )?;
    let expected_exit = if parameter == "invalid" {
        edges(&mut head)?.push(
            json!({"id":"edge.bad", "from":"action.add", "to":"action.absent", "relation":"calls"}),
        );
        1
    } else {
        2
    };
    let report = pair.compare(&head)?;
    ensure!(report.exit_code == expected_exit && report.comparison.coverage == "partial");
    ensure!(
        report
            .diagnostics
            .iter()
            .flat_map(|side| &side.diagnostics)
            .any(|d| d.code == "REVISION_UNRESOLVED")
    );
    ensure!(
        matches!(report.changes.as_slice(), [change] if change.id == "action.add" && change.change == "changed")
    );
    ensure!(report.advisories.is_empty());
    validate(directory, &report)
}

pub(super) fn partial(directory: &Path, _: &str) -> Result<()> {
    let pair = Pair::new(directory)?;
    let mut head = pair.model.clone();
    head["includes"] = json!(["missing.json"]);
    remove_node(&mut head, "scenario.add")?;
    append_text(&mut head["nodes"]["action.add"]["behavior"], " Exact text.")?;
    let report = pair.compare(&head)?;
    ensure!(report.comparison.coverage == "partial" && [1, 2].contains(&report.exit_code));
    ensure!(report.changes.iter().all(|c| c.change == "changed"));
    ensure!(report.advisories.is_empty() && report.metadata_changes.is_empty());
    ensure!(diff_text(&report).contains("absence claims"));
    Ok(())
}

pub(super) fn errors(directory: &Path, _: &str) -> Result<()> {
    let pair = Pair::new(directory)?;
    write_json(&pair.base, &pair.model)?;
    let missing = diff_models(&pair.base, &pair.directory.path().join("missing.json"))?;
    ensure!(missing.exit_code == 2 && missing.changes.is_empty());
    write_json(&pair.head, &json!({"format":"gdd/0.3"}))?;
    let invalid = diff_models(&pair.base, &pair.head)?;
    ensure!(invalid.exit_code == 1 && invalid.changes.is_empty());
    Ok(())
}

pub(super) fn schema(directory: &Path, _: &str) -> Result<()> {
    let pair = Pair::new(directory)?;
    write_json(&pair.base, &pair.model)?;
    validate(
        directory,
        &diff_models(&pair.base, &pair.directory.path().join("missing.json"))?,
    )?;
    let mut head = pair.model.clone();
    head["nodes"]["action.add"]["behavior"] = json!("Return exact sum.");
    validate(directory, &pair.compare(&head)?)?;
    validate(directory, &pair.compare(&json!({"format":"gdd/0.3"}))?)
}
