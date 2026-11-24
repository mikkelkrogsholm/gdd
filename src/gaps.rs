//! Advisory checks over the full graph captured in a native view.
use crate::{perspectives::load_perspective_bundle, view::view_data};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::Path,
};

#[derive(Debug, Serialize)]
pub struct Finding {
    pub target: String,
    pub key: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub omission_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GapReport {
    pub format: &'static str,
    pub spec_revision: String,
    pub scope: Vec<String>,
    pub unsupported_extensions: Vec<String>,
    pub findings: Vec<Finding>,
}

impl GapReport {
    pub fn open_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.omission_reason.is_none())
            .count()
    }
}

fn array(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}

fn strings(value: &Value) -> impl Iterator<Item = &str> {
    array(value).filter_map(Value::as_str)
}

fn fields(value: &Value) -> impl Iterator<Item = (&String, &Value)> {
    value.as_object().into_iter().flatten()
}

fn finding(
    record: &Value,
    target: &str,
    key: &str,
    kind: &'static str,
    message: String,
) -> Finding {
    let omission_reason = record["annotations"]["gdd.gap_omissions"][key]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    Finding {
        target: target.into(),
        key: key.into(),
        message,
        source: None,
        kind,
        omission_reason,
    }
}

/// Analyze a payload admitted by read_view or produced by view_data. Scoped views
/// retain all edges, so an owner outside the drawing is still a real owner.
pub fn from_view(view: &Value) -> GapReport {
    let model = &view["model"];
    let nodes = &model["nodes"];
    let sources = &model["sources"];
    let shown: BTreeSet<_> = strings(&view["manifest"]["shown_nodes"]).collect();
    let scope: Vec<_> = strings(&view["scope"]).map(str::to_owned).collect();
    let mut findings = Vec::new();
    let mut owned = BTreeSet::new();
    let mut exercising = BTreeMap::<&str, BTreeSet<&str>>::new();
    let mut verified = BTreeSet::new();
    let mut referenced = BTreeSet::new();
    for edge in array(&model["edges"]) {
        let (Some(from), Some(to)) = (edge["from"].as_str(), edge["to"].as_str()) else {
            continue;
        };
        match edge["relation"].as_str() {
            Some("owned_by") => {
                owned.insert(from);
            }
            Some("verifies") => {
                verified.insert(to);
            }
            Some("exercises") => {
                exercising.entry(to).or_default().insert(from);
            }
            _ => {}
        }
    }
    for record in fields(nodes)
        .map(|(_, node)| node)
        .chain(array(&model["edges"]))
    {
        for field in ["source_refs", "design_refs"] {
            referenced.extend(array(&record[field]).filter_map(|r| r["source"].as_str()));
        }
    }
    for gap in array(&view["perspectives"]["gaps"]) {
        if let (Some(id), Some(code), Some(message)) = (
            gap["node_id"].as_str(),
            gap["code"].as_str(),
            gap["message"].as_str(),
        ) && shown.contains(id)
        {
            findings.push(finding(&nodes[id], id, code, "structural", message.into()));
        }
    }
    for (id, question) in fields(&model["questions"]) {
        if question["status"] == "open" {
            let blocks = strings(&question["blocks"]).collect::<Vec<_>>().join(", ");
            for target in strings(&question["targets"]).filter(|t| shown.contains(t)) {
                findings.push(finding(
                    question,
                    target,
                    &format!("OPEN_QUESTION/{id}"),
                    "review",
                    format!(
                        "{} (blocks {blocks})",
                        question["statement"].as_str().unwrap_or_default()
                    ),
                ));
            }
        }
    }
    for id in shown {
        let node = &nodes[id];
        let mut add = |key: &str, message: String| {
            findings.push(finding(node, id, key, "structural", message));
        };
        match node["kind"].as_str() {
            Some("action") => {
                if !owned.contains(id) {
                    add(
                        "MISSING_ACTION_OWNER",
                        "No component owns this action.".into(),
                    );
                }
                let scenarios = exercising.get(id);
                let category = |scenario: &str| nodes[scenario]["scenario"]["category"].as_str();
                if !scenarios
                    .into_iter()
                    .flatten()
                    .any(|s| category(s) == Some("normal"))
                {
                    add(
                        "MISSING_NORMAL_SCENARIO",
                        "No normal scenario exercises this action.".into(),
                    );
                }
                let contract = &node["contract"];
                for (error, _) in fields(&contract["errors"]) {
                    let mut cases = strings(&contract["error_cases"][error]).peekable();
                    if cases.peek().is_none()
                        || cases.any(|s| {
                            !scenarios.is_some_and(|set| set.contains(s))
                                || !matches!(category(s), Some("negative" | "boundary"))
                        })
                    {
                        add(
                            &format!("MISSING_ERROR_CASE/{error}"),
                            format!(
                                "Error {error} needs a negative or boundary scenario that exercises this action for each listed case."
                            ),
                        );
                    }
                }
            }
            Some("rule") if node["rule"]["blocking"] == true && !verified.contains(id) => {
                add(
                    "MISSING_RULE_SCENARIO",
                    "No scenario verifies this blocking rule.".into(),
                );
            }
            Some("machine") => machine_gaps(node, add),
            _ => {}
        }
        // Source-backed proposals from an agent or reviewer remain questions.
        for (key, candidate) in fields(&node["annotations"]["gdd.gap_candidates"]) {
            let source = candidate["source"]
                .as_str()
                .filter(|id| sources.get(id).is_some());
            let question = candidate["question"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let (kind, message) = match (source, question) {
                (Some(_), Some(question)) => ("review", question.to_owned()),
                _ => (
                    "structural",
                    format!("Review candidate {key} needs a question and a registered source."),
                ),
            };
            let mut item = finding(node, id, &format!("CANDIDATE/{key}"), kind, message);
            item.source = source.map(str::to_owned);
            findings.push(item);
        }
    }
    if scope.is_empty() {
        for (id, source) in fields(sources) {
            if source["role"] == "knowledge"
                && source["normative"] == true
                && !referenced.contains(id.as_str())
            {
                findings.push(finding(
                    source,
                    id,
                    "UNREFERENCED_KNOWLEDGE",
                    "structural",
                    "No node or edge references this normative knowledge source.".into(),
                ));
            }
        }
    }
    findings.sort_by(|a, b| (&a.target, &a.key).cmp(&(&b.target, &b.key)));
    GapReport {
        format: "gdd-gaps/0.1",
        spec_revision: view["spec_revision"].as_str().unwrap_or_default().into(),
        scope,
        unsupported_extensions: strings(&view["perspectives"]["unsupported_extensions"])
            .map(str::to_owned)
            .collect(),
        findings,
    }
}

fn machine_gaps(node: &Value, mut add: impl FnMut(&str, String)) {
    let machine = &node["machine"];
    let declared: BTreeSet<_> = array(&machine["transitions"])
        .filter_map(|t| Some((t["from"].as_str()?, t["trigger"].as_str()?)))
        .collect();
    for (state, triggers) in fields(&node["annotations"]["gdd.expected_triggers"]) {
        for trigger in strings(triggers).collect::<BTreeSet<_>>() {
            if !declared.contains(&(state.as_str(), trigger)) {
                add(
                    &format!("MISSING_TRANSITION/{state}/{trigger}"),
                    format!(
                        "No transition handles {trigger} in {state}, although the machine lists it as expected."
                    ),
                );
            }
        }
    }
    let Some(initial) = machine["initial"].as_str() else {
        add(
            "MISSING_INITIAL_STATE",
            "The machine has no declared initial state.".into(),
        );
        return;
    };
    let mut next = BTreeMap::<&str, Vec<&str>>::new();
    for transition in array(&machine["transitions"]) {
        if let (Some(from), Some(to)) = (transition["from"].as_str(), transition["to"].as_str()) {
            next.entry(from).or_default().push(to);
        }
    }
    let terminal: BTreeSet<_> = strings(&machine["terminal"]).collect();
    let mut stack = vec![initial];
    let mut reachable = BTreeSet::new();
    while let Some(state) = stack.pop() {
        if reachable.insert(state)
            && let Some(neighbors) = next.get(state)
        {
            stack.extend(neighbors);
        }
    }
    for state in strings(&machine["states"]) {
        if !reachable.contains(state) {
            add(
                &format!("UNREACHABLE_STATE/{state}"),
                format!("No transition path reaches {state} from {initial}."),
            );
        } else if !terminal.contains(state) && !next.contains_key(state) {
            add(
                &format!("NONTERMINAL_DEAD_END/{state}"),
                format!(
                    "Reachable state {state} has no outgoing transition or terminal declaration."
                ),
            );
        }
    }
}

pub fn check(path: &Path, scope: &[String]) -> io::Result<GapReport> {
    let capture = load_perspective_bundle(path)?;
    Ok(from_view(&view_data(&capture, scope)?))
}
