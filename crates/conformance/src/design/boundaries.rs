use super::*;
use gdd::boundaries::boundary_view;
use gdd::context::ContextGraph;
use gdd::report::CheckResult::{Fail, Pass};
use serde_json::json;

const SCREEN_CALL: &str = "edge.screen.reserve.calls";

fn contains(list: &Value, id: &str) -> Result<bool> {
    Ok(list
        .as_array()
        .context("expected a list of IDs")?
        .iter()
        .any(|value| value == id))
}

pub(super) fn complete_boundary_readiness(model: &mut Model, _: &str) -> Result<()> {
    model.model(Pass)?;
    model.readiness(RESERVE, Pass)?;
    Ok(())
}

pub(super) fn partial_boundary_sketch_and_readiness(model: &mut Model, field: &str) -> Result<()> {
    remove(&mut model.data["nodes"][PROVIDER]["boundary"], field)?;
    model.save()?;
    model.model(Pass)?;
    diagnostic(
        &model.readiness(RESERVE, Fail)?,
        "BOUNDARY_RATIONALE_MISSING",
    )
}

pub(super) fn blank_boundary_is_invalid(model: &mut Model, field: &str) -> Result<()> {
    model.data["nodes"][PROVIDER]["boundary"][field] = json!(" \n\t");
    model.save()?;
    model.model(Fail)?;
    Ok(())
}

pub(super) fn explicit_empty_exclusions(model: &mut Model, _: &str) -> Result<()> {
    model.data["nodes"][PROVIDER]["boundary"]["excludes"] = json!([]);
    model.save()?;
    model.readiness(RESERVE, Pass)?;
    Ok(())
}

pub(super) fn unrelated_incomplete_component(model: &mut Model, _: &str) -> Result<()> {
    model.data["nodes"]["component.island"] =
        json!({"kind":"component","description":"Unrelated sketch."});
    model.save()?;
    model.readiness(RESERVE, Pass)?;
    model.readiness("component.island", Fail)?;
    Ok(())
}

pub(super) fn legacy_semantics_preserved(directory: &Path, _: &str) -> Result<()> {
    let path = directory.join("../examples/concert.gdd.yaml");
    assess(
        &path,
        Profile::Readiness,
        &[RESERVE.into()],
        "readiness",
        Pass,
    )?;
    let data = read_document(&path)?;
    let api = &data["nodes"]["component.api"];
    ensure!(api.get("boundary").is_none() && contains(&api["interfaces"], RESERVE)?);
    Ok(())
}

pub(super) fn new_fields_cannot_silently_enter_legacy_format(
    model: &mut Model,
    _: &str,
) -> Result<()> {
    model.data["format"] = json!("gdd/0.2");
    model.save()?;
    model.model(Fail)?;
    Ok(())
}

pub(super) fn fragment_version_must_match(model: &mut Model, _: &str) -> Result<()> {
    let path = model.temp.path().join("fragment.json");
    model.data["includes"] = json!(["fragment.json"]);
    model.save()?;
    for (format, expected) in [("gdd-fragment/0.2", Fail), ("gdd-fragment/0.3", Pass)] {
        write_json(&path, &json!({"format":format,"nodes":{},"edges":[]}))?;
        model.model(expected)?;
    }
    Ok(())
}

pub(super) fn public_interface_requires_matching_owner(model: &mut Model, _: &str) -> Result<()> {
    model.data["nodes"]["component.api"]["interfaces"] = json!([RESERVE]);
    model.save()?;
    diagnostic(&model.model(Fail)?, "INTERFACE_OWNER_MISMATCH")
}

pub(super) fn cross_component_private_call(model: &mut Model, _: &str) -> Result<()> {
    model.data["nodes"][PROVIDER]["interfaces"] = json!([]);
    model.save()?;
    diagnostic(&model.model(Fail)?, "PRIVATE_ACTION_CALL")
}

pub(super) fn internal_private_call_allowed(model: &mut Model, _: &str) -> Result<()> {
    model.data["nodes"][PROVIDER]["interfaces"] = json!([]);
    let edges = model.edges()?;
    edges.retain(|e| e["relation"] != "calls");
    edges
        .push(json!({"id":"edge.internal","from":"action.cancel","relation":"calls","to":RESERVE}));
    model.save()?;
    model.model(Pass)?;
    let view = boundary_view(&model.bundle(), None)?;
    ensure!(
        view["calls"][0]["edge"] == "edge.internal" && view["calls"][0]["status"] == "internal"
    );
    Ok(())
}

pub(super) fn unresolved_call_valid_sketch_unready(model: &mut Model, missing: &str) -> Result<()> {
    let scope = match missing {
        "interfaces" => {
            remove(&mut model.data["nodes"][PROVIDER], "interfaces")?;
            RESERVE
        }
        "provider_owner" | "consumer_owner" => {
            let target = if missing == "provider_owner" {
                RESERVE
            } else {
                "screen.reserve"
            };
            model
                .edges()?
                .retain(|e| !(e["from"] == target && e["relation"] == "owned_by"));
            target
        }
        _ => anyhow::bail!("unknown case parameter {missing}"),
    };
    model.save()?;
    model.model(Pass)?;
    diagnostic(&model.readiness(scope, Fail)?, "PUBLIC_CONTRACT_UNRESOLVED")
}

pub(super) fn postconditions_required_in_read_context(model: &mut Model, _: &str) -> Result<()> {
    remove(
        &mut model.data["nodes"]["action.cancel"]["contract"],
        "postconditions",
    )?;
    model.save()?;
    model.model(Pass)?;
    model.readiness(RESERVE, Fail)?;
    Ok(())
}

pub(super) fn unrelated_private_call_does_not_block_readiness(
    model: &mut Model,
    _: &str,
) -> Result<()> {
    let nodes = &mut model.data["nodes"];
    nodes["component.island"] = json!({"kind":"component","description":"Isolated caller."});
    nodes["component.private"] =
        json!({"kind":"component","description":"Isolated provider.","interfaces":[]});
    nodes["action.private"] = json!({"kind":"action","description":"Isolated private action."});
    model.edges()?.extend([
        json!({"id":"edge.private.owner","from":"action.private","relation":"owned_by","to":"component.private"}),
        json!({"id":"edge.island.call","from":"component.island","relation":"calls","to":"action.private"}),
    ]);
    model.save()?;
    let report = model.readiness(RESERVE, Pass)?;
    ensure!(report.result("model") == Fail);
    diagnostic(&report, "PRIVATE_ACTION_CALL")
}

pub(super) fn community_overlap_and_owner_derived_membership(
    model: &mut Model,
    _: &str,
) -> Result<()> {
    let view = boundary_view(&model.bundle(), None)?;
    let backend = &view["communities"]["community.backend"];
    let feature = &view["communities"]["community.reservation_feature"];
    ensure!(contains(&backend["calls"]["inbound"], SCREEN_CALL)?);
    ensure!(contains(&feature["calls"]["internal"], SCREEN_CALL)?);
    ensure!(feature["membership_basis"][PROVIDER] == json!([RESERVE]));
    let contracts = feature["public_contracts"]
        .as_array()
        .context("missing public contracts")?;
    ensure!(!contracts.is_empty() && contracts.iter().all(|c| c["provider"] == PROVIDER));
    validate_boundaries(model.directory, &view)
}

pub(super) fn community_membership_does_not_grant_access(model: &mut Model, _: &str) -> Result<()> {
    model.data["nodes"][PROVIDER]["interfaces"] = json!([]);
    model.save()?;
    diagnostic(&model.model(Fail)?, "PRIVATE_ACTION_CALL")
}

pub(super) fn scoped_boundary_view_declares_omitted_calls(
    model: &mut Model,
    _: &str,
) -> Result<()> {
    let bundle = model.bundle();
    let scope = ContextGraph::new(&bundle)
        .scope([RESERVE])
        .nodes()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let view = boundary_view(&bundle, Some(&scope))?;
    ensure!(contains(&view["omitted_calls"], SCREEN_CALL)?);
    ensure!(contains(
        &view["communities"]["community.backend"]["omitted_calls"],
        SCREEN_CALL
    )?);
    ensure!(view["components"].get("component.frontend").is_none());
    Ok(())
}

pub(super) fn unrelated_unresolved_call_not_in_every_community(
    model: &mut Model,
    _: &str,
) -> Result<()> {
    model.data["nodes"]["component.island"] =
        json!({"kind":"component","description":"Unrelated caller.","interfaces":[]});
    model.data["nodes"]["action.island"] =
        json!({"kind":"action","description":"Unrelated ownerless action."});
    model.edges()?.push(json!({"id":"edge.island","from":"component.island","relation":"calls","to":"action.island"}));
    model.save()?;
    let view = boundary_view(&model.bundle(), None)?;
    ensure!(contains(&view["unresolved"], "edge.island")?);
    let communities = view["communities"]
        .as_object()
        .context("missing communities")?;
    ensure!(!communities.is_empty());
    for community in communities.values() {
        ensure!(!contains(&community["calls"]["unresolved"], "edge.island")?);
    }
    Ok(())
}

pub(super) fn boundary_and_contract_changes_change_revision(
    model: &mut Model,
    _: &str,
) -> Result<()> {
    let before = model.revision()?;
    model.data["nodes"][PROVIDER]["boundary"]["rationale"] =
        json!("A newly accepted boundary reason.");
    model.save()?;
    let after = model.revision()?;
    ensure!(after != before, "boundary edit retained original revision");
    model.data["nodes"][RESERVE]["contract"]["postconditions"]
        .as_array_mut()
        .context("missing postconditions")?
        .push(json!("An additional accepted obligation."));
    model.save()?;
    ensure!(
        model.revision()? != after,
        "contract edit retained previous revision"
    );
    Ok(())
}

pub(super) fn refinement_preserves_parent_boundary_context(
    model: &mut Model,
    rationale: &str,
) -> Result<()> {
    let mut parent = json!({"kind":"component","description":"Reservation feature boundary.","interfaces":[],
        "boundary":{"responsibility":"Preserve the reservation feature obligations.","excludes":[],"rationale":"Keep public obligations visible during decomposition."}});
    let expected = match rationale {
        "present" => Pass,
        "absent" => {
            remove(&mut parent["boundary"], "rationale")?;
            Fail
        }
        _ => anyhow::bail!("unknown case parameter {rationale}"),
    };
    model.data["nodes"]["component.parent"] = parent;
    model.edges()?.push(
        json!({"id":"edge.parent","from":PROVIDER,"relation":"refines","to":"component.parent"}),
    );
    model.save()?;
    ensure!(
        ContextGraph::new(&model.bundle())
            .scope([RESERVE])
            .contains("component.parent")
    );
    model.readiness(RESERVE, expected)?;
    Ok(())
}
