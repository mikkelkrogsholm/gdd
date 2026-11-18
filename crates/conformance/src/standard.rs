//! Execute the standard's named cases against independently prepared, local inputs.

mod conditions;
mod fixture;

use anyhow::{Context, Result, ensure};
use clap::ValueEnum;
use gdd::validation::{
    GraphifyValidationOptions, Profile, ValidationOptions, validate_paths_with_graphify,
};
use json_patch::Patch;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Deserialize)]
struct Case {
    id: String,
    base: PathBuf,
    profile: String,
    scope: Vec<String>,
    #[serde(default)]
    patches: Patch,
    condition: Option<String>,
    expected: Expected,
}

#[derive(Deserialize, Serialize)]
struct Expected {
    dimension: String,
    result: String,
    diagnostic: Option<String>,
}

#[derive(Serialize)]
struct Run {
    format: &'static str,
    tool: &'static str,
    executed_at: String,
    synthetic_inputs: bool,
    total: usize,
    matched: usize,
    cases: Vec<Value>,
}

fn execute(directory: &Path, case: &Case) -> Result<Value> {
    let temp = tempfile::Builder::new()
        .prefix("gdd-standard-case-")
        .tempdir()?;
    let examples = temp.path().join("examples");
    let base = case
        .base
        .strip_prefix("../examples")
        .context("expected a case base under ../examples/")?;
    ensure!(
        !base.as_os_str().is_empty()
            && base.components().all(|c| matches!(c, Component::Normal(_))),
        "expected a relative example path"
    );
    fixture::stage(&directory.join("../examples"), &examples, base)?;
    let mut path = examples.join(base);
    let mut data = fixture::read(&path)?;
    if !case.patches.is_empty() {
        fixture::patch(&mut data, &case.patches)?;
        fixture::write(&path, &data)?;
    }
    let inputs = conditions::prepare(case, &examples, &mut path, data)?;
    let profile = Profile::from_str(&case.profile, false).map_err(anyhow::Error::msg)?;
    let options = ValidationOptions {
        profiles: &[profile],
        scope: &case.scope,
        activity: inputs.activity,
        unsupported_extensions: &inputs.unsupported_extensions,
        host_context: inputs.host.as_ref(),
        parallel_plan: inputs.parallel.as_ref(),
        package: inputs.package.as_ref(),
    };
    let integration = GraphifyValidationOptions {
        profile: inputs.graphify_profile,
        import_audit: inputs.audit.as_ref(),
        ..Default::default()
    };
    let report = validate_paths_with_graphify(&path, None, options, integration, None)?;
    let expected = &case.expected;
    let actual = report.result(&expected.dimension);
    let diagnostics: BTreeSet<_> = report
        .checks
        .iter()
        .filter(|check| check.dimension == expected.dimension)
        .flat_map(|check| &check.diagnostics)
        .map(|diagnostic| diagnostic.code)
        .collect();
    let matched = actual.to_string() == expected.result
        && expected
            .diagnostic
            .as_deref()
            .is_none_or(|code| diagnostics.contains(code));
    Ok(
        json!({"id": case.id, "matched": matched, "expected": expected, "actual": actual,
        "diagnostics": diagnostics, "report": report}),
    )
}

fn run(directory: &Path) -> Result<Run> {
    let manifest = fixture::read(&directory.join("cases.yaml"))?;
    ensure!(
        manifest["format"] == "gdd-conformance-cases/0.2",
        "unsupported standard case manifest"
    );
    let definitions = manifest["cases"]
        .as_array()
        .context("expected standard cases array")?;
    ensure!(!definitions.is_empty(), "empty standard case manifest");
    let mut cases = Vec::with_capacity(definitions.len());
    for (index, definition) in definitions.iter().enumerate() {
        let result = serde_json::from_value::<Case>(definition.clone())
            .context("read standard case definition")
            .and_then(|case| execute(directory, &case));
        cases.push(result.unwrap_or_else(|error| {
            let id = definition["id"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("cases[{index}]"));
            json!({"id": id, "matched": false, "error": format!("{error:#}")})
        }));
    }
    Ok(Run {
        format: "gdd-conformance-run/0.2",
        tool: concat!("gdd-conformance/", env!("CARGO_PKG_VERSION")),
        executed_at: OffsetDateTime::now_utc().format(&Rfc3339)?,
        synthetic_inputs: true,
        total: cases.len(),
        matched: cases.iter().filter(|row| row["matched"] == true).count(),
        cases,
    })
}

pub(super) fn command(directory: &Path, output: Option<PathBuf>) -> Result<ExitCode> {
    let run = run(directory)?;
    crate::receipt::publish(&run, output, "standard", run.matched, run.total)
}

#[cfg(test)]
mod tests;
