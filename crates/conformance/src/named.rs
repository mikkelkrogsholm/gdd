//! Execute a named catalog of parameterized conformance behaviors.

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

#[derive(Deserialize)]
struct Case {
    id: String,
    assertion: String,
    test: String,
}

pub(super) struct Behavior {
    pub id: &'static str,
    pub reference: &'static str,
    pub parameters: &'static [&'static str],
    pub execute: fn(&Path, &str) -> Result<()>,
}

#[derive(Serialize)]
struct Row {
    id: String,
    assertion: String,
    result: &'static str,
    test_instances: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    errors: Vec<String>,
}

#[derive(Serialize)]
struct Run<P> {
    format: &'static str,
    tool: &'static str,
    runner: &'static str,
    #[serde(flatten)]
    profile: P,
    case_count: usize,
    test_instances: usize,
    passed: bool,
    cases: Vec<Row>,
}

fn execute(
    directory: &Path,
    suite: &str,
    behaviors: &[Behavior],
    case: Case,
    seen: &mut BTreeSet<String>,
) -> Row {
    let mut errors = Vec::new();
    let mut count = 0;
    let behavior = behaviors.iter().find(|b| b.id == case.id);
    if !seen.insert(case.id.clone()) {
        errors.push(format!("Duplicate case ID {}", case.id));
    } else if case.assertion.trim().is_empty() {
        errors.push("Missing case assertion".to_owned());
    } else if let Some(behavior) = behavior {
        if case.test != behavior.reference {
            errors.push(format!("Unrecognized test reference {}", case.test));
        } else {
            for parameter in behavior.parameters {
                count += 1;
                if let Err(error) = (behavior.execute)(directory, parameter) {
                    errors.push(format!("{parameter}: {error:#}"));
                }
            }
        }
    } else {
        errors.push(format!("Unknown {suite} case {}", case.id));
    }
    Row {
        id: case.id,
        assertion: case.assertion,
        result: if errors.is_empty() && count > 0 {
            "PASS"
        } else {
            "FAIL"
        },
        test_instances: count,
        errors,
    }
}

pub(super) fn command<P: Serialize>(
    directory: &Path,
    output: Option<PathBuf>,
    suite: &str,
    format: &'static str,
    behaviors: &[Behavior],
    profile: P,
) -> Result<ExitCode> {
    let bytes = fs::read(directory.join(format!("{suite}.cases.json")))
        .with_context(|| format!("read {suite} case manifest"))?;
    let definitions: Vec<Case> = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse {suite} case definitions"))?;
    ensure!(
        definitions.len() == behaviors.len(),
        "expected {} named {suite} cases, found {}",
        behaviors.len(),
        definitions.len()
    );
    let mut seen = BTreeSet::new();
    let cases: Vec<_> = definitions
        .into_iter()
        .map(|case| execute(directory, suite, behaviors, case, &mut seen))
        .collect();
    let matched = cases.iter().filter(|row| row.result == "PASS").count();
    let run = Run {
        format,
        tool: concat!("gdd-conformance/", env!("CARGO_PKG_VERSION")),
        runner: "rust",
        profile,
        case_count: cases.len(),
        test_instances: cases.iter().map(|r| r.test_instances).sum(),
        passed: matched == cases.len(),
        cases,
    };
    crate::receipt::publish(&run, output, suite, matched, run.case_count)
}

#[cfg(test)]
mod tests;
