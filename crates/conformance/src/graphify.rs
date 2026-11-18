use anyhow::{Context, Result, ensure};
use gdd::graphify::{GraphifyOptions, decode_json, import_graphify, load_observations};
use gdd::input;
use gdd::report::CheckResult;
use gdd::validation::to_json;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Deserialize)]
struct Case {
    id: String,
    base: PathBuf,
    profile: String,
    #[serde(default)]
    patches: json_patch::Patch,
    condition: Option<String>,
    #[serde(default)]
    assertions: Counts,
    expected: Expected,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Counts {
    nodes: Option<usize>,
    edges: Option<usize>,
}

#[derive(Deserialize, Serialize)]
struct Expected {
    result: String,
    diagnostic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    check_id: Option<String>,
}

#[derive(Serialize)]
struct Run {
    format: &'static str,
    executed_at: String,
    total: usize,
    matched: usize,
    cases: Vec<Value>,
}

const PRECISION: &[u8] = br#"{"directed":true,"multigraph":false,"graph":{},"nodes":[{"id":9007199254740992,"file_type":"code","source_file":"a.py","label":"A"}],"links":[{"source":9007199254740993.0,"target":9007199254740992,"relation":"calls"}]}"#;

fn fixture(directory: &Path, case: &Case) -> Result<Vec<u8>> {
    let base = directory.join(&case.base);
    let source = fs::read(&base).with_context(|| format!("read {}", base.display()))?;
    let mut data = decode_json(&source).with_context(|| format!("parse {}", base.display()))?;
    for (index, op) in case.patches.iter().enumerate() {
        if let json_patch::PatchOperation::Test(test) = op {
            // RFC 6902 compares numbers by value, including different decimal spellings.
            ensure!(
                data.pointer(test.path.as_str())
                    .is_some_and(|value| jsonschema_value::cmp::equal(value, &test.value)),
                "fixture patch {index} failed test at {}",
                test.path
            );
        } else {
            // A failed operation discards this case's private value, so no rollback is needed.
            json_patch::patch_unsafe(&mut data, std::slice::from_ref(op))
                .with_context(|| format!("apply fixture patch {index}"))?;
        }
    }

    match case.condition.as_deref() {
        None => {}
        Some("precision") => return Ok(PRECISION.to_vec()),
        Some("duplicate_edge") => {
            let links = data
                .get_mut("links")
                .and_then(Value::as_array_mut)
                .context("expected links array")?;
            let first = links.first().context("expected first link")?.clone();
            links.push(first);
        }
        Some("both_arrays") => {
            let links = data["links"]
                .as_array()
                .context("expected links array")?
                .clone();
            data["edges"] = Value::Array(links);
        }
        Some("docs_only") => {
            for node in data
                .get_mut("nodes")
                .and_then(Value::as_array_mut)
                .context("expected nodes array")?
            {
                let node = node.as_object_mut().context("expected node object")?;
                node.insert("file_type".into(), "document".into());
                node.insert("source_file".into(), "SPEC.md".into());
            }
        }
        Some("hyperedges") => {
            let members = data["nodes"]
                .as_array()
                .context("expected nodes array")?
                .iter()
                .map(|node| node.get("id").context("expected node id"))
                .collect::<Result<Vec<_>>>()?;
            data["hyperedges"] = json!([{"members": members, "unknown": {"preserve": true}}]);
            data.get_mut("graph")
                .and_then(Value::as_object_mut)
                .context("expected graph object")?
                .insert(
                    "hyperedges".into(),
                    json!([{"different": "preserve independently"}]),
                );
        }
        Some(condition @ ("partial_markers" | "conflicting_markers")) => {
            let edge = data
                .pointer_mut("/links/0")
                .and_then(Value::as_object_mut)
                .context("expected first link object")?;
            let source = edge.get("source").context("expected source")?.clone();
            if condition == "partial_markers" {
                edge.insert("_src".into(), source);
            } else {
                let target = edge.get("target").context("expected target")?.clone();
                edge.insert("_src".into(), target);
                edge.insert("_tgt".into(), source);
            }
        }
        Some(unknown) => anyhow::bail!("unknown Graphify condition {unknown:?}"),
    }
    let mut bytes = serde_json::to_vec_pretty(&data)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn execute(directory: &Path, mut case: Case) -> Result<Value> {
    let content = fixture(directory, &case)?;
    let temp = tempfile::Builder::new()
        .prefix("gdd-graphify-case-")
        .tempdir()?;
    let source = temp.path().join("graph.json");
    let bundle = temp.path().join("bundle");
    fs::write(&source, &content)?;
    let report = import_graphify(
        &source,
        &bundle,
        GraphifyOptions {
            profile: Some(&case.profile),
            ..Default::default()
        },
    )?;
    let loaded = load_observations(&bundle)?;
    let index = loaded
        .index
        .context("cannot read the published observations index")?;
    let raw_preserved = loaded.raw.as_deref() == Some(content.as_slice());

    // The identical decimal input is outside the Python adapter's numeric range.
    if case.id == "case.graphify.precision"
        && case.condition.as_deref() == Some("precision")
        && case.expected.result == "UNKNOWN"
        && case.expected.diagnostic.as_deref() == Some("FORMAT_UNSUPPORTED")
    {
        case.expected.result = "FAIL".into();
        case.expected.diagnostic = Some("REFERENCE_UNRESOLVED".into());
    }

    let checks: Vec<_> = report
        .checks
        .iter()
        .filter(|check| {
            case.expected
                .check_id
                .as_ref()
                .is_none_or(|id| id == &check.check_id)
        })
        .collect();
    let actual = checks
        .iter()
        .filter(|check| check.dimension == "integration")
        .map(|check| check.result)
        .max()
        .unwrap_or(CheckResult::Unknown);
    let diagnostics: BTreeSet<_> = checks
        .iter()
        .flat_map(|check| &check.diagnostics)
        .map(|diagnostic| diagnostic.code)
        .collect();
    let mut errors = Vec::new();
    if let Some(id) = &case.expected.check_id
        && checks.is_empty()
    {
        errors.push(format!(
            "Import report contains no check with check_id {id:?}."
        ));
    }
    for (field, expected) in [
        ("nodes", case.assertions.nodes),
        ("edges", case.assertions.edges),
    ] {
        if let Some(expected) = expected {
            let actual = index[field]
                .as_array()
                .with_context(|| format!("missing {field} array"))?
                .len();
            if actual != expected {
                errors.push(format!("Expected {expected} {field}, found {actual}."));
            }
        }
    }
    let matched = raw_preserved
        && errors.is_empty()
        && actual.to_string() == case.expected.result
        && case
            .expected
            .diagnostic
            .as_deref()
            .is_none_or(|code| diagnostics.contains(code));

    let mut result = json!({ "id": case.id, "matched": matched, "expected": case.expected, "actual": actual,
        "diagnostics": diagnostics, "raw_preserved": raw_preserved, "report": report });
    if !errors.is_empty() {
        result["error"] = errors.join(" ").into();
    }
    Ok(result)
}

fn run(directory: &Path) -> Result<Run> {
    let manifest = directory.join("graphify-cases.yaml");
    let raw = fs::read(&manifest).with_context(|| format!("read {}", manifest.display()))?;
    let document = to_json(&input::parse(&raw)?)?;
    ensure!(
        document["format"] == "gdd-graphify-conformance-cases/0.2",
        "unsupported Graphify case manifest"
    );
    let definitions = document["cases"]
        .as_array()
        .context("expected cases array")?;
    ensure!(!definitions.is_empty(), "empty Graphify case manifest");
    let mut cases = Vec::with_capacity(definitions.len());
    for (i, definition) in definitions.iter().enumerate() {
        let result = serde_json::from_value(definition.clone())
            .context("read case definition")
            .and_then(|case| execute(directory, case));
        cases.push(result.unwrap_or_else(|error| {
            let id = definition["id"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("cases[{i}]"));
            json!({ "id": id, "matched": false, "error": format!("{error:#}") })
        }));
    }
    Ok(Run {
        format: "gdd-graphify-conformance-run/0.2",
        executed_at: OffsetDateTime::now_utc().format(&Rfc3339)?,
        total: cases.len(),
        matched: cases.iter().filter(|case| case["matched"] == true).count(),
        cases,
    })
}

pub(super) fn command(directory: &Path, output: Option<PathBuf>) -> Result<ExitCode> {
    let result = run(directory)?;
    if let Some(path) = output {
        let mut bytes = serde_json::to_vec_pretty(&result)?;
        bytes.push(b'\n');
        fs::write(&path, bytes).with_context(|| format!("write receipt {}", path.display()))?;
    }
    for case in &result.cases {
        if case["matched"] == false {
            println!("{case}");
        }
    }
    println!(
        "{}/{} Graphify adapter expectations matched",
        result.matched, result.total
    );
    Ok(if result.matched == result.total {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_graphify_cases() -> Result<()> {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance");
        let run = run(&directory)?;
        let mismatches: Vec<_> = run
            .cases
            .iter()
            .filter(|case| case["matched"] == false)
            .collect();
        assert!(mismatches.is_empty(), "{mismatches:#?}");
        assert_eq!(run.total, 18);
        assert_eq!(run.matched, run.total);

        // Bind the native precision expectation to the exact historical input bytes.
        let precision = run
            .cases
            .iter()
            .find(|case| case["id"] == "case.graphify.precision")
            .unwrap();
        assert_eq!(precision["actual"], "FAIL");
        assert_eq!(precision["diagnostics"], json!(["REFERENCE_UNRESOLVED"]));
        assert_eq!(precision["raw_preserved"], true);
        let raw = precision["report"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["check_id"] == "graphify.raw")
            .unwrap();
        assert!(
            raw["reason"].as_str().unwrap().contains(
                "sha256:2f48c4da0d0861b280a5c7e8e3614f094107ea46f3f5b0a1f43a43b8711cb88f"
            )
        );
        Ok(())
    }

    #[test]
    fn fixture_preserves_opaque_producer_keys_through_import() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let source = br#"{
  "nodes": [
    {
      "id": "a",
      "label": "A",
      "file_type": "code",
      "source_file": "a.rs"
    }
  ],
  "edges": [],
  "hyperedges": {
    "$serde_json::private::Number": {
      "<<": "01"
    }
  }
}
"#;
        fs::write(directory.path().join("opaque.json"), source)?;
        let case = serde_json::from_value(json!({
            "id": "opaque keys", "base": "opaque.json", "profile": "graphify-0.9.75-ast",
            "assertions": {"nodes": 1, "edges": 0}, "expected": {"result": "PASS"}
        }))?;
        assert_eq!(fixture(directory.path(), &case)?, source);
        let result = execute(directory.path(), case)?;
        assert_eq!(result["matched"], true, "{result}");
        assert_eq!(result["raw_preserved"], true);
        Ok(())
    }

    #[test]
    fn fixture_errors_and_wrong_expectations_leave_other_cases_runnable() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/graphify/export.directed.json");
        fs::copy(base, directory.path().join("base.json"))?;
        let case = json!({"base": "base.json", "profile": "graphify-0.9.75-node-link", "expected": {"result": "PASS"}});
        let mut unknown = case.clone();
        unknown["id"] = "unsupported condition".into();
        unknown["condition"] = "unrecognized".into();
        let mut bad_patch = case.clone();
        bad_patch["id"] = "invalid patch".into();
        bad_patch["patches"] = json!([
            {"op": "replace", "path": "/links/0/relation", "value": "temporary"},
            {"op": "move", "from": "/links", "path": "/absent/0"}
        ]);
        let mut wrong = case.clone();
        wrong["id"] = "wrong count".into();
        wrong["assertions"] = json!({"nodes": 4});
        let mut control = case;
        control["id"] = "control".into();
        control["patches"] = json!([{"op": "test", "path": "/links/0/weight", "value": 1}]);
        let mut wrong_result = control.clone();
        wrong_result["id"] = "wrong verdict".into();
        wrong_result["expected"]["result"] = "FAIL".into();
        let mut missing_check = control.clone();
        missing_check["id"] = "missing check".into();
        missing_check["expected"] =
            json!({"result": "UNKNOWN", "check_id": "graphify.misspelling"});
        let manifest = json!({"format":"gdd-graphify-conformance-cases/0.2",
            "cases": [unknown, bad_patch, wrong, wrong_result, missing_check, control]});
        fs::write(
            directory.path().join("graphify-cases.yaml"),
            serde_json::to_vec(&manifest)?,
        )?;

        let run = run(directory.path())?;
        assert_eq!((run.matched, run.total), (1, 6));
        assert!(
            run.cases[0]["error"]
                .as_str()
                .unwrap()
                .contains("unrecognized")
        );
        assert!(run.cases[1]["error"].as_str().unwrap().contains("/absent"));
        assert_eq!(run.cases[2]["actual"], "PASS");
        assert_eq!(run.cases[2]["matched"], false);
        assert_eq!(run.cases[3]["actual"], "PASS");
        assert_eq!(run.cases[3]["matched"], false);
        assert_eq!(run.cases[4]["actual"], "UNKNOWN");
        assert!(
            run.cases[4]["error"]
                .as_str()
                .unwrap()
                .contains("graphify.misspelling")
        );
        assert_eq!(run.cases[4]["matched"], false);
        assert_eq!(run.cases[5]["matched"], true);
        Ok(())
    }
}
