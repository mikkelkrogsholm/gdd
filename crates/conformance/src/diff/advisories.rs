//! Exercise structural advisory mutations and their negative controls.

use super::{Pair, edges, remove_node};
use anyhow::{Result, bail, ensure};
use gdd::model_diff::ModelDiff;
use serde_json::{Value, json};
use std::path::Path;

fn verifier(id: &str) -> Value {
    json!({"id":id, "from":"scenario.add", "to":"rule.boundary", "relation":"verifies"})
}

fn rule_pair(directory: &Path) -> Result<Pair> {
    let mut pair = Pair::new(directory)?;
    pair.model["nodes"]["rule.boundary"] = json!({"kind":"rule", "description":"No import.", "rule":{
        "category":"architecture", "statement":"No dependency.", "scope":"targeted", "blocking":true,
        "verification":["static_analysis"], "constraint":{"relation":"depends_on", "from":"component.arithmetic",
        "to":"action.add", "reach":"transitive", "policy":"forbid"}}});
    edges(&mut pair.model)?.push(verifier("edge.verifier"));
    Ok(pair)
}

fn has_advisory(report: &ModelDiff, code: &str) -> bool {
    report.advisories.iter().any(|a| a.code == code)
}

pub(super) fn weakening(directory: &Path, parameter: &str) -> Result<()> {
    let pair = rule_pair(directory)?;
    ensure!(pair.compare(&pair.model)?.advisories.is_empty());
    let mut head = pair.model.clone();
    let rule = &mut head["nodes"]["rule.boundary"]["rule"];
    let expected = match parameter {
        "nonblocking" => {
            rule["blocking"] = json!(false);
            "BLOCKING_RULE_RELAXED"
        }
        "remove_rule" => {
            remove_node(&mut head, "rule.boundary")?;
            "BLOCKING_RULE_RELAXED"
        }
        "direct" => {
            rule["constraint"]["reach"] = json!("direct");
            "FORBID_REACH_NARROWED"
        }
        "endpoint" => {
            rule["constraint"]["to"] = json!("component.arithmetic");
            "FORBID_ENDPOINT_CHANGED"
        }
        "remove_verifier" => {
            edges(&mut head)?.retain(|e| e["id"] != "edge.verifier");
            "LAST_VERIFIER_REMOVED"
        }
        "interfaces" => {
            head["nodes"]["component.arithmetic"]["interfaces"] = json!([]);
            "INTERFACE_SHRUNK"
        }
        "kind" => {
            head["nodes"]["action.add"] =
                json!({"kind":"component", "description":"An incompatible concept."});
            "IDENTITY_REVIEW"
        }
        _ => bail!("unknown weakening mutation {parameter}"),
    };
    ensure!(
        has_advisory(&pair.compare(&head)?, expected),
        "missing {expected}"
    );
    Ok(())
}

pub(super) fn contracts(directory: &Path, field: &str) -> Result<()> {
    let mut pair = Pair::new(directory)?;
    let (value, empty) = match field {
        "errors" => (
            json!({"Unavailable":"Return an unavailable error."}),
            json!({}),
        ),
        "preconditions" => (json!(["Inputs are integers."]), json!([])),
        "postconditions" => (json!(["Result equals left plus right."]), json!([])),
        _ => bail!("unknown contract field {field}"),
    };
    pair.model["nodes"]["action.add"]["contract"][field] = value;
    let mut head = pair.model.clone();
    head["nodes"]["action.add"]["contract"][field] = empty;
    ensure!(has_advisory(
        &pair.compare(&head)?,
        "CONTRACT_REQUIREMENT_REMOVED"
    ));
    std::mem::swap(&mut head, &mut pair.model);
    ensure!(!has_advisory(
        &pair.compare(&head)?,
        "CONTRACT_REQUIREMENT_REMOVED"
    ));
    Ok(())
}

pub(super) fn verifiers(directory: &Path, _: &str) -> Result<()> {
    let mut pair = rule_pair(directory)?;
    edges(&mut pair.model)?.push(verifier("edge.other_verifier"));
    pair.model["nodes"]["rule.boundary"]["rule"]["constraint"]["reach"] = json!("direct");
    let mut head = pair.model.clone();
    edges(&mut head)?.retain(|e| e["id"] != "edge.verifier");
    head["nodes"]["rule.boundary"]["rule"]["constraint"]["reach"] = json!("transitive");
    ensure!(pair.compare(&head)?.advisories.is_empty());
    Ok(())
}

pub(super) fn source_identity(directory: &Path, _: &str) -> Result<()> {
    let mut pair = Pair::new(directory)?;
    pair.model["sources"] = json!({"source.context":{"role":"input", "title":"Context", "uri":"context.txt",
        "format":"text/plain", "normative":false}});
    let mut head = pair.model.clone();
    head["sources"]["source.context"]["role"] = json!("knowledge");
    let report = pair.compare(&head)?;
    ensure!(report.exit_code == 0 && has_advisory(&report, "IDENTITY_REVIEW"));
    ensure!(
        report
            .advisories
            .first()
            .is_some_and(|a| a.message.contains("historical ID reuse"))
    );
    Ok(())
}
