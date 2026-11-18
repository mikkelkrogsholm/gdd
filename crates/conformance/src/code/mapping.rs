//! Mapping revisions, resource selection, ownership and model constraints.

use super::project::{Project, write_json};
use anyhow::{Context, Result, bail, ensure};
use gdd::code_check::CodeCheckOptions;
use serde_json::json;
use std::{fs, path::Path};

fn stale_revision() -> String {
    format!("sha256:{}", "0".repeat(64))
}

pub(super) fn unresolved_bindings_prevent_pass(_: &Path, change: &str) -> Result<()> {
    let mut project = Project::package()?;
    let records = project.records["records"]
        .as_object_mut()
        .context("expected record map")?;
    let selected = records
        .get_mut("binding.front")
        .context("missing front mapping")?;
    match change {
        "missing" => {
            records.remove("binding.front");
        }
        "ambiguous" => {
            let duplicate = selected.clone();
            records.insert("binding.duplicate".into(), duplicate);
        }
        "stale" => selected["spec_revision"] = stale_revision().into(),
        "other-implementation" => selected["implementation"] = "implementation.other".into(),
        "selector" => selected["selector"] = json!({"kind":"opaque","value":"VALUE"}),
        _ => bail!("unknown binding change {change}"),
    }
    project.save_records()?;
    project.expect("UNKNOWN")?;
    Ok(())
}

pub(super) fn unrelated_historical_binding_does_not_block_current_mapping(
    _: &Path,
    _: &str,
) -> Result<()> {
    let mut project = Project::package()?;
    let mut old = project.records["records"]["binding.front"].clone();
    old["spec_revision"] = stale_revision().into();
    old["implementation"] = "implementation.history".into();
    project.records["records"]["binding.history"] = old;
    project.save_records()?;
    project.expect("PASS")?;
    Ok(())
}

pub(super) fn supported_resource_expansion(_: &Path, resource: &str) -> Result<()> {
    let mut project = Project::package_with("direct", "front")?;
    project.records["records"] = json!({});
    project.bind("front", "front", resource)?;
    project.save_records()?;
    project.expect("PASS")?;
    Ok(())
}

pub(super) fn unsafe_binding_resource_is_unknown(_: &Path, resource: &str) -> Result<()> {
    let mut project = Project::package()?;
    project.records["records"]["binding.front"]["resource"] = resource.into();
    project.save_records()?;
    project.expect("UNKNOWN")?;
    Ok(())
}

pub(super) fn unsupported_selection_has_no_false_pass(_: &Path, option: &str) -> Result<()> {
    let mut project = Project::package()?;
    match option {
        "profile" => {
            project.expect_with(
                CodeCheckOptions {
                    profile: "python-imports/9.9",
                    ..project.options()
                },
                "UNKNOWN",
            )?;
        }
        "scope" => {
            project.expect_with(
                CodeCheckOptions {
                    scope: &["component.front".into()],
                    ..project.options()
                },
                "UNKNOWN",
            )?;
        }
        "constraint" => {
            project.model["nodes"]["rule.boundary"]["rule"]["constraint"]["relation"] =
                "calls".into();
            project.save_model_and_update_bindings()?;
            project.expect("UNKNOWN")?;
        }
        _ => bail!("unknown selection {option}"),
    }
    Ok(())
}

pub(super) fn checked_resource_revision_must_match_current_bytes(_: &Path, _: &str) -> Result<()> {
    let mut project = Project::package()?;
    let selected = &mut project.records["records"]["binding.front"];
    selected["status"] = "checked".into();
    selected["revision"] = stale_revision().into();
    selected["code_revision"] = "historical-observation".into();
    project.save_records()?;
    project.expect("UNKNOWN")?;
    let hash = gdd::sha256(&fs::read(project.source.join("pkg/front.py"))?);
    project.records["records"]["binding.front"]["revision"] = format!("sha256:{hash}").into();
    project.save_records()?;
    project.expect("PASS")?;
    Ok(())
}

pub(super) fn noncomponent_binding_requires_unique_explicit_owner(
    _: &Path,
    owners: &str,
) -> Result<()> {
    let mut project = Project::package()?;
    project.model["nodes"]["action.front"] =
        json!({"kind":"action","description":"Represent front operations."});
    let edges = project.model["edges"]
        .as_array_mut()
        .context("expected model edges")?;
    edges.push(json!({"id":"edge.owner.front","from":"action.front","to":"component.front","relation":"owned_by"}));
    let expected = match owners {
        "unique" => "PASS",
        "ambiguous" => {
            edges.push(json!({"id":"edge.owner.middle","from":"action.front","to":"component.middle","relation":"owned_by"}));
            "UNKNOWN"
        }
        _ => bail!("unknown owner variant {owners}"),
    };
    project.records["records"]["binding.front"]["targets"] = json!(["action.front"]);
    project.save_model_and_update_bindings()?;
    project.expect(expected)?;
    Ok(())
}

pub(super) fn additional_records_cannot_override_declared_mapping(
    _: &Path,
    defect: &str,
) -> Result<()> {
    let project = Project::package()?;
    let path = project.dir.path().join("extra.records.json");
    let mut additional = project.records.clone();
    match defect {
        "wrong-project" => {
            additional["project_id"] = "project.other".into();
            additional["records"] = json!({});
        }
        "duplicate-record" => {}
        _ => bail!("unknown record variant {defect}"),
    }
    write_json(&path, &additional)?;
    project.expect_with(
        CodeCheckOptions {
            records: Some(&path),
            ..project.options()
        },
        "UNKNOWN",
    )?;
    Ok(())
}

pub(super) fn no_selected_constraints_cannot_produce_vacuous_pass(_: &Path, _: &str) -> Result<()> {
    let mut project = Project::package()?;
    project.model["nodes"]
        .as_object_mut()
        .context("expected nodes")?
        .remove("rule.boundary");
    project.save_model_and_update_bindings()?;
    project.expect("UNKNOWN")?;
    Ok(())
}

pub(super) fn already_declared_records_are_not_loaded_twice(_: &Path, _: &str) -> Result<()> {
    let project = Project::package()?;
    let report = project.expect_with(
        CodeCheckOptions {
            records: Some(&project.ledger),
            ..project.options()
        },
        "PASS",
    )?;
    ensure!(
        report["bindings"]
            .as_object()
            .context("missing bindings")?
            .len()
            == 2,
        "expected two bindings"
    );
    Ok(())
}
