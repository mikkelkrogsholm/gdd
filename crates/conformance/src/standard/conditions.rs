//! Synthetic host mandates, evidence, imports and plans for the standard cases.

use super::{Case, fixture};
use anyhow::{Context, Result, bail};
use gdd::{
    bundle, context::context_manifest, graphify::NODE_LINK_PROFILE, input, validation::Activity,
};
use serde_json::{Map, Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub(super) struct Inputs {
    pub activity: Option<Activity>,
    pub host: Option<input::Value>,
    pub parallel: Option<input::Value>,
    pub package: Option<input::Value>,
    pub unsupported_extensions: Vec<String>,
    pub graphify_profile: Option<&'static str>,
    pub audit: Option<input::Value>,
}

fn revision(path: &Path) -> Result<String> {
    let bundle = bundle::load_model_only(path);
    Ok(bundle
        .snapshot()
        .with_context(|| format!("resolve snapshot for {}", path.display()))?
        .revision
        .clone())
}

fn change_extension(directory: &Path, model: &mut Value, override_relation: bool) -> Result<()> {
    let path = directory.join("extensions/sensors.extension.json");
    let mut extension = fixture::read(&path)?;
    if override_relation {
        let relations = extension["relations"]
            .as_object_mut()
            .context("expected extension relations")?;
        let measures = relations
            .shift_remove("example.sensors.measures")
            .context("missing sensor relation")?;
        relations.insert("calls".into(), measures);
    } else {
        extension["version"] = "0.1.1".into();
    }
    fixture::write(&path, &extension)?;
    model["extensions"]["extension.sensors"]["sha256"] = gdd::sha256(&fs::read(path)?).into();
    Ok(())
}

fn evidence(id: &str, records: &mut Value, model: &Path) -> Result<input::Value> {
    let revision = revision(model)?;
    let records = records["records"]
        .as_object_mut()
        .context("expected records")?;
    for record in records.values_mut() {
        record["spec_revision"] = revision.as_str().into();
    }
    let plan = records
        .get("check.rule.capacity")
        .context("missing capacity check")?;
    let mut environment = Map::new();
    for item in plan["required_environment"]
        .as_array()
        .context("missing required environment")?
    {
        environment.insert(
            item.as_str().context("expected environment name")?.into(),
            "synthetic-actual-environment".into(),
        );
    }
    let mut evidence = json!({
        "kind": "evidence", "check": "check.rule.capacity",
        "check_revision": gdd::canonical_digest(plan),
        "implementation": "implementation.fixture", "spec_revision": revision,
        "code_revision": "synthetic-current", "checker": {"name":"synthetic", "version":"1"},
        "scope": plan["scope"], "result": "PASS", "executed": true,
        "reason":"Synthetic observation for validator testing; no application execution.",
        "environment": environment, "evidence_uri": "fixture://synthetic-only"
    });
    let host = json!({
        "implementation":"implementation.fixture", "code_revision":"synthetic-current",
        "trusted_evidence":["evidence.fixture"], "supported_methods":[plan["method"]],
        "environment":environment, "negative_controls":plan["negative_controls"],
        "verified_bindings":["binding.fixture"]
    });
    match id {
        "case.evidence.stale" => evidence["code_revision"] = "synthetic-old".into(),
        "case.evidence.environment" => {
            evidence["environment"] = json!({"stub_database":"synthetic-stub"})
        }
        "case.records.check_hash_changed" => {
            evidence["check_revision"] = format!("sha256:{}", "0".repeat(64)).into()
        }
        _ => bail!("unknown evidence condition {id}"),
    }
    records.insert("evidence.fixture".into(), evidence);
    records.insert(
        "binding.fixture".into(),
        json!({
            "kind":"binding", "spec_revision": revision, "targets":["rule.capacity"],
            "implementation":"implementation.fixture", "role":"test", "resource":"synthetic.py",
            "selector":{"kind":"whole-document"}, "status":"checked",
            "revision":"synthetic-resource", "code_revision":"synthetic-current"
        }),
    );
    Ok((&host).into())
}

fn import_loss(path: &Path) -> Result<input::Value> {
    let nodes: Vec<_> = ["a", "b"].iter().map(|id|
        json!({"id": id, "label":id, "file_type":"code", "source_file":format!("{id}.py")})
    ).collect();
    let links: Vec<_> = ["calls", "imports"]
        .iter()
        .map(|relation| json!({"source":"a", "target":"b", "relation":relation}))
        .collect();
    let graph =
        json!({"directed":true, "multigraph":false, "graph":{}, "nodes":nodes, "links":links});
    fixture::write(path, &graph)?;
    let mut reconstructed = graph.clone();
    reconstructed["links"] = json!(&links[..1]);
    Ok((&json!({"format":"gdd-import-audit/0.2", "original":graph, "reconstructed":reconstructed})).into())
}

fn parallel_plan(path: &Path, overlap: bool) -> Result<input::Value> {
    let scopes = if overlap {
        ["action.reserve", "action.cancel"]
    } else {
        fixture::write(
            path,
            &json!({
                "format":"gdd/0.2", "project":{"id":"example.parallel", "title":"Synthetic", "goal":"Test unknown declarations"},
                "nodes":{"action.a":{"kind":"action", "description":"A"}, "action.b":{"kind":"action", "description":"B"}},
                "edges":[]
            }),
        )?;
        ["action.a", "action.b"]
    };
    let revision = revision(path)?;
    let packages: Vec<_> = scopes
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let resource = if overlap {
                "src/contracts.py".into()
            } else {
                format!("src/{i}.py")
            };
            json!({"id":format!("package.{i}"), "scope":[s], "write_resources":[resource],
            "base_revision":revision, "integration_plan":"Synthetic integration plan"})
        })
        .collect();
    Ok((&json!({"format":"gdd-parallel-plan/0.2", "packages":packages, "serialized":[]})).into())
}

pub(super) fn prepare(
    case: &Case,
    directory: &Path,
    path: &mut PathBuf,
    mut data: Value,
) -> Result<Inputs> {
    let mut inputs = Inputs::default();
    match case.id.as_str() {
        "case.parse.duplicate_key" => fs::write(path, "format: gdd/0.2\nformat: gdd/0.2\n")?,
        "case.parse.alias" => fs::write(path, "format: gdd/0.2\na: &shared {}\nb: *shared\n")?,
        "case.auth.absent" => inputs.activity = Some(Activity::Implementation),
        "case.auth.standing" => {
            inputs.activity = Some(Activity::Implementation);
            inputs.host = Some((&json!({"policy":{
                "revision":"synthetic-policy-1", "actor":"actor.fixture", "activity":"implementation",
                "spec_revision":revision(path)?, "scope":["action.reserve"], "requirement":"standing",
                "authorities":["actor.fixture"], "basis":"Synthetic host mandate; no real authority."
            }})).into());
        }
        "case.extension.unsupported" => {
            inputs.unsupported_extensions.push("example.sensors".into())
        }
        "case.extension.override" | "case.extension.identity" => {
            change_extension(directory, &mut data, case.id == "case.extension.override")?;
            fixture::write(path, &data)?;
        }
        "case.include.collision" => {
            data["nodes"]["entity.note"] =
                json!({"kind":"entity", "description":"Collision fixture"});
            fixture::write(path, &data)?;
        }
        "case.evidence.stale" | "case.evidence.environment" | "case.records.check_hash_changed" => {
            let model = directory.join("concert.gdd.yaml");
            inputs.host = Some(evidence(&case.id, &mut data, &model)?);
            fixture::write(path, &data)?;
            *path = model;
        }
        "case.records.check_plan_missing" => {
            data["records"]
                .as_object_mut()
                .and_then(|r| r.shift_remove("check.rule.capacity"))
                .context("missing capacity check")?;
            fixture::write(path, &data)?;
            *path = directory.join("concert.gdd.yaml");
        }
        "case.integration.loss" => {
            *path = directory.join("graphify.json");
            inputs.graphify_profile = Some(NODE_LINK_PROFILE);
            inputs.audit = Some(import_loss(path)?);
        }
        "case.integration.unsupported" => {
            *path = directory.join("graphify-unknown.json");
            fixture::write(
                path,
                &json!({"nodes":[], "links":[], "schema_version":"99"}),
            )?;
            inputs.graphify_profile = Some(NODE_LINK_PROFILE);
        }
        "case.context.global" => {
            data["nodes"]["rule.capacity"]["rule"]["scope"] = "global".into();
            fixture::write(path, &data)?;
            let bundle = bundle::load_model_only(path);
            let scope = ["action.reserve".into()];
            let mut package = context_manifest(&bundle, &scope)?;
            let index = package
                .nodes
                .iter()
                .position(|n| *n == "rule.capacity")
                .context("global rule missing from context")?;
            package.nodes.remove(index);
            inputs.package = Some((&serde_json::to_value(package)?).into());
        }
        "case.parallel.unknown_effects" | "case.parallel.write_overlap" => {
            inputs.parallel = Some(parallel_plan(
                path,
                case.id == "case.parallel.write_overlap",
            )?);
        }
        // These conditions describe the manifest base or its declared patches.
        "case.model.parallel_edges"
        | "case.readiness.unrelated_question"
        | "case.readiness.design" => {}
        _ if case.condition.is_some() => bail!("unimplemented standard condition for {}", case.id),
        _ => {}
    }
    Ok(inputs)
}
