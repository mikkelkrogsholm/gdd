use super::*;
use gdd::boundaries::PRINCIPLES;
use gdd::input::Value as Input;
use gdd::workflow::{prepare_workflow, verify_workflow};
use serde_json::json;
use std::collections::BTreeSet;

fn plan(model: &Model) -> Result<Value> {
    let request = json!({
        "format":"gdd-work-request/0.1","id":"workflow.synthetic","objective":"Exercise revision-bound packets; no code execution.",
        "implementation":"implementation.synthetic","base_code_revision":"synthetic-code",
        "packages":[{"id":"package.reserve","scope":[RESERVE],"read_resources":["model.gdd.json"],
            "write_resources":["src/reservations.py"],"workspace":"workspaces/reservations","deliverables":["src/reservations.py"],
            "integration_plan":"Run actual reservation integration checks before acceptance."}],
        "serialized":[],"integration":{"owner":"actor.synthetic","checks":[]}
    });
    Ok(prepare_workflow(&model.path, &Input::from(&request), None)?.artifact)
}

fn integrity(model: &Model, plan: &Value) -> Result<CheckResult> {
    let report = verify_workflow(&model.path, &Input::from(plan), None)?;
    Ok(report
        .checks
        .iter()
        .find(|c| c.check_id == "workflow.packet_integrity")
        .context("missing packet integrity check")?
        .result)
}

pub(super) fn packet_contains_canonical_boundaries_and_review_topics(
    model: &mut Model,
    _: &str,
) -> Result<()> {
    let workplan = plan(model)?;
    let packet = &workplan["packages"][0];
    let postconditions = &packet["model_slice"]["nodes"][RESERVE]["contract"]["postconditions"];
    ensure!(
        postconditions.as_array().is_some_and(|a| !a.is_empty())
            && postconditions == &model.data["nodes"][RESERVE]["contract"]["postconditions"]
    );
    let rationale = &packet["boundaries"]["components"][PROVIDER]["boundary"]["rationale"];
    ensure!(
        rationale.as_str().is_some_and(|s| !s.is_empty())
            && rationale == &model.data["nodes"][PROVIDER]["boundary"]["rationale"]
    );
    let topics = packet["design_review"]
        .as_array()
        .context("missing advisory checklist")?
        .iter()
        .map(|topic| {
            topic["principle"]
                .as_str()
                .context("missing review principle")
        })
        .collect::<Result<BTreeSet<_>>>()?;
    ensure!(topics == PRINCIPLES.iter().map(|(id, _)| *id).collect());
    ensure!(
        packet["locked_contracts"]
            .as_array()
            .context("missing locked contracts")?
            .iter()
            .any(|c| c == PROVIDER)
    );
    Ok(())
}

pub(super) fn packet_omission_rejected(model: &mut Model, field: &str) -> Result<()> {
    let mut workplan = plan(model)?;
    ensure!(integrity(model, &workplan)? == CheckResult::Pass);
    remove(&mut workplan["packages"][0], field)?;
    ensure!(integrity(model, &workplan)? == CheckResult::Fail);
    Ok(())
}

pub(super) fn boundary_change_invalidates_plan(model: &mut Model, _: &str) -> Result<()> {
    let workplan = plan(model)?;
    ensure!(integrity(model, &workplan)? == CheckResult::Pass);
    model.data["nodes"][PROVIDER]["boundary"]["rationale"] = json!("A changed accepted reason.");
    model.save()?;
    ensure!(
        integrity(model, &workplan)? == CheckResult::Fail,
        "obsolete packet passed integrity check"
    );
    Ok(())
}
