//! Structure and revision applicability of advisory design review observations.
use super::{boundaries::PRINCIPLES, context::ContextGraph, get, string, text};
use crate::bundle::{Bundle, DocumentKind};
use crate::input::Value;
use crate::report::CheckResult;
use crate::validation::{self, Profile};
use serde::Serialize;
use serde_json::{Value as Json, json};
use std::{collections::BTreeSet, io, sync::OnceLock};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ReviewStatus {
    Unknown,
    Invalid,
    Complete,
}

#[derive(Debug, Serialize)]
pub struct DesignReviewStatus {
    pub format: &'static str,
    pub spec_revision: Option<String>,
    pub status: ReviewStatus,
    pub reasons: Vec<String>,
    pub findings: Vec<Json>,
    pub scope: Vec<String>,
    pub requested_scope: Vec<String>,
    pub implementation: Option<String>,
    pub code_revision: Option<String>,
    pub notice: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_identity: Option<Json>,
}
impl DesignReviewStatus {
    pub fn exit_code(&self) -> u8 {
        match self.status {
            ReviewStatus::Complete => 0,
            ReviewStatus::Invalid => 1,
            ReviewStatus::Unknown => 2,
        }
    }
    fn invalid(mut self, reason: &str) -> Self {
        self.status = ReviewStatus::Invalid;
        self.reasons = vec![reason.into()];
        self
    }
    fn unknown(mut self, reason: &str) -> Self {
        self.reasons.push(reason.into());
        self
    }
}

fn schema() -> &'static jsonschema::Validator {
    static SCHEMA: OnceLock<jsonschema::Validator> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        let value = serde_json::from_str(include_str!("../../schemas/design-review.schema.json"))
            .expect("embedded review schema");
        validation::build_schema(&value).expect("review schema compiles offline")
    })
}

/// Assess a supplied advisory review against the captured GDD 0.3 model.
/// An explicitly empty expected scope is invalid. A result describes only the
/// applicability of the supplied assessments and grants no execution authority.
pub fn review_design(
    bundle: &Bundle,
    review: Option<&Value>,
    implementation: Option<&str>,
    code_revision: Option<&str>,
    expected_scope: Option<&[String]>,
) -> io::Result<DesignReviewStatus> {
    let representation = validation::validate(bundle, Profile::Representation);
    let mut result = DesignReviewStatus {
        format: "gdd-design-review-status/0.3",
        spec_revision: representation.spec_revision.clone(),
        status: ReviewStatus::Unknown,
        reasons: Vec::new(),
        findings: Vec::new(),
        scope: Vec::new(),
        requested_scope: expected_scope.unwrap_or_default().to_vec(),
        implementation: implementation.map(str::to_owned),
        code_revision: code_revision.map(str::to_owned),
        review_identity: None,
        notice: "Advisory review applicability only. This tool does not prove review execution or KISS/DRY compliance and does not grant acceptance.",
    };
    let Some(manifest) = bundle
        .manifest()
        .filter(|m| m.kind == DocumentKind::Manifest03 && result.spec_revision.is_some())
    else {
        return Ok(result.unknown("A resolved GDD 0.3 model snapshot is required."));
    };
    if representation.result("model") != CheckResult::Pass {
        return Ok(result.unknown("Model representation is unresolved or invalid."));
    }
    if super::projection_checks(bundle, manifest)
        .iter()
        .any(|c| c.result != CheckResult::Pass)
    {
        return Ok(result.unknown("Model semantics are unresolved or invalid."));
    }
    let Some(review) = validation::present(review) else {
        return Ok(result.unknown("No design review was supplied."));
    };
    if get(review, "format")
        .and_then(text)
        .is_some_and(|s| s != "gdd-design-review/0.3")
    {
        return Ok(result.unknown("Design review format is unsupported."));
    }
    let document = validation::to_json(review)?;
    result.reasons = schema()
        .iter_errors(&document)
        .map(|e| e.to_string())
        .collect();
    if !result.reasons.is_empty() {
        result.status = ReviewStatus::Invalid;
        return Ok(result);
    }
    result.scope = super::strings(review, "scope").map(str::to_owned).collect();
    result.review_identity = Some(
        json!({"spec_revision":document["spec_revision"],"implementation":document["implementation"],
        "code_revision":document["code_revision"],"scope":document["scope"],"reviewer":document["reviewer"]}),
    );
    let nodes: BTreeSet<_> = bundle.nodes().map(|n| n.id).collect();
    if let Some(expected) = expected_scope {
        if expected.is_empty() || expected.iter().any(|id| !nodes.contains(id.as_str())) {
            return Ok(result.invalid("Requested review scope must name existing model nodes."));
        }
        if expected.iter().any(|id| !result.scope.contains(id)) {
            result
                .reasons
                .push("Review does not cover the requested scope.".into());
        }
    }
    if result.scope.iter().any(|id| !nodes.contains(id.as_str())) {
        return Ok(result.invalid("Review scope names unknown model nodes."));
    }
    let context = ContextGraph::new(bundle).scope(super::strings(review, "scope"));
    let mut targets: BTreeSet<_> = context.nodes().collect();
    targets.extend(
        bundle
            .model_edges()
            .filter(|e| {
                context.contains(string(e.value, "from")) && context.contains(string(e.value, "to"))
            })
            .map(|e| e.id),
    );
    let mut seen = BTreeSet::new();
    for assessment in super::items(review, "assessments") {
        let principle = string(assessment, "principle");
        if !seen.insert(principle) {
            return Ok(result.invalid("A principle is assessed more than once."));
        }
        let findings = super::items(assessment, "findings");
        match string(assessment, "status") {
            "not_applicable" if !findings.is_empty() => {
                return Ok(
                    result.invalid("A not-applicable principle cannot also report findings.")
                );
            }
            "unknown" => result
                .reasons
                .push(format!("Review of {principle} is unresolved.")),
            _ => {}
        }
        for finding in findings {
            if super::strings(finding, "targets").any(|id| !targets.contains(id)) {
                return Ok(
                    result.invalid("Finding targets must belong to the review's model context.")
                );
            }
            let mut value = validation::to_json(finding)?;
            value["principle"] = json!(principle);
            result.findings.push(value);
        }
    }
    if PRINCIPLES.iter().any(|(p, _)| !seen.contains(p)) {
        result
            .reasons
            .push("All four design principles require an explicit assessment.".into());
    }
    if !super::flag(review, "executed") {
        result
            .reasons
            .push("The supplied document declares a planned review.".into());
    }
    if Some(string(review, "spec_revision")) != result.spec_revision.as_deref() {
        result
            .reasons
            .push("Review belongs to another model revision.".into());
    }
    if implementation.is_none_or(str::is_empty) || code_revision.is_none_or(str::is_empty) {
        result
            .reasons
            .push("The host must identify the current implementation and code revision.".into());
    } else if Some(string(review, "implementation")) != implementation
        || Some(string(review, "code_revision")) != code_revision
    {
        result
            .reasons
            .push("Review belongs to another implementation or code revision.".into());
    }
    if result.reasons.is_empty() {
        result.status = ReviewStatus::Complete;
        result.reasons.push("All four advisory assessments are present and match the supplied revisions; findings require judgment.".into());
    }
    Ok(result)
}
