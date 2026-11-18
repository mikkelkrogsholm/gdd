use super::{Graph, Model, entries, get, has_cycle, items, string, strings, text};
use crate::bundle::LocatedRecord;
use crate::input::Value;
use crate::report::{Check, CheckResult, Diagnostic};
use crate::{sha256, validation::to_json};
use std::collections::BTreeMap;

impl Model<'_> {
    pub(super) fn records(&mut self) {
        let start = self.checks.len();
        let records: BTreeMap<_, _> = self
            .bundle
            .records()
            .map(|record| (record.id, record))
            .collect();
        let mut ids = BTreeMap::new();
        let mut count = |id| *ids.entry(id).or_insert(0) += 1;
        for field in ["sources", "questions", "extensions"] {
            for (id, _) in entries(&self.manifest.value, field) {
                count(id.as_str());
            }
        }
        for edge in self.bundle.edges() {
            count(edge.id);
        }
        for node in self.bundle.nodes() {
            count(node.id);
            if let Some(machine) = get(node.value, "machine") {
                for transition in items(machine, "transitions") {
                    count(string(transition, "id"));
                }
            }
        }

        let revision = self
            .bundle
            .snapshot()
            .map(|snapshot| snapshot.revision.as_str());
        let mut supersession = Graph::new();
        for (&id, item) in &records {
            let record = item.value;
            let kind = string(record, "kind");
            if ids.contains_key(id) {
                self.record_issue(
                    item,
                    "ID_COLLISION",
                    CheckResult::Fail,
                    "",
                    format!("Record collides with model ID {id}."),
                );
            }
            let parents = supersession.entry(id).or_default();
            if let Some(previous) = get(record, "supersedes").and_then(text) {
                parents.push(previous);
                if records
                    .get(previous)
                    .is_none_or(|prior| string(prior.value, "kind") != kind)
                {
                    self.record_issue(
                        item,
                        "RECORD_INVALID",
                        CheckResult::Fail,
                        "/supersedes",
                        format!("Invalid supersedes reference in {id}."),
                    );
                }
            }

            let spec_revision = get(record, "spec_revision").and_then(text);
            if revision.is_none() || spec_revision != revision {
                self.record_issue(
                    item,
                    "REVISION_UNRESOLVED",
                    CheckResult::Unknown,
                    "/spec_revision",
                    format!("Historical snapshot unavailable for {id}."),
                );
            } else {
                let field = if get(record, "targets").is_some() {
                    "targets"
                } else {
                    "scope"
                };
                for (index, target) in strings(record, field).enumerate() {
                    let issue = match ids.get(target) {
                        None => "unknown",
                        Some(1) => continue,
                        _ => "ambiguous",
                    };
                    self.record_issue(
                        item,
                        "RECORD_INVALID",
                        CheckResult::Fail,
                        &format!("/{field}/{index}"),
                        format!("Record {id} has {issue} current target {target}."),
                    );
                }
            }

            match kind {
                "decision" => {
                    for (index, target) in strings(record, "checks").enumerate() {
                        if records
                            .get(target)
                            .is_none_or(|check| string(check.value, "kind") != "check")
                        {
                            self.record_issue(
                                item,
                                "RECORD_INVALID",
                                CheckResult::Fail,
                                &format!("/checks/{index}"),
                                format!("Record {id} requires check record {target}."),
                            );
                        }
                    }
                }
                "evidence" => {
                    let target = string(record, "check");
                    let plan = records.get(target).map(|item| item.value);
                    if plan.is_none_or(|plan| string(plan, "kind") != "check") {
                        self.record_issue(
                            item,
                            "RECORD_INVALID",
                            CheckResult::Fail,
                            "/check",
                            format!("Record {id} requires check record {target}."),
                        );
                    }
                    if let Some(plan) = plan {
                        self.evidence(item, plan);
                    }
                }
                _ => {}
            }
        }
        if has_cycle(&supersession) {
            self.issue(
                "RECORD_INVALID",
                "Record supersession contains a cycle.",
                "",
                "",
            );
        }
        for (id, question) in entries(&self.manifest.value, "questions") {
            if string(question, "status") == "resolved"
                && let Some(resolution) = get(question, "resolution")
                && records
                    .get(string(resolution, "decision"))
                    .is_none_or(|item| string(item.value, "kind") != "decision")
            {
                self.issue(
                    "RECORD_INVALID",
                    "Question resolution must refer to a decision record.",
                    id,
                    "/resolution/decision",
                );
            }
        }
        if self.checks.len() == start {
            self.checks.push(Check::new(
                "records.integrity",
                "model",
                CheckResult::Pass,
                "Record identities, references, supersession and current snapshot scopes checked.",
            ));
        }
    }

    fn evidence(&mut self, item: &LocatedRecord<'_>, plan: &Value) {
        let record = item.value;
        let id = item.id;
        if string(plan, "kind") == "check" && string(record, "check_revision") != check_digest(plan)
        {
            self.record_issue(
                item,
                "CHECK_REVISION_MISMATCH",
                CheckResult::Fail,
                "/check_revision",
                format!("Evidence {id} does not match immutable check digest."),
            );
        }
        if let Some(revision) = get(record, "spec_revision").and_then(text)
            && Some(revision) != get(plan, "spec_revision").and_then(text)
        {
            self.record_issue(
                item,
                "RECORD_INVALID",
                CheckResult::Fail,
                "/spec_revision",
                format!("Evidence {id} and its check have different spec revisions."),
            );
        }
        if strings(plan, "scope")
            .any(|target| !strings(record, "scope").any(|member| target == member))
        {
            self.record_issue(
                item,
                "RECORD_INVALID",
                CheckResult::Fail,
                "/scope",
                format!("Evidence {id} does not cover check scope."),
            );
        }
    }

    fn record_issue(
        &mut self,
        item: &LocatedRecord<'_>,
        code: &'static str,
        result: CheckResult,
        suffix: &str,
        message: impl Into<String>,
    ) {
        let diagnostic = Diagnostic::new(
            code,
            result,
            message,
            Some(&item.location.file),
            format!("{}{suffix}", item.location.pointer),
        )
        .with_related_ids([item.id]);
        self.push(diagnostic, result);
    }
}

fn check_digest(plan: &Value) -> String {
    // The strict parser stores object members in sorted order. Check plans have
    // only string and array values, so compact UTF-8 JSON is canonical here.
    let json = to_json(plan).expect("schema-validated check plan");
    let bytes = serde_json::to_vec(&json).expect("JSON serializes to bytes");
    format!("sha256:{}", sha256(&bytes))
}
