//! Replay the named artifact cases against the passive native view profile.

use anyhow::{Context, Result, bail, ensure};
use gdd::view::{PROFILE, render_view, verify_view};
use gdd::{input, validation::to_json};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Deserialize)]
struct Case {
    id: String,
    base: String,
    operation: String,
    expected: String,
}

#[derive(Serialize)]
struct Run {
    format: &'static str,
    profile: &'static str,
    executed_at: String,
    total: usize,
    matched: usize,
    results: Vec<Value>,
}

fn encode_artifact(data: &Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(data)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn mutate_payload(operation: &str, original: &[u8]) -> Result<Vec<u8>> {
    let mut data: Value = serde_json::from_slice(original)?;
    ensure!(
        encode_artifact(&data)? == original,
        "fixture round trip changed view bytes"
    );
    match operation {
        "renderer" => data["generator"]["version"] = "unsupported".into(),
        "revision" => data["spec_revision"] = format!("sha256:{}", "0".repeat(64)).into(),
        "manifest" => data["manifest"]["shown_edges"] = json!([]),
        "direction" | "omit_edge" => {
            let edges = data["model"]["edges"]
                .as_array_mut()
                .context("expected model edges")?;
            let id = if operation == "direction" {
                "edge.call_one"
            } else {
                "edge.call_two"
            };
            let index = edges
                .iter()
                .position(|edge| edge["id"] == id)
                .with_context(|| format!("missing parallel edge {id}"))?;
            ensure!(
                edges[index]["from"] == "constructor" && edges[index]["to"] == "action.second",
                "unexpected endpoints on {id}"
            );
            if operation == "direction" {
                edges[index]["from"] = "action.second".into();
                edges[index]["to"] = "constructor".into();
            } else {
                edges.remove(index);
            }
        }
        unknown => bail!("unknown visualization case operation {unknown:?}"),
    }
    let modified = encode_artifact(&data)?;
    ensure!(
        modified != original,
        "operation {operation} left the artifact unchanged"
    );
    Ok(modified)
}

fn mutate_artifact(operation: &str, path: &Path) -> Result<()> {
    match operation {
        "none" | "scope" | "missing" => return Ok(()),
        "unavailable" => return Ok(fs::remove_file(path)?),
        _ => {}
    }
    let mut bytes = fs::read(path)?;
    match operation {
        "script" => bytes.extend_from_slice(b"Unauthorized trailing content.\n"),
        "blank" => bytes.clear(),
        "crlf" => {
            ensure!(bytes.ends_with(b"\n"), "expected a final LF");
            bytes = std::str::from_utf8(&bytes)?
                .replace('\n', "\r\n")
                .into_bytes();
        }
        "deep" => {
            ensure!(bytes.ends_with(b"}\n"), "expected a complete view object");
            bytes.truncate(bytes.len() - 2);
            bytes.extend_from_slice(b",\"nested\":");
            bytes.extend(std::iter::repeat_n(b'[', 2000));
            bytes.push(b'0');
            bytes.extend(std::iter::repeat_n(b']', 2000));
            bytes.extend_from_slice(b"}\n");
        }
        structured => bytes = mutate_payload(structured, &bytes)?,
    }
    Ok(fs::write(path, bytes)?)
}

fn visualization_check(report: &Value) -> Result<&Value> {
    let checks = report["checks"]
        .as_array()
        .context("expected verification checks")?;
    let [check] = checks.as_slice() else {
        bail!("expected a single native visualization check");
    };
    ensure!(
        check["check_id"] == "integration.visualization" && check["dimension"] == "integration",
        "missing integration.visualization check"
    );
    Ok(check)
}

fn execute(directory: &Path, case: &Case) -> Result<Value> {
    let temp = tempfile::Builder::new()
        .prefix("gdd-view-case-")
        .tempdir()?;
    let model = if case.base == "fixture" {
        let model = temp.path().join("model.json");
        fs::copy(directory.join("fixtures/viewer-multigraph.json"), &model)
            .context("copy viewer multigraph fixture")?;
        model
    } else {
        directory
            .join("../examples")
            .join(format!("{}.gdd.yaml", case.base))
    };
    let artifact = temp.path().join("graph.gddview");
    let scope = if case.operation == "scope" {
        vec!["action.reserve".to_owned()]
    } else {
        vec![]
    };
    render_view(&model, &artifact, &scope)
        .with_context(|| format!("render {}", model.display()))?;
    if !matches!(case.operation.as_str(), "none" | "scope") {
        let baseline = verify_view(&model, Some(&artifact))?;
        ensure!(
            visualization_check(&baseline)?["result"] == "PASS",
            "original artifact failed verification: {baseline}"
        );
    }
    mutate_artifact(&case.operation, &artifact)?;
    let input = (case.operation != "missing").then_some(artifact.as_path());
    let report = verify_view(&model, input)?;
    let check = visualization_check(&report)?;
    let actual = check["result"]
        .as_str()
        .context("expected a verification verdict")?;
    let scope = json!(scope);
    let scope_matches = actual != "PASS" || (check["scope"] == scope && report["scope"] == scope);
    let mut row = json!({"id": case.id, "base": case.base, "operation": case.operation,
        "scope": scope, "expected": case.expected, "actual": actual,
        "matched": actual == case.expected && scope_matches, "report": report});
    if !scope_matches {
        row["error"] = "Verification scope differs from the rendered scope.".into();
    }
    Ok(row)
}

fn run(directory: &Path) -> Result<Run> {
    let manifest = directory.join("visualization-cases.yaml");
    let bytes = fs::read(&manifest).with_context(|| format!("read {}", manifest.display()))?;
    let data = to_json(&input::parse(&bytes)?)?;
    let cases = data["cases"]
        .as_array()
        .context("expected visualization cases")?;
    ensure!(!cases.is_empty(), "empty visualization case manifest");
    let mut results = Vec::with_capacity(cases.len());
    for (index, definition) in cases.iter().enumerate() {
        let outcome = serde_json::from_value::<Case>(definition.clone())
            .context("read visualization case definition")
            .and_then(|case| execute(directory, &case));
        results.push(outcome.unwrap_or_else(|error| {
            let id = definition["id"].as_str().map(str::to_owned)
                .unwrap_or_else(|| format!("cases[{index}]"));
            json!({"id": id, "base": definition["base"], "operation": definition["operation"],
                "expected": definition["expected"], "matched": false, "error": format!("{error:#}")})
        }));
    }
    Ok(Run {
        format: "gdd-view-conformance/0.1",
        profile: PROFILE,
        executed_at: OffsetDateTime::now_utc().format(&Rfc3339)?,
        total: results.len(),
        matched: results.iter().filter(|row| row["matched"] == true).count(),
        results,
    })
}

pub(super) fn command(directory: &Path, output: Option<PathBuf>) -> Result<ExitCode> {
    let run = run(directory)?;
    crate::receipt::publish(&run, output, "visualization", run.matched, run.total)
}

#[cfg(test)]
mod tests;
