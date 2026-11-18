use super::*;
use gdd::boundaries::PRINCIPLES;
use gdd::design_review::{DesignReviewStatus, ReviewStatus, review_design};
use gdd::input::Value as Input;
use serde_json::json;

pub(super) const IMPLEMENTATION: &str = "implementation.synthetic";
pub(super) const CODE_REVISION: &str = "synthetic-code";

pub(super) fn complete(model: &Model) -> Result<Value> {
    let assessments = PRINCIPLES.iter().map(|(principle, _)| json!({
        "principle":principle, "status":"reviewed", "rationale":"Synthetic assessment for envelope testing only.", "findings":[]
    })).collect::<Vec<_>>();
    Ok(
        json!({"format":"gdd-design-review/0.3","spec_revision":model.revision()?,
        "implementation":IMPLEMENTATION,"code_revision":CODE_REVISION,"scope":[RESERVE],
        "reviewer":{"name":"synthetic-reviewer","version":"1"},"executed":true,"assessments":assessments}),
    )
}

fn finding() -> Value {
    json!({"targets":[RESERVE],"code_locations":[{"resource":"src/reservations.py","selector":"reserve"}],
        "consequence":"An unused layer adds navigation cost.","recommendation":"Consider a direct function."})
}

fn status(
    model: &Model,
    review: Option<&Value>,
    scope: Option<&[String]>,
) -> Result<DesignReviewStatus> {
    Ok(review_design(
        &model.bundle(),
        review.map(Input::from).as_ref(),
        Some(IMPLEMENTATION),
        Some(CODE_REVISION),
        scope,
    )?)
}

pub(super) fn broad_principle_is_advisory(model: &mut Model, category: &str) -> Result<()> {
    let blocking = match category {
        "design" => true,
        "architecture" => false,
        _ => anyhow::bail!("unknown rule category {category}"),
    };
    let rule = &mut model.data["nodes"]["rule.design.kiss"]["rule"];
    rule["category"] = json!(category);
    rule["blocking"] = json!(blocking);
    model.save()?;
    diagnostic(&model.model(CheckResult::Fail)?, "DESIGN_PRINCIPLE_INVALID")
}

pub(super) fn precise_design_rule_can_still_block(model: &mut Model, _: &str) -> Result<()> {
    let rule = &mut model.data["nodes"]["rule.design.kiss"]["rule"];
    remove(rule, "principle")?;
    rule["blocking"] = json!(true);
    model.save()?;
    model.model(CheckResult::Pass)?;
    Ok(())
}

pub(super) fn advisory_review_complete_even_with_findings(
    model: &mut Model,
    _: &str,
) -> Result<()> {
    let mut review = complete(model)?;
    review["assessments"][0]["findings"] = json!([finding()]);
    let result = status(model, Some(&review), None)?;
    ensure!(result.status == ReviewStatus::Complete && !result.findings.is_empty());
    ensure!(result.scope == [RESERVE]);
    let report = serde_json::to_value(result)?;
    ensure!(
        report.get("result").is_none()
            && report["review_identity"]["code_revision"] == CODE_REVISION
    );
    Ok(())
}

pub(super) fn review_unknown_is_explicit(model: &mut Model, condition: &str) -> Result<()> {
    let mut review = complete(model)?;
    let wide_scope = [RESERVE.into(), "action.cancel".into()];
    match condition {
        "absent" | "narrow_scope" => {}
        "planned" => review["executed"] = json!(false),
        "missing_principle" => {
            review["assessments"]
                .as_array_mut()
                .context("missing assessments")?
                .pop();
        }
        "unknown" => review["assessments"][0]["status"] = json!("unknown"),
        "stale_model" => review["spec_revision"] = json!(format!("sha256:{}", "0".repeat(64))),
        "stale_code" => review["code_revision"] = json!("synthetic-old"),
        "unsupported" => review["format"] = json!("gdd-design-review/9.0"),
        _ => anyhow::bail!("unknown review condition {condition}"),
    }
    let supplied = (condition != "absent").then_some(&review);
    let scope = (condition == "narrow_scope").then_some(wide_scope.as_slice());
    let result = status(model, supplied, scope)?;
    ensure!(
        result.status == ReviewStatus::Unknown && !result.reasons.is_empty(),
        "expected explicit UNKNOWN, got {result:?}"
    );
    Ok(())
}

pub(super) fn invalid_review_cannot_be_complete(model: &mut Model, condition: &str) -> Result<()> {
    let mut review = complete(model)?;
    match condition {
        "duplicate" => review["assessments"][1]["principle"] = json!("kiss"),
        "unknown_scope" => review["scope"] = json!(["action.unknown"]),
        other => {
            let assessment = &mut review["assessments"][0];
            let mut item = finding();
            match other {
                "unknown_target" => item["targets"] = json!(["action.unknown"]),
                "outside_context" => item["targets"] = json!(["screen.reserve"]),
                "missing_location" => item["code_locations"] = json!([]),
                "not_applicable_findings" => assessment["status"] = json!("not_applicable"),
                _ => anyhow::bail!("unknown review condition {other}"),
            }
            assessment["findings"] = json!([item]);
        }
    }
    let result = status(model, Some(&review), None)?;
    ensure!(
        result.status == ReviewStatus::Invalid,
        "expected INVALID, got {result:?}"
    );
    Ok(())
}

pub(super) fn invalid_model_cannot_supply_complete_review(
    model: &mut Model,
    _: &str,
) -> Result<()> {
    let mut review = complete(model)?;
    model.data["nodes"][PROVIDER]["interfaces"] = json!([]);
    model.save()?;
    ensure!(status(model, Some(&review), None)?.status == ReviewStatus::Unknown);
    // Rebind the review to isolate invalid model semantics from stale identity.
    review["spec_revision"] = json!(model.revision()?);
    ensure!(status(model, Some(&review), None)?.status == ReviewStatus::Unknown);
    Ok(())
}

pub(super) fn complete_review_does_not_replace_blocking_evidence(
    model: &mut Model,
    _: &str,
) -> Result<()> {
    ensure!(status(model, Some(&complete(model)?), None)?.status == ReviewStatus::Complete);
    assess(
        &model.path,
        Profile::Records,
        &[RESERVE.into()],
        "implementation",
        CheckResult::Unknown,
    )?;
    Ok(())
}
